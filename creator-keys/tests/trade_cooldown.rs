//! Integration tests for the timestamp-based per-wallet trade cooldown (issue #974).
//!
//! `set_cooldown(key_id, duration_secs)` configures a per-creator cooldown that
//! blocks both buys and sells from the same wallet within `duration_secs` seconds
//! of their last trade. The entrypoint `get_cooldown_status` reflects the live
//! state, and blocked trades emit a `CooldownViolationEvent` containing
//! `seconds_remaining`.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_test_timestamp,
    test_env_with_auths, DEFAULT_TEST_TIMESTAMP,
};
use creator_keys::events::{self, COOLDOWN_VIOLATION_EVENT_NAME};
use creator_keys::{ContractError, CooldownError, CooldownStatus, MAX_TRADE_COOLDOWN_SECS};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 100;
const COOLDOWN_SECS: u64 = 60; // 1 minute

// ── helpers ──────────────────────────────────────────────────────────────────

struct Setup<'a> {
    client: creator_keys::CreatorKeysContractClient<'a>,
    creator: Address,
}

fn setup_with_cooldown(env: &Env, duration_secs: u64) -> Setup<'_> {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    set_test_timestamp(env, DEFAULT_TEST_TIMESTAMP);
    client.set_cooldown(&creator, &duration_secs);
    Setup { client, creator }
}

/// Collect all `CooldownViolationEvent` payloads from the most recent invocation.
fn violation_events(env: &Env) -> soroban_sdk::Vec<events::CooldownViolationEvent> {
    let mut found = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == COOLDOWN_VIOLATION_EVENT_NAME {
            found.push_back(data.into_val(env));
        }
    }
    found
}

// ── set_cooldown validation ───────────────────────────────────────────────────

/// AC: set_cooldown rejects duration above MAX_TRADE_COOLDOWN_SECS.
#[test]
fn test_set_cooldown_rejects_duration_above_max() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "bob");

    // Exactly at the limit is fine.
    client.set_cooldown(&creator, &MAX_TRADE_COOLDOWN_SECS);

    // One above must fail.
    let result = client.try_set_cooldown(&creator, &(MAX_TRADE_COOLDOWN_SECS + 1));
    assert_eq!(
        result,
        Err(Ok(CooldownError::DurationTooLong)),
        "duration above max must return DurationTooLong"
    );
}

/// AC: set_cooldown of 0 is accepted and disables the cooldown.
#[test]
fn test_set_cooldown_zero_disables() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, 0);

    let buyer = Address::generate(&env);
    // Two back-to-back buys must succeed with zero cooldown.
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(s.client.get_total_key_supply(&s.creator), 2);
}

// ── buy blocked within cooldown ───────────────────────────────────────────────

/// AC: Trade blocked correctly within cooldown window (buy path).
#[test]
fn test_buy_blocked_within_cooldown_window() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN_SECS);

    let buyer = Address::generate(&env);
    // First buy succeeds — no prior trade recorded.
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    // Advance time but stay inside the window.
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + COOLDOWN_SECS - 1);

    let result = s.client.try_buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::CooldownActive)),
        "buy within cooldown window must be rejected"
    );
    // Supply must be unchanged.
    assert_eq!(s.client.get_total_key_supply(&s.creator), 1);
}

/// AC: Trade succeeds after cooldown expires (buy path).
#[test]
fn test_buy_succeeds_after_cooldown_expires() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN_SECS);

    let buyer = Address::generate(&env);
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    // Advance time to exactly the expiry boundary — should succeed.
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + COOLDOWN_SECS);
    let supply = s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(supply, 2, "buy at cooldown boundary must succeed");
}

// ── sell blocked within cooldown ─────────────────────────────────────────────

/// AC: Sell is blocked when called within the cooldown window after a buy.
#[test]
fn test_sell_blocked_within_cooldown_window() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN_SECS);

    let trader = Address::generate(&env);
    // Buy at t=0 — stamps last_trade_timestamp.
    s.client.buy_key(&s.creator, &trader, &KEY_PRICE, &None);

    // Advance time but remain inside the cooldown window.
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + COOLDOWN_SECS - 1);

    // Sell within the cooldown must be rejected.
    let result = s.client.try_sell_key(&s.creator, &trader, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::CooldownActive)),
        "sell within cooldown window must be rejected"
    );
    // Balance unchanged — sell was rolled back.
    assert_eq!(s.client.get_key_balance(&s.creator, &trader), 1);
}

/// AC: Sell succeeds after the cooldown window expires.
#[test]
fn test_sell_succeeds_after_cooldown_expires() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN_SECS);

    let trader = Address::generate(&env);
    s.client.buy_key(&s.creator, &trader, &KEY_PRICE, &None);

    // Advance to exactly the expiry boundary.
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + COOLDOWN_SECS);
    let supply = s.client.sell_key(&s.creator, &trader, &None);
    assert_eq!(supply, 0, "sell at cooldown boundary must succeed");
}

