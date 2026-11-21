//! #1238: recover tokens sent to the contract outside the normal flows.
//!
//! Every flow that makes the contract custody a real token records the amount
//! in a per-token, per-bucket ledger ([`TrackedBucket`]). The sum of those
//! buckets is the token's [`tracked_balance`]. Only the surplus, the contract's
//! live token balance minus the tracked balance, can ever be rescued.
//!
//! Rescues above the admin-configured per-token threshold must first be queued
//! with [`queue_rescue`] and can only run once the protocol timelock delay
//! (`get_timelock_delay`) has elapsed.

use crate::{
    assert_is_admin, constants, extend_key_ttl_to_full_window, DEFAULT_TIMELOCK_DELAY_SECS,
};
use soroban_sdk::{contracterror, contracttype, token::TokenClient, Address, Env, Symbol};

/// Errors raised by the token rescue entrypoints.
///
/// Kept separate from [`crate::ContractError`] because Soroban caps
/// `#[contracterror]` enums at 50 variants.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RescueError {
    /// The caller is not the protocol admin.
    Unauthorized = 1,
    /// The requested amount exceeds the token's untracked surplus.
    NotRescuable = 2,
    /// The amount or threshold was not positive.
    NotPositiveAmount = 3,
    /// Arithmetic overflow or underflow in the tracked-balance ledger.
    Overflow = 4,
    /// The amount is above the timelock threshold and no matching rescue is queued.
    TimelockRequired = 5,
    /// A matching rescue is queued but its timelock delay has not elapsed.
    TimelockNotElapsed = 6,
    /// A matching rescue is already queued.
    AlreadyQueued = 7,
    /// No matching rescue is queued.
    NotQueued = 8,
    /// The transfer would have left the contract holding less than its tracked balance.
    TrackedBalanceViolated = 9,
}

/// The kinds of custodied balances that are never rescuable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum TrackedBucket {
    Reserve,
    Fee,
    Escrow,
    Stake,
}

const ALL_BUCKETS: [TrackedBucket; 4] = [
    TrackedBucket::Reserve,
    TrackedBucket::Fee,
    TrackedBucket::Escrow,
    TrackedBucket::Stake,
];

#[derive(Clone)]
#[contracttype]
pub enum RescueKey {
    /// (token, bucket) -> tracked amount held for that bucket.
    Tracked(Address, TrackedBucket),
    /// token -> amount above which a rescue needs the timelock.
    Threshold(Address),
    /// (token, recipient, amount) -> earliest timestamp the rescue may run.
    Queued(Address, Address, i128),
}

/// Emitted with topic `("Rescued", token)` after a successful rescue.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RescuedEvent {
    pub token: Address,
    pub to: Address,
    pub amount: i128,
}

/// Emitted with topic `("RescueQueued", token)` when a large rescue is queued.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RescueQueuedEvent {
    pub token: Address,
    pub to: Address,
    pub amount: i128,
    pub executable_at: u64,
}

fn require_admin(env: &Env, admin: &Address) -> Result<(), RescueError> {
    admin.require_auth();
    assert_is_admin(env, admin).map_err(|_| RescueError::Unauthorized)
}

fn bucket_amount(env: &Env, token: &Address, bucket: TrackedBucket) -> i128 {
    env.storage()
        .persistent()
        .get(&RescueKey::Tracked(token.clone(), bucket))
        .unwrap_or(0)
}

fn save_bucket(env: &Env, token: &Address, bucket: TrackedBucket, amount: i128) {
    let key = RescueKey::Tracked(token.clone(), bucket);
    env.storage().persistent().set(&key, &amount);
    extend_key_ttl_to_full_window(env, &key);
}

/// Records `amount` of `token` entering custody under `bucket`.
pub(crate) fn track_credit(
    env: &Env,
    token: &Address,
    bucket: TrackedBucket,
    amount: i128,
) -> Result<(), RescueError> {
    let next = bucket_amount(env, token, bucket)
        .checked_add(amount)
        .ok_or(RescueError::Overflow)?;
    save_bucket(env, token, bucket, next);
    Ok(())
}

/// Records `amount` of `token` leaving custody from `bucket`.
pub(crate) fn track_debit(
    env: &Env,
    token: &Address,
    bucket: TrackedBucket,
    amount: i128,
) -> Result<(), RescueError> {
    let next = bucket_amount(env, token, bucket)
        .checked_sub(amount)
        .filter(|v| *v >= 0)
        .ok_or(RescueError::Overflow)?;
    save_bucket(env, token, bucket, next);
    Ok(())
}

