//! Integration and unit tests for `get_price` at zero supply and supply one (Issue #720).
//!
//! Scope & Acceptance Criteria:
//! - Test `get_price` at supply 0 returns the configured base price constant.
//! - Test `get_price` at supply 1 returns a value strictly greater than the base price (for non-zero slope).
//! - Assert no panic for either call.
//! - Test across multiple base prices, curve presets (Linear, Quadratic, Flat), and edge conditions.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_curve_slope, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient, CurvePreset};
use soroban_sdk::{testutils::Address as _, Address, String};

const BASE_PRICE: i128 = 1_000;
const CURVE_SLOPE: i128 = 50;

/// AC-1, AC-2, AC-3:
/// At supply 0, `get_price` returns the configured base price constant.
/// At supply 1, `get_price` returns a price strictly above base price.
/// Neither call panics.
#[test]
fn test_get_price_supply_zero_and_one_standard_linear_curve() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    let creator = register_test_creator(&env, &client, "alice");

    // Supply 0: must return base price and not panic
    let price_at_zero = client.get_price(&creator, &0u64);
    assert_eq!(
        price_at_zero, BASE_PRICE,
        "get_price at supply 0 must return the configured base price"
    );

    // Supply 1: must return price strictly greater than base price and not panic
    let price_at_one = client.get_price(&creator, &1u64);
    assert!(
        price_at_one > BASE_PRICE,
        "get_price at supply 1 ({}) must be strictly greater than base price ({})",
        price_at_one,
        BASE_PRICE
    );
    assert_eq!(
        price_at_one,
        BASE_PRICE + CURVE_SLOPE,
        "get_price at supply 1 must equal base_price + slope"
    );

    // Check try_get_price variants succeed without panic
    let try_zero = client.try_get_price(&creator, &0u64);
    assert_eq!(try_zero, Ok(Ok(BASE_PRICE)));

    let try_one = client.try_get_price(&creator, &1u64);
    assert_eq!(try_one, Ok(Ok(BASE_PRICE + CURVE_SLOPE)));
}

/// Tests that `get_price` at supply 0 returns the configured base price across a variety
/// of base price values, and supply 1 is strictly greater.
#[test]
fn test_get_price_various_base_prices_at_zero_and_one_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = set_pricing_and_fees(&env, &client, 1000, 9000, 1000);
    let slope = 25i128;
    set_curve_slope(&env, &client, slope);

    let test_prices = [1i128, 10, 100, 500, 1_000, 10_000, 100_000, 1_000_000];

    for (i, &price) in test_prices.iter().enumerate() {
        client.set_key_price(&admin, &price);
        let creator = register_test_creator(&env, &client, &format!("creator_{}", i));

        let price_0 = client.get_price(&creator, &0u64);
        assert_eq!(
            price_0, price,
            "get_price at supply 0 must return configured base price {}",
            price
        );

        let price_1 = client.get_price(&creator, &1u64);
        assert!(
            price_1 > price,
            "get_price at supply 1 ({}) must be strictly greater than base price ({})",
            price_1,
            price
        );
        assert_eq!(
            price_1,
            price + slope,
            "get_price at supply 1 must equal base_price + slope"
        );
    }
}

/// Tests `get_price` for Linear, Quadratic, and Flat curve presets at supply 0 and 1.
#[test]
fn test_get_price_curve_presets_supply_zero_and_one() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    // 1. Linear Preset (default)
    let linear_creator = register_test_creator(&env, &client, "linear_creator");
    let linear_0 = client.get_price(&linear_creator, &0u64);
    let linear_1 = client.get_price(&linear_creator, &1u64);
    assert_eq!(linear_0, BASE_PRICE);
    assert_eq!(linear_1, BASE_PRICE + CURVE_SLOPE);
    assert!(linear_1 > linear_0);

    // 2. Quadratic Preset
    let quad_creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: quad_creator.clone(),
            handle: String::from_str(&env, "quad_creator"),
        },
        &None,
        &None,
        &None,
        &Some(CurvePreset::Quadratic),
        &None,
        &None,
    );
    let quad_0 = client.get_price(&quad_creator, &0u64);
    let quad_1 = client.get_price(&quad_creator, &1u64);
    assert_eq!(quad_0, BASE_PRICE);
    assert_eq!(quad_1, BASE_PRICE + CURVE_SLOPE);
    assert!(quad_1 > quad_0);

    // 3. Flat Preset
    let flat_creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: flat_creator.clone(),
            handle: String::from_str(&env, "flat_creator"),
        },
        &None,
        &None,
        &None,
        &Some(CurvePreset::Flat),
        &None,
        &None,
    );
    let flat_0 = client.get_price(&flat_creator, &0u64);
    let flat_1 = client.get_price(&flat_creator, &1u64);
    assert_eq!(flat_0, BASE_PRICE);
    assert_eq!(flat_1, BASE_PRICE);
}

/// Tests that `get_price` is read-only and does not mutate total supply, balances, or state.
#[test]
fn test_get_price_read_only_does_not_mutate_state() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    let creator = register_test_creator(&env, &client, "state_check_creator");

    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(supply_before, 0);

    // Invoke get_price at supply 0 and 1 multiple times
    let _ = client.get_price(&creator, &0u64);
    let _ = client.get_price(&creator, &1u64);
    let _ = client.get_price(&creator, &0u64);

    let supply_after = client.get_total_key_supply(&creator);
    assert_eq!(
        supply_after, supply_before,
        "get_price must not mutate total supply"
    );
}

/// Tests that `get_price` matches `query_price` and `get_buy_quote` at supply 0.
#[test]
fn test_get_price_matches_query_price_and_buy_quote_at_supply_zero() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    let creator = register_test_creator(&env, &client, "match_check_creator");

    let get_price_val = client.get_price(&creator, &0u64);
    let query_price_val = client.query_price(&creator, &0u64);
    let buy_quote = client.get_buy_quote(&creator);

    assert_eq!(get_price_val, BASE_PRICE);
    assert_eq!(query_price_val, BASE_PRICE);
    assert_eq!(buy_quote.price, BASE_PRICE);
    assert_eq!(get_price_val, query_price_val);
}

/// Tests that `get_price` returns an error cleanly when base key price is not set, without panic.
#[test]
fn test_get_price_uninitialized_base_price_returns_error_without_panic() {
    let env = test_env_with_auths();
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &id);

    let creator = Address::generate(&env);

    let result_0 = client.try_get_price(&creator, &0u64);
    assert_eq!(
        result_0,
        Err(Ok(ContractError::KeyPriceNotSet)),
        "uninitialized key price must return KeyPriceNotSet error"
    );

    let result_1 = client.try_get_price(&creator, &1u64);
    assert_eq!(
        result_1,
        Err(Ok(ContractError::KeyPriceNotSet)),
        "uninitialized key price must return KeyPriceNotSet error"
    );
}

/// Tests that `get_price` remains callable and answers correctly during emergency pause.
#[test]
fn test_get_price_callable_during_emergency_pause() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    let creator = register_test_creator(&env, &client, "pause_creator");

    // Pause contract
    client.pause(&admin);
    assert!(client.get_is_paused());

    // Price reads must still answer without panic
    let price_0 = client.get_price(&creator, &0u64);
    let price_1 = client.get_price(&creator, &1u64);

    assert_eq!(price_0, BASE_PRICE);
    assert_eq!(price_1, BASE_PRICE + CURVE_SLOPE);
    assert!(price_1 > price_0);
}