// ── CooldownViolationEvent ────────────────────────────────────────────────────

/// AC: CooldownViolation event includes seconds_remaining when buy is blocked.
#[test]
fn test_violation_event_has_correct_seconds_remaining_on_buy() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN_SECS);

    let buyer = Address::generate(&env);
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    // Advance by 10 seconds → 50 seconds remaining.
    let elapsed: u64 = 10;
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + elapsed);

    let _ = s.client.try_buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    let evts = violation_events(&env);
    assert_eq!(evts.len(), 1, "exactly one violation event must be emitted");

    let ev = evts.get(0).unwrap();
    assert_eq!(ev.wallet, buyer);
    assert_eq!(ev.creator_id, s.creator);
    assert_eq!(
        ev.seconds_remaining,
        COOLDOWN_SECS - elapsed,
        "seconds_remaining must equal cooldown_secs minus elapsed"
    );
    assert_eq!(
        ev.expires_at,
        DEFAULT_TEST_TIMESTAMP + COOLDOWN_SECS,
        "expires_at must be last_trade_ts + duration_secs"
    );
}

// ── get_cooldown_status ───────────────────────────────────────────────────────

/// AC: get_cooldown_status returns active=true and correct expires_at inside window.
#[test]
fn test_get_cooldown_status_active_inside_window() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN_SECS);

    let trader = Address::generate(&env);
    s.client.buy_key(&s.creator, &trader, &KEY_PRICE, &None);

    // Still inside the window.
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + 5);

    let status: CooldownStatus = s.client.get_cooldown_status(&s.creator, &trader);
    assert!(status.active, "status must be active inside the window");
    assert_eq!(
        status.expires_at,
        DEFAULT_TEST_TIMESTAMP + COOLDOWN_SECS,
        "expires_at must be last_trade_ts + duration_secs"
    );
}

/// AC: get_cooldown_status returns active=false after cooldown expires.
#[test]
fn test_get_cooldown_status_inactive_after_expiry() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN_SECS);

    let trader = Address::generate(&env);
    s.client.buy_key(&s.creator, &trader, &KEY_PRICE, &None);

    // Past expiry.
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + COOLDOWN_SECS + 1);

    let status: CooldownStatus = s.client.get_cooldown_status(&s.creator, &trader);
    assert!(!status.active, "status must be inactive after expiry");
}

/// AC: get_cooldown_status returns active=false for a wallet that has never traded.
#[test]
fn test_get_cooldown_status_inactive_for_new_wallet() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN_SECS);

    let fresh_wallet = Address::generate(&env);
    let status: CooldownStatus = s.client.get_cooldown_status(&s.creator, &fresh_wallet);
    assert!(
        !status.active,
        "wallet with no trade history must not be in cooldown"
    );
    assert_eq!(status.expires_at, 0);
}

// ── independence guarantees ───────────────────────────────────────────────────

/// AC: Cooldown is per-wallet; blocking one wallet does not affect another.
#[test]
fn test_cooldown_independent_per_wallet() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN_SECS);

    let wallet_a = Address::generate(&env);
    let wallet_b = Address::generate(&env);

    s.client.buy_key(&s.creator, &wallet_a, &KEY_PRICE, &None);

    // wallet_b hasn't traded yet — advance into wallet_a's cooldown window.
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + 5);

    // wallet_a is blocked.
    assert_eq!(
        s.client
            .try_buy_key(&s.creator, &wallet_a, &KEY_PRICE, &None),
        Err(Ok(ContractError::CooldownActive))
    );

    // wallet_b has no prior trade — must succeed freely.
    let supply = s.client.buy_key(&s.creator, &wallet_b, &KEY_PRICE, &None);
    assert_eq!(supply, 2);
}

/// AC: Cooldown is per-creator; the same wallet can trade on a different creator's key.
#[test]
fn test_cooldown_independent_per_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP);

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");

    // Only creator_a has a cooldown.
    client.set_cooldown(&creator_a, &COOLDOWN_SECS);

    let buyer = Address::generate(&env);
    client.buy_key(&creator_a, &buyer, &KEY_PRICE, &None);
    client.buy_key(&creator_b, &buyer, &KEY_PRICE, &None);

    // Move into creator_a's cooldown window.
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + 5);

    // Blocked for creator_a.
    assert_eq!(
        client.try_buy_key(&creator_a, &buyer, &KEY_PRICE, &None),
        Err(Ok(ContractError::CooldownActive))
    );

    // Unrestricted for creator_b (no cooldown set).
    let supply_b = client.buy_key(&creator_b, &buyer, &KEY_PRICE, &None);
    assert_eq!(supply_b, 2);
}
