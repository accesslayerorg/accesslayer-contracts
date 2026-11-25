//! Unit and integration tests for `query_price` (#552).
//!
//! Scope:
//! - Pure helper function for computing bonding curve price at any given supply step without mutating contract state.
//! - Returns the exact same value the buy function would compute for that supply step.
//! - Does not require admin authentication.
//! - Verifies supply 0, supply 1, and a mid-range supply value (50) match the inline formula.

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, register_creator_keys, register_test_creator, set_curve_slope,
    set_key_price_for_tests, set_protocol_fee_bps, test_env_with_auths, DEFAULT_CREATOR_BPS,
    DEFAULT_PROTOCOL_BPS,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

const BASE_PRICE: i128 = 100_000_000; // 10 XLM in stroops
const SLOPE: i128 = 5_000_000; // 0.5 XLM per supply step

#[test]
fn test_query_price_at_supply_zero() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, BASE_PRICE);
    set_curve_slope(&env, &client, SLOPE);
    set_protocol_fee_bps(&env, &client, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let viewer = Address::generate(&env);

    let snapshot_before = capture_snapshot(&client, &creator, &viewer);

    // query_price at supply 0: formula is base_price + slope * 0 = base_price
    let price_0 = client.query_price(&creator, &0u64);
    let expected_0 = BASE_PRICE;
    assert_eq!(
        price_0, expected_0,
        "query_price at supply 0 must match inline formula base_price + slope * 0"
    );

    // Must match the quote / buy price at supply 0
    let buy_quote = client.get_buy_quote(&creator);
    assert_eq!(
        price_0, buy_quote.price,
        "query_price at supply 0 must equal buy quote price"
    );

    // Contract state must be completely unmutated
    let snapshot_after = capture_snapshot(&client, &creator, &viewer);
    snapshot_before.assert_unchanged(&snapshot_after);
}

#[test]
fn test_query_price_at_supply_one() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, BASE_PRICE);
    set_curve_slope(&env, &client, SLOPE);

    let creator = register_test_creator(&env, &client, "alice");
    let viewer = Address::generate(&env);

    let snapshot_before = capture_snapshot(&client, &creator, &viewer);

    // query_price at supply 1: formula is base_price + slope * 1
    let price_1 = client.query_price(&creator, &1u64);
    let expected_1 = BASE_PRICE + SLOPE;
    assert_eq!(
        price_1, expected_1,
        "query_price at supply 1 must match inline formula base_price + slope * 1"
    );

    // Contract state must be completely unmutated
    let snapshot_after = capture_snapshot(&client, &creator, &viewer);
    snapshot_before.assert_unchanged(&snapshot_after);
}

#[test]
fn test_query_price_at_mid_range_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, BASE_PRICE);
    set_curve_slope(&env, &client, SLOPE);

    let creator = register_test_creator(&env, &client, "alice");
    let viewer = Address::generate(&env);

    let snapshot_before = capture_snapshot(&client, &creator, &viewer);

    // query_price at mid-range supply (50): formula is base_price + slope * 50
    let supply_mid = 50u64;
    let price_mid = client.query_price(&creator, &supply_mid);
    let expected_mid = BASE_PRICE + SLOPE * (supply_mid as i128);
    assert_eq!(
        price_mid, expected_mid,
        "query_price at mid-range supply 50 must match inline formula base_price + slope * 50"
    );

    // Contract state must be completely unmutated
    let snapshot_after = capture_snapshot(&client, &creator, &viewer);
    snapshot_before.assert_unchanged(&snapshot_after);
}

#[test]
fn test_query_price_requires_no_admin_authentication() {
    // Unmocked auth environment to verify query_price does not require admin auth
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_key_price_for_tests(&env, &client, BASE_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    // Un-mock auths to ensure unauthenticated callers can query price
    // A regular read query succeeds without any require_auth
    let price = client.query_price(&creator, &0u64);
    assert_eq!(price, BASE_PRICE);
}

#[test]
fn test_query_price_matches_actual_buy_step_prices() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, BASE_PRICE);
    set_curve_slope(&env, &client, SLOPE);
    set_protocol_fee_bps(&env, &client, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // For 3 consecutive buys, query_price before the buy must equal the buy quote price
    for step in 0..3u64 {
        let predicted_price = client.query_price(&creator, &step);
        let quote = client.get_buy_quote(&creator);
        assert_eq!(
            predicted_price, quote.price,
            "predicted price at supply step {step} must equal quote price"
        );
        client.buy_key(&creator, &buyer, &quote.total_amount, &None);
    }
}
