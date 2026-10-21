//! #1030 — On-chain price alerts for bonding-curve threshold crossings.
//!
//! A wallet registers a target price on a key together with an
//! [`AlertDirection`]. Every buy and sell compares the key's bonding-curve
//! spot price (the price of the next key, as returned by
//! `get_price_at_supply` for the current supply) before and after the trade.
//! An alert whose target the price crossed emits
//! [`events::PriceAlertTriggeredEvent`] and is deactivated in the same call, so
//! it never fires again, even if the price later crosses back and forth.
//!
//! Crossing rules, with `prev` / `new` the spot price before / after a trade:
//!
//! - [`AlertDirection::Above`] fires when `prev < target && new >= target`.
//! - [`AlertDirection::Below`] fires when `prev > target && new <= target`.
//!
//! An alert registered while the price already sits on the far side of its
//! target therefore waits for the price to come back and cross it.
//!
//! # Storage
//!
//! - [`PriceAlertDataKey::KeyAlerts`] holds every active alert on one key. It
//!   is the source of truth and the only entry a trade reads, so the cost a
//!   trade pays for alerts is one ledger entry, however many wallets watch the
//!   key.
//! - [`PriceAlertDataKey::WalletAlerts`] lists `(key_id, alert_id)` references
//!   for one wallet so [`get_alerts`] does not scan every key. Firing an alert
//!   only rewrites the key's entry: updating each triggered wallet's entry too
//!   would let alerts force an unbounded number of writes into somebody
//!   else's trade. References to fired alerts are therefore left behind,
//!   filtered out by [`get_alerts`], and pruned the next time the wallet
//!   registers or cancels an alert.
//! - [`PriceAlertDataKey::NextAlertId`] is the last issued alert id.
//!
//! All three are persistent entries extended to the full
//! [`crate::CREATOR_TTL_LEDGERS`] window when written. A key's alert entry is
//! also extended on every trade that reads it, like the rest of the creator
//! state a trade touches.
//!
//! # Limits
//!
//! - [`MAX_ACTIVE_ALERTS_PER_KEY`] bounds the entry a trade reads and the
//!   number of [`events::PriceAlertTriggeredEvent`]s one trade can emit.
//! - [`MAX_ACTIVE_ALERTS_PER_WALLET`] bounds the entries [`get_alerts`] reads.
//!
//! A wallet frees a slot when one of its alerts fires or when it calls
//! [`cancel_alert`].

use crate::events::{
    self, PriceAlertCancelledEvent, PriceAlertRegisteredEvent, PriceAlertTriggeredEvent,
};
use crate::{
    compute_bonding_curve_price, constants, extend_key_ttl_to_full_window,
    read_registered_creator_profile, ContractError,
};
use soroban_sdk::{contracttype, Address, Env, Vec};

/// Maximum number of active alerts on a single key.
///
/// Every buy and sell on the key reads all of them, and a single trade can
/// fire all of them at once, emitting one [`events::PriceAlertTriggeredEvent`]
/// each (roughly 0.5 KB of event data per alert). The cap keeps that worst
/// case well inside Soroban's per-transaction read and event-size limits, so
/// alerts can never make a trade fail.
pub const MAX_ACTIVE_ALERTS_PER_KEY: u32 = 16;

/// Maximum number of active alerts a single wallet can hold across all keys.
pub const MAX_ACTIVE_ALERTS_PER_WALLET: u32 = 10;

/// Which way the spot price must cross [`PriceAlert::target_price`] to fire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum AlertDirection {
    /// Fires when the price rises from below the target to at or above it.
    Above = 0,
    /// Fires when the price falls from above the target to at or below it.
    Below = 1,
}

/// An active price alert.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceAlert {
    /// Contract-wide unique id, assigned at registration and never reused.
    pub alert_id: u64,
    /// Wallet that registered the alert.
    pub wallet: Address,
    /// Key (creator address) whose price is watched.
    pub key_id: Address,
    /// Crossing direction that fires the alert.
    pub direction: AlertDirection,
    /// Spot price threshold, in the same units as the bonding-curve price.
    pub target_price: i128,
    /// Ledger sequence at which the alert was registered.
    pub created_ledger: u32,
}

/// Reference from a wallet to one of its alerts, stored in
/// [`PriceAlertDataKey::WalletAlerts`].
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceAlertRef {
    /// Key the alert watches; selects the [`PriceAlertDataKey::KeyAlerts`] entry.
    pub key_id: Address,
    /// Id of the alert inside that entry.
    pub alert_id: u64,
}

