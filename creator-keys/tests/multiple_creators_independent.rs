//! Integration test: two distinct creators operate independently with no shared state.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_two_creators_have_independent_supplies() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");

    let buyer_a = soroban_sdk::Address::generate(&env);
    let buyer_b = soroban_sdk::Address::generate(&env);

    for _ in 0..3 {
        let quote = client.get_buy_quote(&creator_a);
        client.buy_key(&creator_a, &buyer_a, &quote.total_amount, &None);
    }

    let quote_b = client.get_buy_quote(&creator_b);
    client.buy_key(&creator_b, &buyer_b, &quote_b.total_amount, &None);

    assert_eq!(client.get_total_key_supply(&creator_a), 3);
    assert_eq!(client.get_total_key_supply(&creator_b), 1);
}

#[test]
fn test_holder_balance_a_does_not_affect_holder_balance_b() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");

    let buyer_a = soroban_sdk::Address::generate(&env);

    for _ in 0..3 {
        let quote = client.get_buy_quote(&creator_a);
        client.buy_key(&creator_a, &buyer_a, &quote.total_amount, &None);
    }

    assert_eq!(client.get_key_balance(&creator_a, &buyer_a), 3);
    assert_eq!(client.get_key_balance(&creator_b, &buyer_a), 0);
}

#[test]
fn test_fee_bps_update_for_a_does_not_change_b() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");

    let fee_a_before = client.get_creator_fee_config(&creator_a);
    let fee_b_before = client.get_creator_fee_config(&creator_b);

    assert_eq!(fee_a_before.creator_bps, 9_000);
    assert_eq!(fee_b_before.creator_bps, 9_000);

    client.set_fee_config(&admin, &8_000u32, &2_000u32);

    let fee_a_after = client.get_creator_fee_config(&creator_a);
    let fee_b_after = client.get_creator_fee_config(&creator_b);

    assert_eq!(fee_a_after.creator_bps, 8_000);
    assert_eq!(fee_b_after.creator_bps, 8_000);
}

#[test]
fn test_ttl_extension_for_a_does_not_affect_b() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");

    let buyer_a = soroban_sdk::Address::generate(&env);
    let buyer_b = soroban_sdk::Address::generate(&env);

    let quote_a = client.get_buy_quote(&creator_a);
    client.buy_key(&creator_a, &buyer_a, &quote_a.total_amount, &None);

    let quote_b = client.get_buy_quote(&creator_b);
    client.buy_key(&creator_b, &buyer_b, &quote_b.total_amount, &None);

    assert_eq!(client.get_total_key_supply(&creator_a), 1);
    assert_eq!(client.get_total_key_supply(&creator_b), 1);

    let quote_a2 = client.get_buy_quote(&creator_a);
    client.buy_key(&creator_a, &buyer_a, &quote_a2.total_amount, &None);

    assert_eq!(client.get_total_key_supply(&creator_a), 2);
    assert_eq!(client.get_total_key_supply(&creator_b), 1);
}
