//! Atomic bundle purchase of multiple creator keys (Issue #1057).
//!
//! * [`CreatorKeysContract::bundle_buy`] buys several keys in one transaction.
//!   Every item is executed through the same buy implementation as a
//!   stand-alone `buy_keys`, so all guards (pause, blacklist, whitelist,
//!   caps, circuit breaker, fees, events) behave identically. Any failing item
//!   returns an error, and a failed invocation reverts **all** state changes,
//!   so the bundle succeeds or reverts as a unit.
//! * [`CreatorKeysContract::get_bundle_quote`] is a read-only estimate of the
//!   same purchase.
//!
//! ### Slippage guard
//!
//! Each item carries a `max_price`: the ceiling on the total cost of that
//! item's `amount` keys. Because the bundle is atomic, one item exceeding its
//! ceiling reverts the entire bundle, so the per-item ceilings act as a single
//! guard over the whole bundle.

use crate::{
    compute_bonding_curve_price, constants, events, read_registered_creator_profile, AuctionConfig,
    ContractError, CreatorKeysContract, CreatorKeysContractArgs, CreatorKeysContractClient,
};
use soroban_sdk::{contractimpl, contracttype, Address, Env, Vec};

/// Maximum number of items accepted by a single bundle.
pub const MAX_BUNDLE_ITEMS: u32 = 10;

/// One line of a bundle: buy `amount` keys of `key_id`, paying at most
/// `max_price` in total for that line.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BundleItem {
    pub key_id: Address,
    pub amount: u32,
    pub max_price: i128,
}

/// Estimate for one bundle item.
///
/// `price_impact_bps` is the rise of the spot price caused by buying the item,
/// `(price_after - price_before) * 10_000 / price_before`, saturating at
/// `u32::MAX`. It is `0` while the key is still in its fixed-price auction.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BundleItemQuote {
    pub key_id: Address,
    pub amount: u32,
    pub estimated_cost: i128,
    pub price_impact_bps: u32,
}

/// Estimate for a whole bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BundleQuote {
    pub items: Vec<BundleItemQuote>,
    pub total_cost: i128,
}

/// Rejects bundles with more than [`MAX_BUNDLE_ITEMS`] items.
fn validate_bundle(items: &Vec<BundleItem>) -> Result<(), ContractError> {
    if items.len() > MAX_BUNDLE_ITEMS {
        return Err(ContractError::BundleSizeExceeded);
    }
    Ok(())
}

#[contractimpl]
impl CreatorKeysContract {
    /// Atomically buys every item in `items` for `buyer`.
    ///
    /// Succeeds only if **all** items succeed within their `max_price`;
    /// otherwise the call errors and no item's state change persists. Emits a
    /// single [`events::BundleExecutedEvent`] (in addition to the usual
    /// per-key buy events) and returns the same payload.
    ///
    /// # Errors
    ///
    /// - [`ContractError::BundleSizeExceeded`] — more than [`MAX_BUNDLE_ITEMS`] items.
    /// - [`ContractError::SlippageExceeded`] — an item costs more than its
    ///   `max_price`.
    /// - [`ContractError::BundleItemUnfilled`] — an item could not be filled in
    ///   full (e.g. supply or per-wallet cap).
    /// - Any error a stand-alone `buy_keys` would return for an item.
    pub fn bundle_buy(
        env: Env,
        buyer: Address,
        items: Vec<BundleItem>,
    ) -> Result<events::BundleExecutedEvent, ContractError> {
        buyer.require_auth();
        validate_bundle(&items)?;

        let mut breakdown: Vec<events::BundleItemBreakdown> = Vec::new(&env);
        let mut total_cost: i128 = 0;

        for item in items.iter() {
            // `payment` is unbounded on purpose: the item's ceiling is enforced
            // through `max_price`, so a price overrun surfaces as
            // `SlippageExceeded` rather than as a silent partial fill.
            let outcome = Self::buy_keys_core(
                env.clone(),
                item.key_id.clone(),
                buyer.clone(),
                item.amount,
                i128::MAX,
                Some(item.max_price),
                None,
                None,
            )?;

            if outcome.keys_bought != item.amount {
                return Err(ContractError::BundleItemUnfilled);
            }

            total_cost = total_cost
                .checked_add(outcome.total_price)
                .ok_or(ContractError::Overflow)?;
            breakdown.push_back(events::BundleItemBreakdown {
                key_id: item.key_id,
                amount: item.amount,
                price_paid: outcome.total_price,
            });
        }

        let executed = events::BundleExecutedEvent {
            buyer: buyer.clone(),
            items: breakdown,
            total_cost,
            ledger: env.ledger().sequence(),
        };
        env.events()
            .publish(events::bundle_executed_topics(&buyer), executed.clone());
        Ok(executed)
    }

    /// Read-only estimate of what [`Self::bundle_buy`] would charge.
    ///
    /// Each item is priced key by key along the same curve (or fixed auction
    /// price) that `bundle_buy` uses, so `estimated_cost` equals the
    /// `price_paid` a successful execution at the same ledger state reports.
    /// Pricing only: guards that can reject a buy (caps, cooldown, circuit
    /// breaker, whitelist) are not simulated.
    pub fn get_bundle_quote(
        env: Env,
        items: Vec<BundleItem>,
    ) -> Result<BundleQuote, ContractError> {
        validate_bundle(&items)?;

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        let mut quotes: Vec<BundleItemQuote> = Vec::new(&env);
        let mut total_cost: i128 = 0;

        for item in items.iter() {
            let profile = read_registered_creator_profile(&env, &item.key_id)?;
            let supply_before = profile.supply;

            let auction: Option<AuctionConfig> = env
                .storage()
                .persistent()
                .get(&constants::storage::auction_config(&item.key_id));
            let auction_price = auction
                .filter(|c| supply_before < c.auction_supply)
                .map(|c| c.auction_price);

            let mut cost: i128 = 0;
            let mut supply = supply_before;
            for _ in 0..item.amount {
                let price = match auction_price {
                    Some(p) => p,
                    None => compute_bonding_curve_price(&env, &item.key_id, base_price, supply)?,
                };
                cost = cost.checked_add(price).ok_or(ContractError::Overflow)?;
                supply = supply.checked_add(1).ok_or(ContractError::Overflow)?;
            }

            let price_impact_bps = if auction_price.is_some() {
                0
            } else {
                let before =
                    compute_bonding_curve_price(&env, &item.key_id, base_price, supply_before)?;
                let after = compute_bonding_curve_price(&env, &item.key_id, base_price, supply)?;
                if before > 0 && after > before {
                    let bps = ((after - before) as u128)
                        .checked_mul(10_000)
                        .ok_or(ContractError::Overflow)?
                        / (before as u128);
                    u32::try_from(bps).unwrap_or(u32::MAX)
                } else {
                    0
                }
            };

            total_cost = total_cost
                .checked_add(cost)
                .ok_or(ContractError::Overflow)?;
            quotes.push_back(BundleItemQuote {
                key_id: item.key_id,
                amount: item.amount,
                estimated_cost: cost,
                price_impact_bps,
            });
        }

        Ok(BundleQuote {
            items: quotes,
            total_cost,
        })
    }
}
