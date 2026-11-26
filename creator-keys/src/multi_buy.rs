//! Slippage-protected atomic multi-buy (Issue #717).
//!
//! A single invocation purchases keys from several creators. Soroban's
//! transaction atomicity gives the legs all-or-nothing semantics: if any leg
//! breaches its `max_price` (or any single-buy guard rejects it), the whole
//! invocation reverts and no creator supply changes persist.

use crate::{
    events, read_registered_creator_profile, ContractError, CreatorKeysContract,
    CreatorKeysContractArgs, CreatorKeysContractClient, FeatureKey,
};
use soroban_sdk::{contractimpl, contracttype, token::TokenClient, Address, Env, Vec};

/// Maximum number of legs accepted by a single multi-buy.
pub const MAX_MULTI_BUY_LEGS: u32 = 10;

/// One leg of a multi-buy: buy `amount` keys from `creator`.
///
/// `max_price` is the ceiling for a single key in this leg. The worst-case
/// cost of the leg is therefore `amount * max_price`, which is also what the
/// pre-flight balance check reserves.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MultiBuyLeg {
    pub creator: Address,
    pub amount: u32,
    pub max_price: i128,
}

/// Per-leg execution result returned by [`CreatorKeysContract::multi_buy`].
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MultiBuyResult {
    pub creator: Address,
    pub amount: u32,
    pub total_cost: i128,
    pub new_supply: u32,
}

fn validate_legs(legs: &Vec<MultiBuyLeg>, deadline: u32, env: &Env) -> Result<i128, ContractError> {
    if legs.is_empty() {
        return Err(ContractError::LegsEmpty);
    }
    if legs.len() > MAX_MULTI_BUY_LEGS {
        return Err(ContractError::TooManyLegs);
    }
    if env.ledger().sequence() > deadline {
        return Err(ContractError::DeadlinePassed);
    }

    let mut seen: Vec<Address> = Vec::new(env);
    let mut worst_case_total: i128 = 0;
    for leg in legs.iter() {
        if seen.contains(&leg.creator) {
            return Err(ContractError::DuplicateCreator);
        }
        seen.push_back(leg.creator.clone());

        if leg.amount == 0 || leg.max_price < 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        let leg_budget = leg
            .max_price
            .checked_mul(leg.amount as i128)
            .ok_or(ContractError::Overflow)?;
        worst_case_total = worst_case_total
            .checked_add(leg_budget)
            .ok_or(ContractError::Overflow)?;
    }
    Ok(worst_case_total)
}

/// Checks the buyer's XLM balance against the worst-case cost of all legs.
///
/// The balance check is deliberately performed before any purchase: an
/// underfunded order must not mutate a single creator's supply. When no XLM
/// Stellar Asset Contract has been configured the check is skipped because the
/// contract has no token account to inspect.
fn assert_sufficient_balance(
    env: &Env,
    buyer: &Address,
    required: i128,
) -> Result<(), ContractError> {
    if let Some(token) = env
        .storage()
        .persistent()
        .get::<FeatureKey, Address>(&FeatureKey::XlmTokenAddress)
    {
        let balance = TokenClient::new(env, &token).balance(buyer);
        if balance < required {
            return Err(ContractError::InsufficientFunds);
        }
    }
    Ok(())
}

#[contractimpl]
impl CreatorKeysContract {
    /// Atomically buys keys from each leg's creator.
    ///
    /// Every leg runs through the standard `buy_keys_core` implementation, so
    /// curve pricing, per-creator fees, supply/holder updates and TTL
    /// extensions are identical to a stand-alone multi-key buy. A leg whose
    /// cost exceeds `amount * max_price` returns `SlippageExceeded` and rolls
    /// back the whole transaction.
    pub fn multi_buy(
        env: Env,
        buyer: Address,
        legs: Vec<MultiBuyLeg>,
        global_deadline_ledger: u32,
    ) -> Result<Vec<MultiBuyResult>, ContractError> {
        buyer.require_auth();

        let worst_case_total = validate_legs(&legs, global_deadline_ledger, &env)?;
        assert_sufficient_balance(&env, &buyer, worst_case_total)?;

        let mut results: Vec<MultiBuyResult> = Vec::new(&env);
        let mut total_cost: i128 = 0;

        for leg in legs.iter() {
            let leg_budget = leg
                .max_price
                .checked_mul(leg.amount as i128)
                .ok_or(ContractError::Overflow)?;
            let outcome = Self::buy_keys_core(
                env.clone(),
                leg.creator.clone(),
                buyer.clone(),
                leg.amount,
                i128::MAX,
                Some(leg_budget),
                None,
                None,
            )?;
            if outcome.keys_bought != leg.amount {
                return Err(ContractError::BundleItemUnfilled);
            }
            total_cost = total_cost
                .checked_add(outcome.total_price)
                .ok_or(ContractError::Overflow)?;
            results.push_back(MultiBuyResult {
                creator: leg.creator,
                amount: leg.amount,
                total_cost: outcome.total_price,
                new_supply: outcome.new_supply,
            });
        }

        env.events().publish(
            events::multi_buy_completed_topics(&buyer),
            events::MultiBuyCompletedEvent {
                buyer: buyer.clone(),
                leg_count: results.len(),
                total_cost,
                ledger: env.ledger().sequence(),
            },
        );
        Ok(results)
    }

    /// Read-only helper returning the worst-case XLM budget for a set of legs.
    pub fn get_multi_buy_budget(env: Env, legs: Vec<MultiBuyLeg>) -> Result<i128, ContractError> {
        let deadline = env.ledger().sequence();
        validate_legs(&legs, deadline, &env)
    }

    /// Read-only check that a creator's registered supply is available to the
    /// multi-buy view layer. Useful for clients validating legs off-chain.
    pub fn multi_buy_creator_supply(env: Env, creator: Address) -> Result<u32, ContractError> {
        Ok(read_registered_creator_profile(&env, &creator)?.supply)
    }
}
