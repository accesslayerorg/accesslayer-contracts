//! Integration tests for the time-weighted average price (TWAP) view (issue #802).
//!
//! Every buy and sell records a `(price, ledger)` snapshot into a per-creator
//! ring buffer capped at [`MAX_PRICE_SNAPSHOTS`] entries. `get_twap` returns
//! the simple average of the snapshots whose ledger falls inside the requested
//! window, falling back to the current spot price when fewer than 2 snapshots
//! are in the window. The ring buffer key's TTL is bumped on every read and
//! write so actively polled price history never expires.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_curve_slope, set_key_price_for_tests,
    set_ledger_sequence, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{constants, DataKey, PriceSnapshot, MAX_PRICE_SNAPSHOTS};
use soroban_sdk::{
    testutils::{storage::Persistent as _, Address as _, Ledger},
    Address, Env, Vec,
};

const KEY_PRICE: i128 = 1_000;
const CURVE_SLOPE: i128 = 100;

/// Extends the contract instance and code so far-future ledger advances do
/// not archive them mid-test.
fn extend_contract_lifetime(env: &Env, contract_id: &Address) {
    let horizon = creator_keys::CREATOR_TTL_LEDGERS;
    env.deployer()
        .extend_ttl(contract_id.clone(), horizon, horizon);
}

fn advance_ledger(env: &Env, ledgers: u32) {
    let mut ledger = env.ledger().get();
    ledger.sequence_number += ledgers;
    env.ledger().set(ledger);
}

fn snapshot_ttl(env: &Env, contract_id: &Address, creator: &Address) -> u32 {
    let key = constants::storage::price_snapshots(creator);
    env.as_contract(contract_id, || env.storage().persistent().get_ttl(&key))
}

fn stored_snapshots(env: &Env, contract_id: &Address, creator: &Address) -> Vec<PriceSnapshot> {
    let key = constants::storage::price_snapshots(creator);
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .get::<DataKey, Vec<PriceSnapshot>>(&key)
            .unwrap_or_else(|| Vec::new(env))
    })
}

/// Expected linear bonding-curve price at a given supply.
fn expected_price(supply: u32) -> i128 {
    KEY_PRICE + CURVE_SLOPE * i128::from(supply)
}

/// Sets the ledger sequence, buys one key from `buyer`, and returns the price paid.
fn buy_at_ledger(
    env: &Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
    ledger: u32,
) -> i128 {
    set_ledger_sequence(env, ledger);
    let quote = client.get_buy_quote(creator);
    client.buy_key(creator, buyer, &quote.total_amount, &None);
    quote.price
}

#[test]
fn test_twap_is_average_of_snapshots_within_window() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Three buys at distinct ledgers, each at a distinct price.
    buy_at_ledger(&env, &client, &creator, &buyer, 100);
    buy_at_ledger(&env, &client, &creator, &buyer, 200);
    buy_at_ledger(&env, &client, &creator, &buyer, 300);

    // Window [200, 300] covers only the second and third snapshots.
    let twap = client.get_twap(&creator, &150);
    let expected = (expected_price(1) + expected_price(2)) / 2;
    assert_eq!(
        twap, expected,
        "TWAP over the two in-window snapshots must be their average"
    );

    // Window covering all three ledgers averages all three snapshots.
    let full_twap = client.get_twap(&creator, &300);
    let full_expected = (expected_price(0) + expected_price(1) + expected_price(2)) / 3;
    assert_eq!(full_twap, full_expected);
}

#[test]
fn test_twap_ignores_snapshots_outside_window() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    buy_at_ledger(&env, &client, &creator, &buyer, 100);
    buy_at_ledger(&env, &client, &creator, &buyer, 500);

    // A window that excludes the ledger-100 snapshot must average only the
    // ledger-500 snapshot... but that is 1 snapshot, so spot price is returned.
    set_ledger_sequence(&env, 500);
    let twap = client.get_twap(&creator, &100); // window [400, 500]
    assert_eq!(
        twap,
        expected_price(2),
        "with a single in-window snapshot the spot price is returned"
    );

    // A wide window covering both snapshots averages them.
    let twap = client.get_twap(&creator, &500);
    assert_eq!(twap, (expected_price(0) + expected_price(1)) / 2);
}

#[test]
fn test_twap_returns_spot_price_when_fewer_than_two_snapshots() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);
    let creator = register_test_creator(&env, &client, "alice");

    // No trades at all: fewer than 2 snapshots, so the spot price (supply 0) is returned.
    assert_eq!(
        client.get_twap(&creator, &1_000),
        expected_price(0),
        "empty buffer falls back to the spot price"
    );

    // One buy: still fewer than 2 snapshots, spot price at supply 1.
    let buyer = Address::generate(&env);
    buy_at_ledger(&env, &client, &creator, &buyer, 100);
    assert_eq!(
        client.get_twap(&creator, &1_000),
        expected_price(1),
        "single snapshot falls back to the spot price"
    );
}

