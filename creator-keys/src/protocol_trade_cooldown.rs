//! #1175 — Protocol-wide per-wallet trade cooldown for the bonding curve.
//!
//! The protocol admin can configure a cooldown window, in seconds, that applies
//! to **every** bonding-curve buy and sell, no matter which creator's key is
//! being traded. A wallet that traded inside the window cannot trade again
//! until the window has elapsed, so a wallet cannot round-trip keys
//! (buy → sell → buy → sell) inside the window. This is the protocol-level
//! counterpart of the creator-scoped cooldown configured with `set_cooldown`
//! (issue #974): the creator-scoped cooldown only covers trades of that
//! creator's own key, while this one follows the wallet across every creator.
//!
//! # Storage
//!
//! - [`TradeCooldownDataKey::CooldownSecs`] — the configured window in
//!   seconds. `0` (or absent) disables the cooldown.
//! - [`TradeCooldownDataKey::LastTrade`] — per wallet, the Unix timestamp of
//!   the wallet's most recent successful trade.
//!
//! # Behaviour
//!
//! A trade is rejected with [`ContractError::CooldownActive`] while
//! `now - last_trade_timestamp < cooldown_secs`. The number of seconds left in
//! the window is reported by [`remaining_secs`] (and by
//! [`crate::CreatorKeysContract::get_trade_cooldown_remaining`]), and the
//! rejection also publishes a
//! [`crate::events::TradeCooldownBlockedEvent`] carrying the same value.
//!
//! The timestamp is written once the trade has settled, so a reverted trade
//! never consumes the wallet's cooldown window.
//!
//! # Scope
//!
//! One *logical* trade consumes one cooldown window. A multi-key purchase
//! (`buy_keys`, `bundle_buy`) or a batch (`batch_buy`, `batch_sell`) is a
//! single logical trade, so the check and the timestamp write happen once
//! around the whole operation rather than once per key.

use crate::events::{self, TradeCooldownBlockedEvent, TradeCooldownSetEvent};
use crate::{extend_key_ttl_to_full_window, ContractError, MAX_TRADE_COOLDOWN_SECS};
use soroban_sdk::{contracterror, contracttype, Address, Env};

/// Errors raised by the protocol-wide trade-cooldown entrypoints.
///
/// Kept separate from [`ContractError`] so the admin-facing flow has its own
/// stable, self-describing error codes, matching the other feature modules.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TradeCooldownError {
    /// The caller is not the protocol admin.
    Unauthorized = 1,
    /// The requested cooldown exceeds [`MAX_TRADE_COOLDOWN_SECS`].
    DurationTooLong = 2,
}

/// Storage keys for the protocol-wide trade cooldown (issue #1175).
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum TradeCooldownDataKey {
    /// The configured cooldown window in seconds (`0` disables the cooldown).
    CooldownSecs,
    /// (wallet) -> Unix timestamp of the wallet's most recent trade.
    LastTrade(Address),
}

/// Read-only: the configured protocol-wide cooldown window in seconds.
///
/// Returns `0` when no cooldown has been configured, which disables it.
pub fn cooldown_secs(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&TradeCooldownDataKey::CooldownSecs)
        .unwrap_or(0)
}

/// Read-only: the Unix timestamp of `wallet`'s most recent trade.
///
/// Returns `None` when the wallet has never traded since the cooldown was
/// introduced.
pub fn last_trade_timestamp(env: &Env, wallet: &Address) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&TradeCooldownDataKey::LastTrade(wallet.clone()))
}

/// Read-only: seconds left before `wallet` may trade again.
///
/// Returns `0` when the cooldown is disabled, when the wallet has never
/// traded, or when the window has already elapsed. The value is exact: it is
/// `cooldown_secs - (now - last_trade_timestamp)` while the window is open.
pub fn remaining_secs(env: &Env, wallet: &Address) -> u64 {
    let cooldown = cooldown_secs(env);
    if cooldown == 0 {
        return 0;
    }

    let last_trade = match last_trade_timestamp(env, wallet) {
        Some(timestamp) => timestamp,
        None => return 0,
    };

    let elapsed = env.ledger().timestamp().saturating_sub(last_trade);
    cooldown.saturating_sub(elapsed)
}

/// Rejects a trade by `wallet` while its cooldown window is still open.
///
/// The rejection publishes a [`TradeCooldownBlockedEvent`] carrying the exact
/// number of seconds left in the window before the trade is refused.
pub fn assert_trade_allowed(env: &Env, wallet: &Address) -> Result<(), ContractError> {
    let seconds_remaining = remaining_secs(env, wallet);
    if seconds_remaining == 0 {
        return Ok(());
    }

    let now = env.ledger().timestamp();
    env.events().publish(
        events::trade_cooldown_blocked_topics(wallet),
        TradeCooldownBlockedEvent {
            wallet: wallet.clone(),
            expires_at: now.saturating_add(seconds_remaining),
            seconds_remaining,
        },
    );

    Err(ContractError::CooldownActive)
}

/// Records `wallet`'s most recent trade timestamp.
///
/// Called once a trade has settled: the write is part of the same transaction
/// as the trade itself, so a reverted trade leaves the timestamp untouched.
pub fn record_trade(env: &Env, wallet: &Address) {
    let key = TradeCooldownDataKey::LastTrade(wallet.clone());
    env.storage()
        .persistent()
        .set(&key, &env.ledger().timestamp());
    extend_key_ttl_to_full_window(env, &key);
}

/// Sets the protocol-wide per-wallet trade cooldown window.
///
/// Only the protocol admin may call this. `seconds` must be in
/// `0..=MAX_TRADE_COOLDOWN_SECS`; a value of `0` disables the cooldown.
pub fn set_cooldown(env: &Env, admin: &Address, seconds: u64) -> Result<(), TradeCooldownError> {
    crate::assert_is_admin(env, admin).map_err(|_| TradeCooldownError::Unauthorized)?;
    if seconds > MAX_TRADE_COOLDOWN_SECS {
        return Err(TradeCooldownError::DurationTooLong);
    }

    env.storage()
        .persistent()
        .set(&TradeCooldownDataKey::CooldownSecs, &seconds);
    extend_key_ttl_to_full_window(env, &TradeCooldownDataKey::CooldownSecs);

    env.events().publish(
        events::trade_cooldown_set_topics(admin),
        TradeCooldownSetEvent {
            admin: admin.clone(),
            cooldown_secs: seconds,
            ledger: env.ledger().sequence(),
        },
    );

    Ok(())
}
