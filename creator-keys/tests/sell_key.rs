//! Tests for the initial `sell_key` contract flow.

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    client.set_key_price(&admin, &100_i128);

    let creator = Address::generate(env);
    client.register_creator(&creator, &String::from_str(env, "alice"));

    (client, admin, creator)
}

#[test]
fn test_sell_key_decrements_supply_and_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, creator) = setup(&env);
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
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, creator) = setup(&env);
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
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, creator) = setup(&env);
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
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let seller = Address::generate(&env);

    let result = client.try_sell_key(&creator, &seller);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_sell_key_fails_when_seller_has_no_keys() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, creator) = setup(&env);
    let seller = Address::generate(&env);

    let result = client.try_sell_key(&creator, &seller);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}