#[test]
fn test_sell_records_snapshot_for_twap() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);
    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    buy_at_ledger(&env, &client, &creator, &trader, 100);
    buy_at_ledger(&env, &client, &creator, &trader, 200);

    // Sell at ledger 300 — records a snapshot at the sell price (supply 1).
    set_ledger_sequence(&env, 300);
    client.sell_key(&creator, &trader, &None);

    // Window [200, 300] averages the ledger-200 buy snapshot and the sell snapshot.
    let twap = client.get_twap(&creator, &150);
    let expected = (expected_price(1) + expected_price(1)) / 2;
    assert_eq!(
        twap, expected,
        "buy and sell snapshots inside the window must be averaged together"
    );
}

#[test]
fn test_ring_buffer_capped_at_max_snapshots_oldest_overwritten_first() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    extend_contract_lifetime(&env, &contract_id);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Perform more than MAX_PRICE_SNAPSHOTS buys, each at a distinct ledger.
    let total_buys = MAX_PRICE_SNAPSHOTS + 5;
    for i in 0..total_buys {
        set_ledger_sequence(&env, 100 + i);
        client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    }

    let snapshots = stored_snapshots(&env, &contract_id, &creator);
    assert_eq!(
        snapshots.len(),
        MAX_PRICE_SNAPSHOTS,
        "ring buffer must hold at most MAX_PRICE_SNAPSHOTS entries"
    );

    // The oldest surviving snapshot is from buy #6 (105 - 100 + 1).
    let first = snapshots.get(0).unwrap();
    assert_eq!(
        first.ledger,
        100 + (total_buys - MAX_PRICE_SNAPSHOTS),
        "oldest snapshot must have been overwritten first"
    );

    // The newest snapshot is the last buy's ledger.
    let last = snapshots.get(snapshots.len() - 1).unwrap();
    assert_eq!(last.ledger, 100 + total_buys - 1);
}

#[test]
fn test_ttl_bumped_on_write_and_read() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    extend_contract_lifetime(&env, &contract_id);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // A buy records a snapshot and extends the buffer key to the full window.
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    let ttl_after_write = snapshot_ttl(&env, &contract_id, &creator);
    assert!(
        ttl_after_write >= creator_keys::TTL_MIN_EXTENSION_LEDGERS,
        "a snapshot write must extend the buffer TTL past the 30-day floor: ttl={ttl_after_write}"
    );

    // Drain the entry close to expiry so only the read's bump keeps it live.
    advance_ledger(&env, creator_keys::CREATOR_TTL_LEDGERS - 100);
    let ttl_before_read = snapshot_ttl(&env, &contract_id, &creator);
    assert!(
        ttl_before_read < creator_keys::TTL_MIN_EXTENSION_LEDGERS,
        "precondition: buffer entry is close to expiry"
    );

    // A read (get_twap) must bump the TTL back above the 30-day floor.
    let _ = client.get_twap(&creator, &1_000);
    let ttl_after_read = snapshot_ttl(&env, &contract_id, &creator);
    assert!(
        ttl_after_read >= creator_keys::TTL_MIN_EXTENSION_LEDGERS,
        "a get_twap read must bump the buffer TTL past the 30-day floor: \
         before={ttl_before_read} after={ttl_after_read}"
    );
}

#[test]
fn test_twap_never_panics_on_edge_inputs() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    extend_contract_lifetime(&env, &contract_id);

    // Unregistered creator, no key price set, no snapshots.
    let nobody = Address::generate(&env);
    let result = client.get_twap(&nobody, &1_000);
    assert_eq!(result, 0, "unregistered creator with no price yields 0");

    // Registered creator with a price but no trades.
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");
    let result = client.get_twap(&creator, &0);
    assert_eq!(
        result, KEY_PRICE,
        "zero-length window falls back to spot price"
    );
    let result = client.get_twap(&creator, &u32::MAX);
    assert_eq!(
        result, KEY_PRICE,
        "huge window with empty buffer falls back to spot price"
    );

    // After trades, extreme window sizes still never panic.
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    let result = client.get_twap(&creator, &u32::MAX);
    assert_eq!(
        result, KEY_PRICE,
        "one snapshot averaged with the spot fallback window"
    );
    let _ = client.get_twap(&creator, &0);

    // The storage layout matches the contract's own ring buffer invariants.
    let snapshots = stored_snapshots(&env, &contract_id, &creator);
    assert_eq!(snapshots.len(), 1);
}
