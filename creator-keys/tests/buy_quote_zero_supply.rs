//! Tests for buy quote at zero supply.
//!
//! When total supply is zero, the buy price for the first key follows the bonding curve formula.
//! These tests verify the function returns the correct non-zero first-key price and does not
//! produce an unexpected zero or error.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};

/// At zero supply, buy quote returns the expected first-key price, not zero.
///
/// The first key's price is determined by the bonding curve formula and should always be
/// non-zero for a non-zero base price.
#[test]
fn test_buy_quote_zero_supply_returns_first_key_price() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let key_price = 1000_i128;
    set_pricing_and_fees(&env, &client, key_price, 9000, 1000);

    let creator = register_test_creator(&env, &client, "alice");

    // At zero supply, buy quote should return the first-key price
    let quote = client.get_buy_quote(&creator);

    assert_ne!(
        quote.price, 0,
        "first-key price should be non-zero for non-zero base price"
    );
    assert_eq!(
        quote.price, key_price,
        "first-key price should equal the configured base price"
    );
}

/// At zero supply with various base prices, buy quote returns non-zero expected prices.
#[test]
fn test_buy_quote_zero_supply_various_prices() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let test_prices = [1, 10, 100, 500, 1000, 10000];

    for (i, price) in test_prices.iter().enumerate() {
        set_pricing_and_fees(&env, &client, *price, 9000, 1000);
        let creator = register_test_creator(&env, &client, &format!("creator{}", i));

        let quote = client.get_buy_quote(&creator);

        assert_eq!(
            quote.price, *price,
            "first-key price should equal configured base price for supply=0"
        );
        assert_ne!(quote.price, 0, "first-key price should be non-zero");
    }
}

/// At zero supply, buy quote result is well-formed with correct fee breakdown.
#[test]
fn test_buy_quote_zero_supply_well_formed() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let key_price = 1000_i128;
    set_pricing_and_fees(&env, &client, key_price, 8000, 2000);

    let creator = register_test_creator(&env, &client, "bob");

    let quote = client.get_buy_quote(&creator);

    // Verify well-formed quote structure
    assert_eq!(quote.price, key_price, "price should equal base price");
    assert_eq!(
        quote.price,
        quote.creator_fee + quote.protocol_fee + quote.total_amount,
        "price should equal sum of all components"
    );

    // With 80/20 split on 1000: creator = 800, protocol = 200
    assert_eq!(
        quote.creator_fee, 800,
        "creator fee should be 80% of price at zero supply"
    );
    assert_eq!(
        quote.protocol_fee, 200,
        "protocol fee should be 20% of price at zero supply"
    );
}

/// Multiple zero-supply quote calls are consistent.
#[test]
fn test_buy_quote_zero_supply_consistent_across_calls() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let key_price = 500_i128;
    set_pricing_and_fees(&env, &client, key_price, 9000, 1000);

    let creator = register_test_creator(&env, &client, "charlie");

    // Fetch the quote multiple times
    let quote1 = client.get_buy_quote(&creator);
    let quote2 = client.get_buy_quote(&creator);
    let quote3 = client.get_buy_quote(&creator);

    assert_eq!(
        quote1.price, quote2.price,
        "quote price should be consistent"
    );
    assert_eq!(
        quote2.price, quote3.price,
        "quote price should be consistent"
    );
    assert_eq!(
        quote1.creator_fee, quote2.creator_fee,
        "creator fee should be consistent"
    );
    assert_eq!(
        quote1.protocol_fee, quote2.protocol_fee,
        "protocol fee should be consistent"
    );
}
