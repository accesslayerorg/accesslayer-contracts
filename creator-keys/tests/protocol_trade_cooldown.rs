//! Integration tests for the protocol-wide per-wallet trade cooldown
//! (issue #1175).
//!
//! The protocol admin configures a cooldown window with `set_trade_cooldown`.
//! While the window is open, the same wallet cannot trade again — a buy is
//! rejected with `ContractError::CooldownActive`, a sell is rejected the same
//! way, and the number of seconds left in the window is reported both by
//! `get_trade_cooldown_remaining` and by the `TradeCooldownBlockedEvent`.
//!
//! Acceptance criteria verified:
//! - A second trade inside the window is rejected with `CooldownActive`
//! - A trade after the window elapsed succeeds
//! - `seconds_remaining` is reported accurately (view + event)
//! - The wallet's last-trade timestamp is updated by a successful trade
//! - `0` disables the cooldown
//! - The cooldown is per wallet, not global
//! - The cooldown follows the wallet across creators
//! - Only the protocol admin can configure the window
//! - A window above `MAX_TRADE_COOLDOWN_SECS` is rejected
//! - A multi-key purchase consumes a single window

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_ledger_sequence,
    set_test_timestamp, test_env_with_auths, DEFAULT_TEST_TIMESTAMP,
};
use creator_keys::bundle::BundleItem;
use creator_keys::events::{self, TRADE_COOLDOWN_BLOCKED_EVENT_NAME};
use creator_keys::protocol_trade_cooldown::TradeCooldownError;
use creator_keys::{ContractError, CreatorKeysContractClient, MAX_TRADE_COOLDOWN_SECS};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol, Vec,
};

const KEY_PRICE: i128 = 100;
/// Cooldown window used by most tests, in seconds.
const COOLDOWN: u64 = 600;
/// Generous ceiling for bundle items that are expected to succeed.
const LOOSE_MAX: i128 = 1_000_000_000;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Contract + admin + registered creator + key price.
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = set_key_price_for_tests(env, &client, KEY_PRICE);
    client.set_protocol_admin(&admin, &admin);
    let creator = register_test_creator(env, &client, "alice");
    set_test_timestamp(env, DEFAULT_TEST_TIMESTAMP);
    set_ledger_sequence(env, 100);
    (client, admin, creator)
}

/// Contract + admin + two creators, so a wallet can trade across creators.
fn setup_with_two_creators(
    env: &Env,
) -> (CreatorKeysContractClient<'_>, Address, Address, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = set_key_price_for_tests(env, &client, KEY_PRICE);
    client.set_protocol_admin(&admin, &admin);
    let creator_a = register_test_creator(env, &client, "alice");
    let creator_b = register_test_creator(env, &client, "bob");
    set_test_timestamp(env, DEFAULT_TEST_TIMESTAMP);
    set_ledger_sequence(env, 100);
    (client, admin, creator_a, creator_b)
}

/// Advance the ledger timestamp by `seconds`.
fn advance(env: &Env, seconds: u64) {
    set_test_timestamp(env, DEFAULT_TEST_TIMESTAMP + seconds);
}

/// Collect all `TradeCooldownBlockedEvent` payloads emitted so far.
fn blocked_events(env: &Env) -> soroban_sdk::Vec<events::TradeCooldownBlockedEvent> {
    let mut found = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == TRADE_COOLDOWN_BLOCKED_EVENT_NAME {
            found.push_back(data.into_val(env));
        }
    }
    found
}

// ── acceptance criteria ───────────────────────────────────────────────────────

/// AC: A second trade inside the window is rejected with `CooldownActive`.
#[test]
fn test_second_buy_within_cooldown_is_rejected() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    client.set_trade_cooldown(&admin, &COOLDOWN);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    advance(&env, COOLDOWN - 1);

    let result = client.try_buy_key(&creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::CooldownActive)),
        "buy inside the cooldown window must return CooldownActive"
    );
    // No state change from the rejected trade.
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
}

/// AC: A trade after the window elapsed succeeds.
#[test]
fn test_trade_after_cooldown_elapsed_succeeds() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    client.set_trade_cooldown(&admin, &COOLDOWN);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    advance(&env, COOLDOWN);

    let supply = client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(supply, 2, "buy at the cooldown boundary must succeed");
}

/// AC: A sell right after a buy is rejected — the round trip is what the
/// cooldown exists to prevent.
#[test]
fn test_sell_after_buy_within_cooldown_is_rejected() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    client.set_trade_cooldown(&admin, &COOLDOWN);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    let result = client.try_sell_key(&creator, &buyer, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::CooldownActive)),
        "sell inside the cooldown window must return CooldownActive"
    );
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
}

/// AC: `seconds_remaining` is reported accurately by the view and the event.
#[test]
fn test_remaining_seconds_are_reported_accurately() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    client.set_trade_cooldown(&admin, &COOLDOWN);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    // Nothing to wait for before the first trade.
    advance(&env, 100);
    assert_eq!(
        client.get_trade_cooldown_remaining(&buyer),
        COOLDOWN - 100,
        "the view must report the exact number of seconds left"
    );

    let _ = client.try_buy_key(&creator, &buyer, &KEY_PRICE, &None);

    let found = blocked_events(&env);
    assert_eq!(
        found.len(),
        1,
        "exactly one TradeCooldownBlocked event per rejection"
    );
    let ev = found.get(0).unwrap();
    assert_eq!(ev.wallet, buyer, "event.wallet must be the blocked wallet");
    assert_eq!(
        ev.seconds_remaining,
        COOLDOWN - 100,
        "event.seconds_remaining must match the view"
    );
    assert_eq!(
        ev.expires_at,
        DEFAULT_TEST_TIMESTAMP + COOLDOWN,
        "event.expires_at must be the end of the window"
    );
}

