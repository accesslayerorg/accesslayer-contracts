//! Tests for `buy_key` creator validation and payment checks.

mod contract_test_env;

use contract_test_env::{
    compute_expected_buy_price, register_creator_keys, register_test_creator,
    set_key_price_for_tests, set_protocol_fee_bps, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_buy_key_unregistered_creator_fails() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let base_price = 100i128;
    set_key_price_for_tests(&env, &client, base_price);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    let expected_price = compute_expected_buy_price(0, base_price);

    let fee_view_before = client.get_protocol_fee_view();
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);

    let result = client.try_buy_key(&creator, &buyer, &expected_price);

    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);

    let fee_view_after = client.get_protocol_fee_view();
    assert_eq!(fee_view_after.creator_bps, fee_view_before.creator_bps);
    assert_eq!(fee_view_after.protocol_bps, fee_view_before.protocol_bps);
    assert_eq!(fee_view_after.is_configured, fee_view_before.is_configured);
}

#[test]
fn test_buy_key_insufficient_payment_fails() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let base_price = 100i128;
    set_key_price_for_tests(&env, &client, base_price);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let expected_price = compute_expected_buy_price(0, base_price);
    let result = client.try_buy_key(&creator, &buyer, &(expected_price - 1));
    assert_eq!(result, Err(Ok(ContractError::InsufficientPayment)));
}

#[test]
fn test_buy_key_sufficient_payment_succeeds() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let base_price = 100i128;
    set_key_price_for_tests(&env, &client, base_price);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let expected_price = compute_expected_buy_price(0, base_price);
    let supply = client.buy_key(&creator, &buyer, &expected_price);
    assert_eq!(supply, 1);

    let profile = client.get_creator(&creator);
    assert_eq!(profile.supply, 1);
}
