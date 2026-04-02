//! Tests for invalid balance lookups to ensure read-method edge cases are covered.

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
fn test_get_key_balance_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);
    let wallet = Address::generate(&env);

    // Should return 0, not panic or error
    assert_eq!(client.get_key_balance(&unregistered_creator, &wallet), 0);
}

#[test]
fn test_get_key_balance_creator_is_wallet() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, creator) = setup(&env);

    // Creator should have 0 balance of their own keys initially
    assert_eq!(client.get_key_balance(&creator, &creator), 0);

    // After buying one
    client.buy_key(&creator, &creator, &100_i128);
    assert_eq!(client.get_key_balance(&creator, &creator), 1);
}

#[test]
fn test_get_holder_key_count_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);
    let holder = Address::generate(&env);

    let view = client.get_holder_key_count(&unregistered_creator, &holder);
    assert_eq!(view.key_count, 0);
    assert!(!view.creator_exists);
    assert_eq!(view.creator, unregistered_creator);
    assert_eq!(view.holder, holder);
}

#[test]
fn test_get_total_key_supply_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);

    // Should return 0, avoiding panics
    assert_eq!(client.get_total_key_supply(&unregistered_creator), 0);
}

#[test]
fn test_get_creator_holder_count_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);

    // Should return 0, avoiding panics
    assert_eq!(client.get_creator_holder_count(&unregistered_creator), 0);
}

#[test]
fn test_get_creator_supply_unregistered_creator_fails() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);

    // This method returns a Result, so it should be Err(NotRegistered)
    let result = client.try_get_creator_supply(&unregistered_creator);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_get_creator_details_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);

    let details = client.get_creator_details(&unregistered_creator);
    assert_eq!(details.creator, unregistered_creator);
    assert_eq!(details.handle, String::from_str(&env, ""));
    assert_eq!(details.supply, 0);
    assert!(!details.is_registered);
}

#[test]
fn test_get_key_balance_with_contract_address() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100_i128);

    let creator = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "alice"));

    // Lookup balance where holder is the contract itself
    let contract_address = contract_id.clone();
    assert_eq!(client.get_key_balance(&creator, &contract_address), 0);

    // Buy a key for the contract address
    client.buy_key(&creator, &contract_address, &100_i128);
    assert_eq!(client.get_key_balance(&creator, &contract_address), 1);
}
