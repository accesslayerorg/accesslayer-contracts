//! Regression tests for buy quote stability across repeated calls.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::Vec;

#[test]
fn test_buy_quote_is_stable_across_multiple_calls() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Setup typical pricing and fee configuration
    let key_price = 1000_i128;
    let creator_bps = 9000;
    let protocol_bps = 1000;
    set_pricing_and_fees(&env, &client, key_price, creator_bps, protocol_bps);

    let creator = register_test_creator(&env, &client, "alice");

    // Verify no state changes by checking supply before and after
    let supply_before = client.get_creator_supply(&creator);

    // Capture initial quote
    let first_quote = client.get_buy_quote(&creator);

    // Perform multiple consecutive calls and verify they match the first result
    for i in 0..5 {
        let subsequent_quote = client.get_buy_quote(&creator);

        assert_eq!(
            first_quote, subsequent_quote,
            "Quote drift detected at iteration {}: expected {:?}, got {:?}",
            i, first_quote, subsequent_quote
        );
    }

    let supply_after = client.get_creator_supply(&creator);
    assert_eq!(
        supply_before, supply_after,
        "State changed during read-only quote calls: supply moved from {} to {}",
        supply_before, supply_after
    );
}

#[test]
fn test_buy_quote_stability_with_different_fee_configs() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");

    // Test cases for different fee configurations
    let configs = [
        (1000, 9500, 500),  // 5% protocol fee
        (5000, 5000, 5000), // 50% protocol fee (max cap)
        (2500, 10000, 0),   // 0% protocol fee
    ];

    for (price, c_bps, p_bps) in configs {
        set_pricing_and_fees(&env, &client, price, c_bps, p_bps);

        let supply_before = client.get_creator_supply(&creator);

        let mut quotes = Vec::new(&env);
        for _ in 0..3 {
            quotes.push_back(client.get_buy_quote(&creator));
        }

        let q1 = quotes.get(0).unwrap();
        let q2 = quotes.get(1).unwrap();
        let q3 = quotes.get(2).unwrap();

        assert_eq!(q1, q2);
        assert_eq!(q2, q3);

        let supply_after = client.get_creator_supply(&creator);
        assert_eq!(supply_before, supply_after);
    }
}
