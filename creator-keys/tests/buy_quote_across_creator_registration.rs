//! Regression test: registering a new creator should not affect the buy price for an existing creator.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};

#[test]
fn test_buy_quote_unchanged_after_creator_registration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Setup typical pricing and fee configuration
    let key_price = 1000_i128;
    let creator_bps = 9000;
    let protocol_bps = 1000;
    set_pricing_and_fees(&env, &client, key_price, creator_bps, protocol_bps);

    // Register first creator and capture their buy quote
    let creator1 = register_test_creator(&env, &client, "alice");
    let quote_before = client.get_buy_quote(&creator1);

    // Register a second, unrelated creator
    let creator2 = register_test_creator(&env, &client, "bob");

    // Assert the buy quote for the first creator is unchanged
    let quote_after = client.get_buy_quote(&creator1);
    assert_eq!(
        quote_before, quote_after,
        "Buy quote for first creator changed after second creator registration: before={:?}, after={:?}",
        quote_before, quote_after
    );

    // Verify both creators are registered
    assert!(client.is_creator_registered(&creator1));
    assert!(client.is_creator_registered(&creator2));
}
