//! Protocol fee calculation unit/integration tests across supply edge values (#557).
//!
//! Verifies that protocol fee is computed correctly at supply 0 (first key),
//! supply 1, and supply 1000, that calculations use the configured fee bps
//! dynamically from the test environment, and that rounding behavior is floor
//! (rounds down, not ceiling) for non-integer stroop results.

mod contract_test_env;

use contract_test_env::{
    compute_expected_protocol_fee, register_creator_keys, register_test_creator,
    set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::fee;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

/// Helper to advance key supply by purchasing `target` keys.
fn advance_supply_to(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
    target: u32,
) {
    let current = client.get_total_key_supply(creator);
    for _ in current..target {
        let quote = client.get_buy_quote(creator);
        client.buy_key(creator, buyer, &quote.total_amount, &None);
    }
}

/// Test protocol fee calculation at supply 0 (first key purchase quote & fee split).
#[test]
fn test_protocol_fee_at_supply_0() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let key_price = 1000_i128;
    let creator_bps = 9000_u32;
    let protocol_bps = 1000_u32;
    set_pricing_and_fees(&env, &client, key_price, creator_bps, protocol_bps);

    let creator = register_test_creator(&env, &client, "creator_supply_0");
    assert_eq!(client.get_total_key_supply(&creator), 0);

    let quote = client.get_buy_quote(&creator);

    // Fetch configured protocol fee bps directly from creator fee config view
    let fee_config = client.get_creator_fee_config(&creator);
    assert_eq!(fee_config.protocol_bps, protocol_bps);

    let expected_fee = compute_expected_protocol_fee(quote.price, fee_config.protocol_bps);
    assert_eq!(
        quote.protocol_fee, expected_fee,
        "Protocol fee at supply 0 must match computed fee using configured bps"
    );
}

/// Test protocol fee calculation at supply 1 (second key purchase quote & fee split).
#[test]
fn test_protocol_fee_at_supply_1() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let key_price = 1000_i128;
    let creator_bps = 8500_u32;
    let protocol_bps = 1500_u32;
    set_pricing_and_fees(&env, &client, key_price, creator_bps, protocol_bps);

    let creator = register_test_creator(&env, &client, "creator_supply_1");
    let buyer = Address::generate(&env);

    // Advance supply to 1
    advance_supply_to(&client, &creator, &buyer, 1);
    assert_eq!(client.get_total_key_supply(&creator), 1);

    let quote = client.get_buy_quote(&creator);

    let fee_config = client.get_creator_fee_config(&creator);
    assert_eq!(fee_config.protocol_bps, protocol_bps);

    let expected_fee = compute_expected_protocol_fee(quote.price, fee_config.protocol_bps);
    assert_eq!(
        quote.protocol_fee, expected_fee,
        "Protocol fee at supply 1 must match computed fee using configured bps"
    );
}

/// Test protocol fee calculation at large supply 1000.
#[test]
fn test_protocol_fee_at_supply_1000() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let key_price = 100_i128;
    let creator_bps = 9200_u32;
    let protocol_bps = 800_u32;
    set_pricing_and_fees(&env, &client, key_price, creator_bps, protocol_bps);

    let creator = register_test_creator(&env, &client, "creator_supply_1000");
    let buyer = Address::generate(&env);

    // Advance supply to 1000
    advance_supply_to(&client, &creator, &buyer, 1000);
    assert_eq!(client.get_total_key_supply(&creator), 1000);

    let quote = client.get_buy_quote(&creator);

    let fee_config = client.get_creator_fee_config(&creator);
    assert_eq!(fee_config.protocol_bps, protocol_bps);

    let expected_fee = compute_expected_protocol_fee(quote.price, fee_config.protocol_bps);
    assert_eq!(
        quote.protocol_fee, expected_fee,
        "Protocol fee at supply 1000 must match computed fee using configured bps"
    );
}