/// Sum of reserves, fees, escrows and stakes the contract holds in `token`.
pub fn tracked_balance(env: &Env, token: &Address) -> Result<i128, RescueError> {
    ALL_BUCKETS.iter().try_fold(0i128, |acc, bucket| {
        acc.checked_add(bucket_amount(env, token, *bucket))
            .ok_or(RescueError::Overflow)
    })
}

/// Untracked surplus of `token` held by the contract; never negative.
pub fn rescuable(env: &Env, token: &Address) -> Result<i128, RescueError> {
    let balance = TokenClient::new(env, token).balance(&env.current_contract_address());
    let tracked = tracked_balance(env, token)?;
    Ok(balance.saturating_sub(tracked).max(0))
}

/// Amount above which a rescue of `token` needs the timelock; `None` if unset.
pub fn rescue_threshold(env: &Env, token: &Address) -> Option<i128> {
    env.storage()
        .persistent()
        .get(&RescueKey::Threshold(token.clone()))
}

pub fn set_rescue_threshold(
    env: &Env,
    admin: &Address,
    token: &Address,
    threshold: i128,
) -> Result<(), RescueError> {
    require_admin(env, admin)?;
    if threshold <= 0 {
        return Err(RescueError::NotPositiveAmount);
    }
    let key = RescueKey::Threshold(token.clone());
    env.storage().persistent().set(&key, &threshold);
    extend_key_ttl_to_full_window(env, &key);
    Ok(())
}

fn timelock_delay(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&constants::storage::TIMELOCK_DELAY_SECS)
        .unwrap_or(DEFAULT_TIMELOCK_DELAY_SECS)
}

fn needs_timelock(env: &Env, token: &Address, amount: i128) -> bool {
    rescue_threshold(env, token).is_some_and(|threshold| amount > threshold)
}

/// Queues a rescue and returns the timestamp from which it can execute.
///
/// The surplus is not checked here; it is checked again when the rescue runs,
/// so tokens that become tracked in the meantime stay protected.
pub fn queue_rescue(
    env: &Env,
    admin: &Address,
    token: &Address,
    to: &Address,
    amount: i128,
) -> Result<u64, RescueError> {
    require_admin(env, admin)?;
    if amount <= 0 {
        return Err(RescueError::NotPositiveAmount);
    }
    let key = RescueKey::Queued(token.clone(), to.clone(), amount);
    if env.storage().persistent().has(&key) {
        return Err(RescueError::AlreadyQueued);
    }
    let executable_at = env
        .ledger()
        .timestamp()
        .checked_add(timelock_delay(env))
        .ok_or(RescueError::Overflow)?;
    env.storage().persistent().set(&key, &executable_at);
    extend_key_ttl_to_full_window(env, &key);
    env.events().publish(
        (Symbol::new(env, "RescueQueued"), token.clone()),
        RescueQueuedEvent {
            token: token.clone(),
            to: to.clone(),
            amount,
            executable_at,
        },
    );
    Ok(executable_at)
}

pub fn cancel_rescue(
    env: &Env,
    admin: &Address,
    token: &Address,
    to: &Address,
    amount: i128,
) -> Result<(), RescueError> {
    require_admin(env, admin)?;
    let key = RescueKey::Queued(token.clone(), to.clone(), amount);
    if !env.storage().persistent().has(&key) {
        return Err(RescueError::NotQueued);
    }
    env.storage().persistent().remove(&key);
    Ok(())
}

/// Moves `amount` of untracked `token` surplus to `to`.
pub fn rescue(
    env: &Env,
    admin: &Address,
    token: &Address,
    to: &Address,
    amount: i128,
) -> Result<(), RescueError> {
    require_admin(env, admin)?;
    if amount <= 0 {
        return Err(RescueError::NotPositiveAmount);
    }

    if needs_timelock(env, token, amount) {
        let key = RescueKey::Queued(token.clone(), to.clone(), amount);
        let executable_at: u64 = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(RescueError::TimelockRequired)?;
        if env.ledger().timestamp() < executable_at {
            return Err(RescueError::TimelockNotElapsed);
        }
        env.storage().persistent().remove(&key);
    }

    if amount > rescuable(env, token)? {
        return Err(RescueError::NotRescuable);
    }

    let client = TokenClient::new(env, token);
    let this = env.current_contract_address();
    client.transfer(&this, to, &amount);

    // Re-check against the live balance so a token that moves more than asked
    // (or the protocol's own reserve token) can never end up below tracked totals.
    if client.balance(&this) < tracked_balance(env, token)? {
        return Err(RescueError::TrackedBalanceViolated);
    }

    env.events().publish(
        (Symbol::new(env, "Rescued"), token.clone()),
        RescuedEvent {
            token: token.clone(),
            to: to.clone(),
            amount,
        },
    );
    Ok(())
}
