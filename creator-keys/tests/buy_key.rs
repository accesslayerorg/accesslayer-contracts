//! Tests for `buy_key` creator validation and payment checks.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_buy_key_unregistered_creator_fails() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    let result = client.try_buy_key(&creator, &buyer, &100i128);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_buy_key_insufficient_payment_fails() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let result = client.try_buy_key(&creator, &buyer, &99i128);
    assert_eq!(result, Err(Ok(ContractError::InsufficientPayment)));
}

#[test]
fn test_buy_key_sufficient_payment_succeeds() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let supply = client.buy_key(&creator, &buyer, &100i128);
    assert_eq!(supply, 1);

    let profile = client.get_creator(&creator);
    assert_eq!(profile.supply, 1);
}