/// Test protocol fee calculation rounding behavior is floor (down, not ceiling/up)
/// when the result is not a whole number of stroops.
#[test]
fn test_protocol_fee_rounding_is_floor_not_ceiling() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Configure fee bps to 1000 (10.00%)
    let creator_bps = 9000_u32;
    let protocol_bps = 1000_u32;

    // 1. Price = 999: 999 * 1000 / 10000 = 99.9 stroops
    // Floor is 99, ceiling would be 100.
    let key_price = 999_i128;
    let admin = set_pricing_and_fees(&env, &client, key_price, creator_bps, protocol_bps);

    let creator1 = register_test_creator(&env, &client, "floor_test_999");
    let quote1 = client.get_buy_quote(&creator1);

    let fee_config1 = client.get_creator_fee_config(&creator1);
    let expected_floor_1 = (quote1.price * fee_config1.protocol_bps as i128) / 10_000;
    assert_eq!(expected_floor_1, 99);
    assert_eq!(
        quote1.protocol_fee, 99,
        "999 at 10% should round down to 99, not ceiling 100"
    );
    assert_ne!(
        quote1.protocol_fee, 100,
        "999 at 10% must not use ceiling rounding"
    );

    // 2. Price = 1: 1 * 1000 / 10000 = 0.1 stroops
    // Floor is 0, ceiling would be 1.
    let key_price_dust = 1_i128;
    client.set_key_price(&admin, &key_price_dust);

    let creator2 = register_test_creator(&env, &client, "floor_test_1");
    let quote2 = client.get_buy_quote(&creator2);

    let fee_config2 = client.get_creator_fee_config(&creator2);
    let expected_floor_2 = (quote2.price * fee_config2.protocol_bps as i128) / 10_000;
    assert_eq!(expected_floor_2, 0);
    assert_eq!(
        quote2.protocol_fee, 0,
        "1 at 10% should round down to 0, not ceiling 1"
    );
    assert_ne!(
        quote2.protocol_fee, 1,
        "1 at 10% must not use ceiling rounding"
    );

    // 3. Price = 1001: 1001 * 1000 / 10000 = 100.1 stroops
    // Floor is 100, ceiling would be 101.
    let key_price_1001 = 1001_i128;
    client.set_key_price(&admin, &key_price_1001);

    let creator3 = register_test_creator(&env, &client, "floor_test_1001");
    let quote3 = client.get_buy_quote(&creator3);

    let fee_config3 = client.get_creator_fee_config(&creator3);
    let expected_floor_3 = (quote3.price * fee_config3.protocol_bps as i128) / 10_000;
    assert_eq!(expected_floor_3, 100);
    assert_eq!(
        quote3.protocol_fee, 100,
        "1001 at 10% should round down to 100, not ceiling 101"
    );
    assert_ne!(
        quote3.protocol_fee, 101,
        "1001 at 10% must not use ceiling rounding"
    );
}

/// Verify rounding behavior in core `fee::apply_percentage_fee` direct helper across multiple bps configurations.
#[test]
fn test_apply_percentage_fee_uses_configured_bps_and_floors() {
    let test_cases = [
        // (amount, bps, expected_floor, ceiling_would_be)
        (999_i128, 1000_u32, 99_i128, 100_i128),
        (1_i128, 1000_u32, 0_i128, 1_i128),
        (1001_i128, 1000_u32, 100_i128, 101_i128),
        (777_i128, 500_u32, 38_i128, 39_i128), // 777 * 500 / 10000 = 38.85
        (1234_i128, 2500_u32, 308_i128, 309_i128), // 1234 * 2500 / 10000 = 308.5
    ];

    for (amount, bps, expected_floor, ceiling_val) in test_cases {
        let computed =
            fee::apply_percentage_fee(amount, bps).expect("fee computation should succeed");
        assert_eq!(
            computed, expected_floor,
            "apply_percentage_fee({}, {}) must equal floor {}",
            amount, bps, expected_floor
        );
        assert_ne!(
            computed, ceiling_val,
            "apply_percentage_fee({}, {}) must not equal ceiling {}",
            amount, bps, ceiling_val
        );
    }
}
