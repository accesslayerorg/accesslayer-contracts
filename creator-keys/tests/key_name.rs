//! Tests for the get_key_name read-only method.

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env, String};

#[test]
fn test_get_key_name_success() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = soroban_sdk::Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle, &None, &None, &None, &None, &None);

    let name = client.get_key_name(&creator);
    assert_eq!(name, handle);
}

#[test]
fn test_get_key_name_fails_if_not_registered() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = soroban_sdk::Address::generate(&env);

    let result = client.try_get_key_name(&creator);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}
