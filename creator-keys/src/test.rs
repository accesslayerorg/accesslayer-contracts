use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_register_creator() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle);

    let profile = client.get_creator(&creator);
    assert_eq!(profile.handle, handle);
    assert_eq!(profile.creator, creator);
    assert_eq!(profile.supply, 0);
}

#[test]
fn test_duplicate_registration_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle);

    // Second registration should fail with AlreadyRegistered error
    let result = client.try_register_creator(&creator, &handle);
    assert_eq!(result, Err(Ok(ContractError::AlreadyRegistered)));
}

#[test]
fn test_buy_key_fails_if_not_registered() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    let result = client.try_buy_key(&creator, &buyer, &100);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_buy_key_success() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let buyer = Address::generate(&env);
    let supply = client.buy_key(&creator, &buyer, &100);
    assert_eq!(supply, 1);

    let profile = client.get_creator(&creator);
    assert_eq!(profile.supply, 1);
}

#[test]
fn test_get_creator_fails_if_not_registered() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);

    let result = client.try_get_creator(&creator);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_buy_key_insufficient_payment() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let buyer = Address::generate(&env);
    let result = client.try_buy_key(&creator, &buyer, &99);
    assert_eq!(result, Err(Ok(ContractError::InsufficientPayment)));
}

#[test]
fn test_set_key_price_invalid_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let result = client.try_set_key_price(&admin, &0);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));

    let result = client.try_set_key_price(&admin, &-1);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}
