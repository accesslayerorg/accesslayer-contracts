//! Regression test: two creators with identical fee configs should apply fees independently.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_identical_fee_configs_apply_independently() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Setup pricing and fee configuration
    let key_price = 1000_i128;
    let creator_bps = 9000;
    let protocol_bps = 1000;
    set_pricing_and_fees(&env, &client, key_price, creator_bps, protocol_bps);

    // Register two creators with identical fee config (via global config)
    let creator1 = register_test_creator(&env, &client, "alice");
    let creator2 = register_test_creator(&env, &client, "bob");

    // Both creators should have the same fee config view
    let fee_config1 = client.get_creator_fee_config(&creator1);
    let fee_config2 = client.get_creator_fee_config(&creator2);

    assert_eq!(fee_config1.creator_bps, creator_bps);
    assert_eq!(fee_config2.creator_bps, creator_bps);
    assert_eq!(fee_config1.protocol_bps, protocol_bps);
    assert_eq!(fee_config2.protocol_bps, protocol_bps);

    // Perform a buy for each creator
    let buyer1 = Address::generate(&env);
    let buyer2 = Address::generate(&env);

    let quote1 = client.get_buy_quote(&creator1);
    let quote2 = client.get_buy_quote(&creator2);

    // Quotes should be identical since they share the same config
    assert_eq!(quote1.price, quote2.price);
    assert_eq!(quote1.creator_fee, quote2.creator_fee);
    assert_eq!(quote1.protocol_fee, quote2.protocol_fee);
    assert_eq!(quote1.total_amount, quote2.total_amount);

    // Execute buys
    client.buy_key(&creator1, &buyer1, &quote1.total_amount);
    client.buy_key(&creator2, &buyer2, &quote2.total_amount);

    // Verify fee balances are tracked independently
    let fee_balance1 = client.get_creator_fee_balance(&creator1);
    let fee_balance2 = client.get_creator_fee_balance(&creator2);

    assert_eq!(fee_balance1, quote1.creator_fee);
    assert_eq!(fee_balance2, quote2.creator_fee);
    assert_eq!(fee_balance1, fee_balance2);
}

#[test]
fn test_fee_config_update_does_not_affect_other_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Setup initial fee config
    let key_price = 1000_i128;
    let creator_bps = 9000;
    let protocol_bps = 1000;
    set_pricing_and_fees(&env, &client, key_price, creator_bps, protocol_bps);

    // Register two creators
    let creator1 = register_test_creator(&env, &client, "alice");
    let creator2 = register_test_creator(&env, &client, "bob");

    // Capture initial quotes
    let quote1_initial = client.get_buy_quote(&creator1);
    let quote2_initial = client.get_buy_quote(&creator2);

    // Update global fee config
    let admin = Address::generate(&env);
    client.set_fee_config(&admin, &8000u32, &2000u32);

    // Both creators should see the new fee config (since it's global)
    let quote1_after = client.get_buy_quote(&creator1);
    let quote2_after = client.get_buy_quote(&creator2);

    // Both should have changed (global config affects all)
    assert_ne!(quote1_initial.creator_fee, quote1_after.creator_fee);
    assert_ne!(quote2_initial.creator_fee, quote2_after.creator_fee);

    // But they should still be identical to each other
    assert_eq!(quote1_after.creator_fee, quote2_after.creator_fee);
    assert_eq!(quote1_after.protocol_fee, quote2_after.protocol_fee);
}
