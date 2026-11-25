#![no_std]
#![cfg_attr(
    not(test),
    deny(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)
)]

use common::math::{checked_add, checked_mul_div, MathError, RoundingDirection};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Env, Symbol, Vec,
};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum KeyDividendError {
    NoDividendClaimable = 19,
    ZeroDistributionAmount = 20,
    NoKeyHolders = 21,
    AlreadyClaimed = 23,
    Overflow = 3,
    DeadlinePassed = 41,
    ProposalNotFound = 45,
    SnapshotNotFound = 71,
    TimelockNotElapsed = 74,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyDividend {
    pub creator: Address,
    pub token: Address,
    pub total_amount: i128,
    pub total_supply: i128,
    pub claimed: i128,
    pub snapshot_ledger: u32,
    pub expiry_ledger: u32,
    pub reclaimed: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum KeyDividendKey {
    NextId(Address),
    Dividend(Address, u32),
    Balance(Address, u32, Address),
    Claimed(Address, u32, Address),
}

fn emit<T, D>(env: &Env, name: &str, topic: T, data: D)
where
    T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    D: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
{
    env.events().publish((Symbol::new(env, name), topic), data);
}

#[contract]
pub struct KeyDividendContract;

#[contractimpl]
impl KeyDividendContract {
    /// Snapshots holder balances and escrows a fixed pool for pro-rata claims.
    pub fn declare_dividend(
        env: Env,
        creator: Address,
        key_id: Address,
        token: Address,
        total_amount: i128,
        holders: Vec<Address>,
        expiry_ledger: u32,
    ) -> Result<u32, KeyDividendError> {
        creator.require_auth();
        if total_amount <= 0 {
            return Err(KeyDividendError::ZeroDistributionAmount);
        }
        if expiry_ledger <= env.ledger().sequence() {
            return Err(KeyDividendError::DeadlinePassed);
        }

        let store = env.storage().persistent();
        let id = store
            .get::<_, u32>(&KeyDividendKey::NextId(key_id.clone()))
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(KeyDividendError::Overflow)?;

        let key = token::Client::new(&env, &key_id);
        let mut total_supply = 0i128;
        for holder in holders.iter() {
            let slot = KeyDividendKey::Balance(key_id.clone(), id, holder.clone());
            if store.has(&slot) {
                continue;
            }
            let balance = key.balance(&holder);
            if balance > 0 {
                store.set(&slot, &balance);
                total_supply =
                    checked_add(total_supply, balance).map_err(|_| KeyDividendError::Overflow)?;
            }
        }
        if total_supply == 0 {
            return Err(KeyDividendError::NoKeyHolders);
        }

        token::Client::new(&env, &token).transfer(
            &creator,
            &env.current_contract_address(),
            &total_amount,
        );

        store.set(&KeyDividendKey::NextId(key_id.clone()), &id);
        store.set(
            &KeyDividendKey::Dividend(key_id.clone(), id),
            &KeyDividend {
                creator,
                token,
                total_amount,
                total_supply,
                claimed: 0,
                snapshot_ledger: env.ledger().sequence(),
                expiry_ledger,
                reclaimed: false,
            },
        );
        emit(
            &env,
            "dividend_declared",
            (key_id, id),
            (total_amount, total_supply),
        );
        Ok(id)
    }

    /// Returns the unclaimed pro-rata share, or zero when no claim is available.
    pub fn get_dividend_claimable(
        env: Env,
        key_id: Address,
        dividend_id: u32,
        wallet: Address,
    ) -> Result<i128, KeyDividendError> {
        let Ok(dividend) = Self::get_dividend(env.clone(), key_id.clone(), dividend_id) else {
            return Ok(0);
        };
        let store = env.storage().persistent();
        if env.ledger().sequence() > dividend.expiry_ledger
            || store.has(&KeyDividendKey::Claimed(
                key_id.clone(),
                dividend_id,
                wallet.clone(),
            ))
        {
            return Ok(0);
        }
        let balance: i128 = store
            .get(&KeyDividendKey::Balance(key_id, dividend_id, wallet))
            .unwrap_or(0);
        checked_mul_div(
            &env,
            balance,
            dividend.total_amount,
            dividend.total_supply,
            RoundingDirection::Down,
        )
        .map_err(|error| match error {
            MathError::DivisionByZero | MathError::Overflow => KeyDividendError::Overflow,
        })
    }

    pub fn claim_dividend(
        env: Env,
        wallet: Address,
        key_id: Address,
        dividend_id: u32,
    ) -> Result<i128, KeyDividendError> {
        wallet.require_auth();
        let mut dividend = Self::get_dividend(env.clone(), key_id.clone(), dividend_id)?;
        let claimed_slot = KeyDividendKey::Claimed(key_id.clone(), dividend_id, wallet.clone());
        if env.storage().persistent().has(&claimed_slot) {
            return Err(KeyDividendError::AlreadyClaimed);
        }
        if env.ledger().sequence() > dividend.expiry_ledger {
            return Err(KeyDividendError::DeadlinePassed);
        }
        let share =
            Self::get_dividend_claimable(env.clone(), key_id.clone(), dividend_id, wallet.clone())?;
        if share == 0 {
            return Err(KeyDividendError::NoDividendClaimable);
        }

        dividend.claimed =
            checked_add(dividend.claimed, share).map_err(|_| KeyDividendError::Overflow)?;
        let store = env.storage().persistent();
        store.set(&claimed_slot, &true);
        store.set(
            &KeyDividendKey::Dividend(key_id.clone(), dividend_id),
            &dividend,
        );
        token::Client::new(&env, &dividend.token).transfer(
            &env.current_contract_address(),
            &wallet,
            &share,
        );
        emit(
            &env,
            "dividend_claimed",
            (key_id, dividend_id),
            (wallet, share),
        );
        Ok(share)
    }

    /// Returns unclaimed pool funds to the creator after expiry.
    pub fn reclaim_expired_dividend(
        env: Env,
        key_id: Address,
        dividend_id: u32,
    ) -> Result<i128, KeyDividendError> {
        let mut dividend = Self::get_dividend(env.clone(), key_id.clone(), dividend_id)?;
        if env.ledger().sequence() <= dividend.expiry_ledger {
            return Err(KeyDividendError::TimelockNotElapsed);
        }
        if dividend.reclaimed {
            return Err(KeyDividendError::AlreadyClaimed);
        }
        let amount = dividend
            .total_amount
            .checked_sub(dividend.claimed)
            .ok_or(KeyDividendError::Overflow)?;
        dividend.reclaimed = true;
        env.storage().persistent().set(
            &KeyDividendKey::Dividend(key_id.clone(), dividend_id),
            &dividend,
        );
        if amount > 0 {
            token::Client::new(&env, &dividend.token).transfer(
                &env.current_contract_address(),
                &dividend.creator,
                &amount,
            );
        }
        emit(
            &env,
            "dividend_reclaimed",
            (key_id, dividend_id),
            (dividend.creator, amount),
        );
        Ok(amount)
    }

    pub fn get_dividend(
        env: Env,
        key_id: Address,
        dividend_id: u32,
    ) -> Result<KeyDividend, KeyDividendError> {
        env.storage()
            .persistent()
            .get(&KeyDividendKey::Dividend(key_id, dividend_id))
            .ok_or(KeyDividendError::SnapshotNotFound)
    }
}
