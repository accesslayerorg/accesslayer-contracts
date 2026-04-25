//! Tests for the initial `sell_key` contract flow.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_test_timestamp,
    test_env_with_auths, DEFAULT_TEST_TIMESTAMP,
};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, 100_i128);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

#[test]
fn test_sell_key_decrements_supply_and_balance() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let seller = Address::generate(&env);

    client.buy_key(&creator, &seller, &100_i128);
    client.buy_key(&creator, &seller, &100_i128);

    let new_supply = client.sell_key(&creator, &seller);

    assert_eq!(new_supply, 1);
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &seller), 1);
}

#[test]
fn test_sell_key_removes_holder_when_last_key_is_sold() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let seller = Address::generate(&env);

    client.buy_key(&creator, &seller, &100_i128);
    assert_eq!(client.get_creator_holder_count(&creator), 1);

    let new_supply = client.sell_key(&creator, &seller);

    assert_eq!(new_supply, 0);
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &seller), 0);
    assert_eq!(client.get_creator_holder_count(&creator), 0);
}

#[test]
fn test_sell_key_preserves_holder_count_when_seller_still_has_keys() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let seller = Address::generate(&env);

    client.buy_key(&creator, &seller, &100_i128);
    client.buy_key(&creator, &seller, &100_i128);
    assert_eq!(client.get_creator_holder_count(&creator), 1);

    let new_supply = client.sell_key(&creator, &seller);

    assert_eq!(new_supply, 1);
    assert_eq!(client.get_key_balance(&creator, &seller), 1);
    assert_eq!(client.get_creator_holder_count(&creator), 1);
}

#[test]
fn test_sell_key_fails_for_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let seller = Address::generate(&env);

    let result = client.try_sell_key(&creator, &seller);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_sell_key_fails_when_seller_has_no_keys() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let seller = Address::generate(&env);

    let result = client.try_sell_key(&creator, &seller);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}

#[test]
fn test_sell_full_exit_then_rebuy_updates_state() {
    let env = test_env_with_auths();
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP);
    let (client, creator) = setup(&env);
    let trader = Address::generate(&env);

    client.buy_key(&creator, &trader, &100_i128);
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &trader), 1);
    assert_eq!(client.get_creator_holder_count(&creator), 1);

    client.sell_key(&creator, &trader);
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &trader), 0);
    assert_eq!(client.get_creator_holder_count(&creator), 0);

    let supply_after_rebuy = client.buy_key(&creator, &trader, &100_i128);
    assert_eq!(supply_after_rebuy, 1);

    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &trader), 1);
    assert_eq!(client.get_creator_holder_count(&creator), 1);
}
