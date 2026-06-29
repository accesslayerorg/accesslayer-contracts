//! Regression test for bonding curve preset pricing ordering (#446).
//!
//! A steeper bonding curve must always produce a higher buy price than a
//! flatter curve at the same supply level. This test locks in the ordering
//! so that a formula change cannot accidentally invert the relationship.
//!
//! Uses two different slope values to represent Linear (lower slope) vs
//! Quadratic-like (higher slope) pricing behavior.

mod contract_test_env;

use contract_test_env::{
    compute_expected_bonding_curve_price, register_creator_keys, register_test_creator,
    set_curve_slope, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

/// Lower slope representing Linear preset behavior.
const LINEAR_SLOPE: i128 = 10;

/// Higher slope representing steeper (Quadratic-like) curve behavior.
const QUADRATIC_SLOPE: i128 = 100;

/// Advance supply to exactly `target` by buying keys.
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

#[test]
fn test_steeper_curve_higher_at_supply_10() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let steeper_creator = register_test_creator(&env, &client, "steep");
    let flatter_creator = register_test_creator(&env, &client, "flat");

    set_curve_slope(&env, &client, QUADRATIC_SLOPE);
    // Note: curve slope is global, so we test sequentially
    // First test with steeper slope
    let buyer = Address::generate(&env);
    advance_supply_to(&client, &steeper_creator, &buyer, 10);
    let quote_steeper = client.get_buy_quote(&steeper_creator);

    // Change slope and test with flatter curve
    set_curve_slope(&env, &client, LINEAR_SLOPE);
    let buyer2 = Address::generate(&env);
    advance_supply_to(&client, &flatter_creator, &buyer2, 10);
    let quote_flatter = client.get_buy_quote(&flatter_creator);

    assert!(
        quote_steeper.price > quote_flatter.price,
        "steeper curve ({}) must exceed flatter curve ({}) at supply 10",
        quote_steeper.price,
        quote_flatter.price,
    );
}

#[test]
fn test_steeper_curve_higher_at_supply_100() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let steeper_creator = register_test_creator(&env, &client, "steep");
    let flatter_creator = register_test_creator(&env, &client, "flat");

    // Test with steeper slope
    set_curve_slope(&env, &client, QUADRATIC_SLOPE);
    let buyer = Address::generate(&env);
    advance_supply_to(&client, &steeper_creator, &buyer, 100);
    let quote_steeper = client.get_buy_quote(&steeper_creator);

    // Test with flatter slope
    set_curve_slope(&env, &client, LINEAR_SLOPE);
    let buyer2 = Address::generate(&env);
    advance_supply_to(&client, &flatter_creator, &buyer2, 100);
    let quote_flatter = client.get_buy_quote(&flatter_creator);

    assert!(
        quote_steeper.price > quote_flatter.price,
        "steeper curve ({}) must exceed flatter curve ({}) at supply 100",
        quote_steeper.price,
        quote_flatter.price,
    );
}

#[test]
fn test_steeper_curve_higher_at_supply_1000() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let steeper_creator = register_test_creator(&env, &client, "steep");
    let flatter_creator = register_test_creator(&env, &client, "flat");

    // Test with steeper slope
    set_curve_slope(&env, &client, QUADRATIC_SLOPE);
    let buyer = Address::generate(&env);
    advance_supply_to(&client, &steeper_creator, &buyer, 1000);
    let quote_steeper = client.get_buy_quote(&steeper_creator);

    // Test with flatter slope
    set_curve_slope(&env, &client, LINEAR_SLOPE);
    let buyer2 = Address::generate(&env);
    advance_supply_to(&client, &flatter_creator, &buyer2, 1000);
    let quote_flatter = client.get_buy_quote(&flatter_creator);

    assert!(
        quote_steeper.price > quote_flatter.price,
        "steeper curve ({}) must exceed flatter curve ({}) at supply 1000",
        quote_steeper.price,
        quote_flatter.price,
    );
}

#[test]
fn test_steeper_curve_higher_at_supply_1() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let steeper_creator = register_test_creator(&env, &client, "steep");
    let flatter_creator = register_test_creator(&env, &client, "flat");

    // Test with steeper slope
    set_curve_slope(&env, &client, QUADRATIC_SLOPE);
    let buyer = Address::generate(&env);
    advance_supply_to(&client, &steeper_creator, &buyer, 1);
    let quote_steeper = client.get_buy_quote(&steeper_creator);

    // Test with flatter slope
    set_curve_slope(&env, &client, LINEAR_SLOPE);
    let buyer2 = Address::generate(&env);
    advance_supply_to(&client, &flatter_creator, &buyer2, 1);
    let quote_flatter = client.get_buy_quote(&flatter_creator);

    assert!(
        quote_steeper.price > quote_flatter.price,
        "steeper curve ({}) must exceed flatter curve ({}) at supply 1",
        quote_steeper.price,
        quote_flatter.price,
    );
}

#[test]
fn test_steeper_curve_price_grows_faster_with_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "test");

    // Test with steeper slope
    set_curve_slope(&env, &client, QUADRATIC_SLOPE);
    let buyer = Address::generate(&env);

    // At supply 1
    advance_supply_to(&client, &creator, &buyer, 1);
    let price_at_1 = client.get_buy_quote(&creator).price;

    // At supply 10
    advance_supply_to(&client, &creator, &buyer, 10);
    let price_at_10 = client.get_buy_quote(&creator).price;

    // At supply 100
    advance_supply_to(&client, &creator, &buyer, 100);
    let price_at_100 = client.get_buy_quote(&creator).price;

    // Price should grow with supply
    assert!(
        price_at_10 > price_at_1,
        "price at 10 ({}) should exceed price at 1 ({})",
        price_at_10,
        price_at_1,
    );
    assert!(
        price_at_100 > price_at_10,
        "price at 100 ({}) should exceed price at 10 ({})",
        price_at_100,
        price_at_10,
    );

    // The price difference should be proportional to the slope
    let expected_price_at_1 = compute_expected_bonding_curve_price(QUADRATIC_SLOPE, KEY_PRICE, 1);
    let expected_price_at_10 = compute_expected_bonding_curve_price(QUADRATIC_SLOPE, KEY_PRICE, 10);
    let expected_price_at_100 =
        compute_expected_bonding_curve_price(QUADRATIC_SLOPE, KEY_PRICE, 100);

    assert_eq!(price_at_1, expected_price_at_1);
    assert_eq!(price_at_10, expected_price_at_10);
    assert_eq!(price_at_100, expected_price_at_100);
}