/// AC: The wallet's last-trade timestamp is updated by a successful trade.
#[test]
fn test_last_trade_timestamp_is_updated_on_trade() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    client.set_trade_cooldown(&admin, &COOLDOWN);

    let buyer = Address::generate(&env);
    assert_eq!(
        client.get_wallet_last_trade_timestamp(&buyer),
        0,
        "a wallet that never traded has no timestamp"
    );

    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(
        client.get_wallet_last_trade_timestamp(&buyer),
        DEFAULT_TEST_TIMESTAMP,
        "the timestamp must be the time of the last trade"
    );

    advance(&env, COOLDOWN);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(
        client.get_wallet_last_trade_timestamp(&buyer),
        DEFAULT_TEST_TIMESTAMP + COOLDOWN,
        "each successful trade refreshes the timestamp"
    );
}

/// AC: `0` disables the cooldown — back-to-back trades are allowed.
#[test]
fn test_zero_cooldown_disables_the_restriction() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    client.set_trade_cooldown(&admin, &0);
    assert_eq!(client.get_trade_cooldown(), 0);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    let supply = client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(supply, 2, "a zero window must allow back-to-back buys");
    assert_eq!(client.get_trade_cooldown_remaining(&buyer), 0);
}

/// AC: The cooldown is per wallet — one blocked wallet does not affect others.
#[test]
fn test_cooldown_is_independent_per_wallet() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    client.set_trade_cooldown(&admin, &COOLDOWN);

    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);

    client.buy_key(&creator, &buyer_a, &KEY_PRICE, &None);
    advance(&env, 10);

    // buyer_b never traded, so it is free to trade.
    let supply = client.buy_key(&creator, &buyer_b, &KEY_PRICE, &None);
    assert_eq!(supply, 2, "a wallet that never traded must not be blocked");

    // buyer_a is still inside its own window.
    assert_eq!(
        client.try_buy_key(&creator, &buyer_a, &KEY_PRICE, &None),
        Err(Ok(ContractError::CooldownActive))
    );
}

/// AC: The cooldown follows the wallet across creators.
#[test]
fn test_cooldown_follows_the_wallet_across_creators() {
    let env = test_env_with_auths();
    let (client, admin, creator_a, creator_b) = setup_with_two_creators(&env);
    client.set_trade_cooldown(&admin, &COOLDOWN);

    let buyer = Address::generate(&env);
    client.buy_key(&creator_a, &buyer, &KEY_PRICE, &None);

    // Trading a different creator's key is still the same wallet trading.
    assert_eq!(
        client.try_buy_key(&creator_b, &buyer, &KEY_PRICE, &None),
        Err(Ok(ContractError::CooldownActive)),
        "the window must apply to every creator's key"
    );
}

/// AC: Only the protocol admin can configure the window.
#[test]
fn test_non_admin_cannot_set_trade_cooldown() {
    let env = test_env_with_auths();
    let (client, _admin, _creator) = setup(&env);

    let impostor = Address::generate(&env);
    let result = client.try_set_trade_cooldown(&impostor, &COOLDOWN);
    assert_eq!(
        result,
        Err(Ok(TradeCooldownError::Unauthorized)),
        "a non-admin must not be able to set the cooldown"
    );
    assert_eq!(client.get_trade_cooldown(), 0);
}

/// AC: A window above `MAX_TRADE_COOLDOWN_SECS` is rejected.
#[test]
fn test_set_trade_cooldown_above_max_is_rejected() {
    let env = test_env_with_auths();
    let (client, admin, _creator) = setup(&env);

    // Exactly at the limit is allowed.
    client.set_trade_cooldown(&admin, &MAX_TRADE_COOLDOWN_SECS);
    assert_eq!(client.get_trade_cooldown(), MAX_TRADE_COOLDOWN_SECS);

    let result = client.try_set_trade_cooldown(&admin, &(MAX_TRADE_COOLDOWN_SECS + 1));
    assert_eq!(
        result,
        Err(Ok(TradeCooldownError::DurationTooLong)),
        "a window above the maximum must be rejected"
    );
    assert_eq!(
        client.get_trade_cooldown(),
        MAX_TRADE_COOLDOWN_SECS,
        "the rejected update must not change the stored window"
    );
}

/// AC: A multi-key purchase consumes a single window, not one per key.
#[test]
fn test_bundle_buy_consumes_a_single_window() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    client.set_trade_cooldown(&admin, &COOLDOWN);

    let buyer = Address::generate(&env);
    let items = Vec::from_array(
        &env,
        [BundleItem {
            key_id: creator.clone(),
            amount: 3,
            max_price: LOOSE_MAX,
        }],
    );
    client.bundle_buy(&buyer, &items);
    assert_eq!(client.get_key_balance(&creator, &buyer), 3);

    // The whole bundle consumed one window, so the next trade is blocked.
    assert_eq!(
        client.try_buy_key(&creator, &buyer, &KEY_PRICE, &None),
        Err(Ok(ContractError::CooldownActive)),
        "a bundle must consume a single cooldown window"
    );
    assert_eq!(
        client.get_trade_cooldown_remaining(&buyer),
        COOLDOWN,
        "the window must start at the bundle, not at the last key"
    );
}
