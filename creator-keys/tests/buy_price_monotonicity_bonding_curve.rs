//! Integration test verifying buy price strictly increases as creator supply
//! grows along a linear bonding curve (Issue #601).
//!
//! Configures a non-zero slope and performs five sequential buys, asserting
//! that each successive buy quote is strictly greater than the previous.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_curve_slope, set_pricing_and_fees,
    test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

const BASE_PRICE: i128 = 1000;
const CURVE_SLOPE: i128 = 10;
const NUM_BUYS: usize = 5;

#[test]
fn test_buy_price_strictly_increases_across_five_buys() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    let creator = register_test_creator(&env, &client, "alice");

    // Price at supply 0 must equal base_price + slope * 0 = BASE_PRICE.
    let q0 = client.get_buy_quote(&creator);
    assert_eq!(
        q0.price, BASE_PRICE,
        "price at supply 0 must equal BASE_PRICE"
    );

    let mut prices: Vec<i128> = Vec::new();

    for i in 0..NUM_BUYS {
        let buyer = soroban_sdk::Address::generate(&env);
        let quote = client.get_buy_quote(&creator);

        // Each successive quote must be strictly greater than the previous.
        if let Some(&prev) = prices.last() {
            assert!(
                quote.price > prev,
                "buy {} price ({}) must be strictly greater than buy {} price ({})",
                i + 1,
                quote.price,
                i,
                prev
            );
        }

        prices.push(quote.price);

        client.buy_key(&creator, &buyer, &quote.total_amount, &None);
    }

    // Verify we collected the expected number of prices.
    assert_eq!(prices.len(), NUM_BUYS);

    // Final supply must match the number of buys performed.
    let final_supply = client.get_total_key_supply(&creator);
    assert_eq!(
        final_supply, NUM_BUYS as u32,
        "supply must equal the number of buys performed"
    );
}

#[test]
fn test_prices_match_bonding_curve_formula() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    let creator = register_test_creator(&env, &client, "bob");

    let mut prices: Vec<i128> = Vec::new();

    for i in 0..NUM_BUYS {
        let buyer = soroban_sdk::Address::generate(&env);
        let quote = client.get_buy_quote(&creator);

        // Verify against the bonding curve formula: price = base_price + slope * supply
        let expected_price = BASE_PRICE + CURVE_SLOPE * i as i128;
        assert_eq!(
            quote.price, expected_price,
            "price at supply {} must match base_price + slope * supply",
            i
        );

        prices.push(quote.price);
        client.buy_key(&creator, &buyer, &quote.total_amount, &None);
    }

    // All prices must be in strictly ascending order.
    for window in prices.windows(2) {
        assert!(
            window[1] > window[0],
            "prices must be strictly ascending: got {} then {}",
            window[0],
            window[1]
        );
    }
}