/// Storage keys for the price alert module.
#[derive(Clone)]
#[contracttype]
pub enum PriceAlertDataKey {
    /// Last issued alert id (`u64`); the first alert gets id `1`.
    NextAlertId,
    /// Active alerts on a key (`Vec<PriceAlert>`), in registration order.
    /// Removed once the last one fires or is cancelled.
    KeyAlerts(Address),
    /// A wallet's alert references (`Vec<PriceAlertRef>`). May include alerts
    /// that have already fired; see the module docs.
    WalletAlerts(Address),
}

fn read_key_alerts(env: &Env, key_id: &Address) -> Vec<PriceAlert> {
    env.storage()
        .persistent()
        .get(&PriceAlertDataKey::KeyAlerts(key_id.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Writes a key's active alerts, removing the entry when none remain so keys
/// without alerts keep no storage.
fn write_key_alerts(env: &Env, key_id: &Address, alerts: &Vec<PriceAlert>) {
    let key = PriceAlertDataKey::KeyAlerts(key_id.clone());
    if alerts.is_empty() {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, alerts);
        extend_key_ttl_to_full_window(env, &key);
    }
}

fn read_wallet_refs(env: &Env, wallet: &Address) -> Vec<PriceAlertRef> {
    env.storage()
        .persistent()
        .get(&PriceAlertDataKey::WalletAlerts(wallet.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

fn write_wallet_refs(env: &Env, wallet: &Address, refs: &Vec<PriceAlertRef>) {
    let key = PriceAlertDataKey::WalletAlerts(wallet.clone());
    if refs.is_empty() {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, refs);
        extend_key_ttl_to_full_window(env, &key);
    }
}

fn find_alert(alerts: &Vec<PriceAlert>, alert_id: u64) -> Option<u32> {
    alerts
        .iter()
        .position(|a| a.alert_id == alert_id)
        .map(|i| i as u32)
}

/// Resolves a wallet's references to its still-active alerts, in the order
/// they were registered. Each distinct key's entry is read once.
fn resolve_active_alerts(env: &Env, refs: &Vec<PriceAlertRef>) -> Vec<PriceAlert> {
    let mut active = Vec::new(env);
    let mut last_key: Option<Address> = None;
    let mut key_alerts = Vec::new(env);
    for r in refs.iter() {
        if last_key.as_ref() != Some(&r.key_id) {
            key_alerts = read_key_alerts(env, &r.key_id);
            last_key = Some(r.key_id.clone());
        }
        if let Some(i) = find_alert(&key_alerts, r.alert_id) {
            active.push_back(key_alerts.get_unchecked(i));
        }
    }
    active
}

/// References for the alerts in `active`, dropping those that have fired.
fn refs_for(env: &Env, active: &Vec<PriceAlert>) -> Vec<PriceAlertRef> {
    let mut refs = Vec::new(env);
    for a in active.iter() {
        refs.push_back(PriceAlertRef {
            key_id: a.key_id,
            alert_id: a.alert_id,
        });
    }
    refs
}

fn next_alert_id(env: &Env) -> Result<u64, ContractError> {
    let key = PriceAlertDataKey::NextAlertId;
    let next = env
        .storage()
        .persistent()
        .get::<_, u64>(&key)
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&key, &next);
    extend_key_ttl_to_full_window(env, &key);
    Ok(next)
}

/// Registers a price alert for `wallet` on `key_id` and returns its id.
///
/// See [`crate::CreatorKeysContract::register_alert`].
pub fn register_alert(
    env: &Env,
    key_id: &Address,
    wallet: &Address,
    direction: AlertDirection,
    target_price: i128,
) -> Result<u64, ContractError> {
    wallet.require_auth();
    read_registered_creator_profile(env, key_id)?;
    if target_price <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    // Prune references to alerts that have fired so they stop counting
    // against the wallet's cap.
    let mut wallet_alerts = resolve_active_alerts(env, &read_wallet_refs(env, wallet));
    if wallet_alerts.len() >= MAX_ACTIVE_ALERTS_PER_WALLET {
        return Err(ContractError::WalletAlertLimitReached);
    }

    let mut key_alerts = read_key_alerts(env, key_id);
    if key_alerts.len() >= MAX_ACTIVE_ALERTS_PER_KEY {
        return Err(ContractError::KeyAlertLimitReached);
    }

    let alert = PriceAlert {
        alert_id: next_alert_id(env)?,
        wallet: wallet.clone(),
        key_id: key_id.clone(),
        direction,
        target_price,
        created_ledger: env.ledger().sequence(),
    };
    key_alerts.push_back(alert.clone());
    write_key_alerts(env, key_id, &key_alerts);
    wallet_alerts.push_back(alert.clone());
    write_wallet_refs(env, wallet, &refs_for(env, &wallet_alerts));

    env.events().publish(
        events::price_alert_registered_topics(key_id, wallet),
        PriceAlertRegisteredEvent {
            wallet: wallet.clone(),
            key_id: key_id.clone(),
            alert_id: alert.alert_id,
            direction,
            target_price,
            ledger: alert.created_ledger,
        },
    );

    Ok(alert.alert_id)
}

/// Cancels one of `wallet`'s active alerts.
///
/// See [`crate::CreatorKeysContract::cancel_alert`].
pub fn cancel_alert(env: &Env, wallet: &Address, alert_id: u64) -> Result<(), ContractError> {
    wallet.require_auth();

    let mut wallet_alerts = resolve_active_alerts(env, &read_wallet_refs(env, wallet));
    let i = find_alert(&wallet_alerts, alert_id).ok_or(ContractError::AlertNotFound)?;
    let key_id = wallet_alerts.get_unchecked(i).key_id;
    wallet_alerts.remove(i);
    write_wallet_refs(env, wallet, &refs_for(env, &wallet_alerts));

    let mut key_alerts = read_key_alerts(env, &key_id);
    if let Some(k) = find_alert(&key_alerts, alert_id) {
        key_alerts.remove(k);
    }
    write_key_alerts(env, &key_id, &key_alerts);

    env.events().publish(
        events::price_alert_cancelled_topics(&key_id, wallet),
        PriceAlertCancelledEvent {
            wallet: wallet.clone(),
            key_id,
            alert_id,
            ledger: env.ledger().sequence(),
        },
    );

    Ok(())
}

/// Returns `wallet`'s active alerts in registration order. Alerts that have
/// fired or been cancelled are never included.
pub fn get_alerts(env: &Env, wallet: &Address) -> Vec<PriceAlert> {
    resolve_active_alerts(env, &read_wallet_refs(env, wallet))
}

fn crossed(direction: AlertDirection, target: i128, prev_price: i128, new_price: i128) -> bool {
    match direction {
        AlertDirection::Above => prev_price < target && new_price >= target,
        AlertDirection::Below => prev_price > target && new_price <= target,
    }
}

/// Fires and deactivates every alert on `key_id` whose target the spot price
/// crossed while a trade moved supply from `prev_supply` to `new_supply`.
///
/// Called once per key at the end of every buy and sell, after the trade's
/// own state and events are written. Never fails: alerts must not be able to
/// block a trade, so if a price cannot be computed no alert fires.
pub(crate) fn check_price_alerts(env: &Env, key_id: &Address, prev_supply: u32, new_supply: u32) {
    let key = PriceAlertDataKey::KeyAlerts(key_id.clone());
    let alerts: Vec<PriceAlert> = match env.storage().persistent().get(&key) {
        Some(alerts) => alerts,
        None => return,
    };
    // The trade read the entry, so keep it live like the rest of the key's state.
    extend_key_ttl_to_full_window(env, &key);

    if prev_supply == new_supply {
        return;
    }
    let base_price: i128 = match env
        .storage()
        .persistent()
        .get(&constants::storage::KEY_PRICE)
    {
        Some(price) => price,
        None => return,
    };
    let (prev_price, new_price) = match (
        compute_bonding_curve_price(env, key_id, base_price, prev_supply),
        compute_bonding_curve_price(env, key_id, base_price, new_supply),
    ) {
        (Ok(prev), Ok(new)) => (prev, new),
        _ => return,
    };
    if prev_price == new_price {
        return;
    }

    let mut remaining = Vec::new(env);
    for alert in alerts.iter() {
        if crossed(alert.direction, alert.target_price, prev_price, new_price) {
            env.events().publish(
                events::price_alert_triggered_topics(key_id, &alert.wallet),
                PriceAlertTriggeredEvent {
                    wallet: alert.wallet.clone(),
                    key_id: key_id.clone(),
                    alert_id: alert.alert_id,
                    direction: alert.direction,
                    target_price: alert.target_price,
                    new_price,
                    ledger: env.ledger().sequence(),
                },
            );
        } else {
            remaining.push_back(alert);
        }
    }
    if remaining.len() != alerts.len() {
        write_key_alerts(env, key_id, &remaining);
    }
}
