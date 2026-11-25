//! Subscription tier management with on-chain access checks (#1179).
//!
//! A key creator defines one or more access tiers. Each tier carries a periodic
//! fee, a period length in seconds, and a label. A wallet subscribes (and later
//! renews) by paying the tier fee for one period; the fee is credited to the
//! creator's earnings ledger, matching how trade fees are accounted for.
//!
//! `is_subscribed` is the access check external contracts can query. It is
//! evaluated against the ledger state on read, so an expired subscription stops
//! granting access without any revocation transaction.
//!
//! The functions here perform no authorization: the contract entry points in
//! `lib.rs` authenticate the creator or subscriber before calling them.

use crate::{credit_creator_fee_balance, read_registered_creator_profile, DataKey};
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Symbol};

/// Emitted when a creator defines a tier.
pub const TIER_CREATED_EVENT: Symbol = symbol_short!("tier_new");
/// Emitted when a wallet starts a subscription.
pub const SUBSCRIPTION_STARTED_EVENT: Symbol = symbol_short!("sub_start");
/// Emitted when an active subscription is extended by another period.
pub const SUBSCRIPTION_RENEWED_EVENT: Symbol = symbol_short!("sub_renew");

/// Errors returned by the subscription tier entry points.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SubscriptionTierError {
    /// The key has not been registered.
    NotRegistered = 1,
    /// The per-period fee is negative.
    InvalidFee = 2,
    /// The period length is zero seconds.
    InvalidPeriod = 3,
    /// A tier already exists for this `(key_id, tier_id)`.
    TierAlreadyExists = 4,
    /// No tier is configured for this `(key_id, tier_id)`.
    TierNotFound = 5,
    /// Arithmetic overflow while storing or extending an expiry.
    Overflow = 6,
}

/// Stored configuration of a creator's access tier.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SubscriptionTier {
    pub key_id: Address,
    pub tier_id: u32,
    pub fee_per_period: i128,
    pub period_seconds: u64,
    pub label: String,
}

/// A wallet's subscription to one tier of a key.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TierSubscription {
    pub key_id: Address,
    pub tier_id: u32,
    pub wallet: Address,
    /// Unix timestamp at which access stops being granted.
    pub expires_at: u64,
    /// Period length in effect when this subscription was last paid for.
    pub period_seconds: u64,
}

fn tier_storage_key(key_id: &Address, tier_id: u32) -> DataKey {
    DataKey::SubscriptionTier(key_id.clone(), tier_id)
}

fn subscription_storage_key(key_id: &Address, wallet: &Address, tier_id: u32) -> DataKey {
    DataKey::TierSubscription(key_id.clone(), wallet.clone(), tier_id)
}

/// Stores a tier definition for `key_id`.
pub fn create_tier(
    env: &Env,
    key_id: &Address,
    tier_id: u32,
    fee_per_period: i128,
    period_seconds: u64,
    label: String,
) -> Result<(), SubscriptionTierError> {
    read_registered_creator_profile(env, key_id)
        .map_err(|_| SubscriptionTierError::NotRegistered)?;

    if fee_per_period < 0 {
        return Err(SubscriptionTierError::InvalidFee);
    }
    if period_seconds == 0 {
        return Err(SubscriptionTierError::InvalidPeriod);
    }

    let key = tier_storage_key(key_id, tier_id);
    if env.storage().persistent().has(&key) {
        return Err(SubscriptionTierError::TierAlreadyExists);
    }

    let tier = SubscriptionTier {
        key_id: key_id.clone(),
        tier_id,
        fee_per_period,
        period_seconds,
        label,
    };
    env.storage().persistent().set(&key, &tier);
    crate::extend_key_ttl_to_full_window(env, &key);

    env.events().publish(
        (TIER_CREATED_EVENT, key_id.clone()),
        (tier_id, fee_per_period, period_seconds),
    );

    Ok(())
}

/// The tier configuration for `(key_id, tier_id)`, when defined.
pub fn get_tier(env: &Env, key_id: &Address, tier_id: u32) -> Option<SubscriptionTier> {
    env.storage()
        .persistent()
        .get(&tier_storage_key(key_id, tier_id))
}

/// Charges one period of `tier_id` to `wallet` and records or extends access.
///
/// Renewals extend from `max(now, current_expiry)`, so a wallet never loses the
/// remaining time it has already paid for. Returns the resulting expiry
/// timestamp.
pub fn subscribe(
    env: &Env,
    key_id: &Address,
    wallet: &Address,
    tier_id: u32,
) -> Result<u64, SubscriptionTierError> {
    read_registered_creator_profile(env, key_id)
        .map_err(|_| SubscriptionTierError::NotRegistered)?;

    let tier = get_tier(env, key_id, tier_id).ok_or(SubscriptionTierError::TierNotFound)?;

    let now = env.ledger().timestamp();
    let key = subscription_storage_key(key_id, wallet, tier_id);
    let existing: Option<TierSubscription> = env.storage().persistent().get(&key);
    let is_active = existing
        .as_ref()
        .map(|sub| now < sub.expires_at)
        .unwrap_or(false);

    let starts_from = existing
        .as_ref()
        .map(|sub| sub.expires_at.max(now))
        .unwrap_or(now);
    let expires_at = starts_from
        .checked_add(tier.period_seconds)
        .ok_or(SubscriptionTierError::Overflow)?;

    credit_creator_fee_balance(env, key_id, tier.fee_per_period)
        .map_err(|_| SubscriptionTierError::Overflow)?;

    let record = TierSubscription {
        key_id: key_id.clone(),
        tier_id,
        wallet: wallet.clone(),
        expires_at,
        period_seconds: tier.period_seconds,
    };
    env.storage().persistent().set(&key, &record);
    crate::extend_key_ttl_to_full_window(env, &key);

    let topic = if is_active {
        SUBSCRIPTION_RENEWED_EVENT
    } else {
        SUBSCRIPTION_STARTED_EVENT
    };
    env.events().publish(
        (topic, key_id.clone(), wallet.clone()),
        (tier_id, expires_at, tier.fee_per_period),
    );

    Ok(expires_at)
}

/// Whether `wallet` currently holds an unexpired subscription to `tier_id`.
pub fn is_subscribed(env: &Env, key_id: &Address, wallet: &Address, tier_id: u32) -> bool {
    match env
        .storage()
        .persistent()
        .get::<DataKey, TierSubscription>(&subscription_storage_key(key_id, wallet, tier_id))
    {
        Some(sub) => env.ledger().timestamp() < sub.expires_at,
        None => false,
    }
}

/// The stored subscription record for `(key_id, wallet, tier_id)`, if any.
pub fn get_tier_subscription(
    env: &Env,
    key_id: &Address,
    wallet: &Address,
    tier_id: u32,
) -> Option<TierSubscription> {
    env.storage()
        .persistent()
        .get(&subscription_storage_key(key_id, wallet, tier_id))
}
