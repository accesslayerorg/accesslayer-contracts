//! Tests for is_creator_registered view method (#28) and duplicate registration rejection (#31).

use creator_keys::{
    ContractError, CreatorKeysContract, CreatorKeysContractClient, HANDLE_LEN_MAX, HANDLE_LEN_MIN,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

// ── is_creator_registered tests (#28) ───────────────────────────────────

#[test]
fn test_is_creator_registered_returns_false_for_unknown() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unknown = Address::generate(&env);
    assert!(!client.is_creator_registered(&unknown));
}

#[test]
fn test_is_creator_registered_returns_true_after_registration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "alice"));

    assert!(client.is_creator_registered(&creator));
}

#[test]
fn test_is_creator_registered_is_read_only() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "alice"));

    // Multiple calls should return the same result without mutating state
    let r1 = client.is_creator_registered(&creator);
    let r2 = client.is_creator_registered(&creator);
    let r3 = client.is_creator_registered(&creator);
    assert_eq!(r1, r2);
    assert_eq!(r2, r3);
}

#[test]
fn test_is_creator_registered_different_creators_independent() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.register_creator(&alice, &String::from_str(&env, "alice"));

    assert!(client.is_creator_registered(&alice));
    assert!(!client.is_creator_registered(&bob));
}

// ── Duplicate registration rejection tests (#31) ────────────────────────

#[test]
fn test_register_creator_duplicate_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle);
    // Second registration with the same address should fail with error
    let result = client.try_register_creator(&creator, &handle);
    assert_eq!(result, Err(Ok(ContractError::AlreadyRegistered)));
}

#[test]
fn test_register_creator_duplicate_different_handle_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"));
    // Re-registering with a different handle should still fail
    let result = client.try_register_creator(&creator, &String::from_str(&env, "alice_v2"));
    assert_eq!(result, Err(Ok(ContractError::AlreadyRegistered)));
}

#[test]
fn test_register_creator_different_addresses_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.register_creator(&alice, &String::from_str(&env, "alice"));
    client.register_creator(&bob, &String::from_str(&env, "bob"));

    assert!(client.is_creator_registered(&alice));
    assert!(client.is_creator_registered(&bob));
}

#[test]
fn test_register_creator_accepts_min_handle_length() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let min_handle = "a".repeat(HANDLE_LEN_MIN as usize);
    client.register_creator(&creator, &String::from_str(&env, &min_handle));

    assert!(client.is_creator_registered(&creator));
}

#[test]
fn test_register_creator_accepts_max_handle_length() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let max_handle = "a".repeat(HANDLE_LEN_MAX as usize);
    client.register_creator(&creator, &String::from_str(&env, &max_handle));

    assert!(client.is_creator_registered(&creator));
}

#[test]
fn test_register_creator_rejects_handle_shorter_than_min() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let short_handle = "a".repeat((HANDLE_LEN_MIN - 1) as usize);
    let result = client.try_register_creator(&creator, &String::from_str(&env, &short_handle));
    assert_eq!(result, Err(Ok(ContractError::HandleTooShort)));
}

#[test]
fn test_register_creator_rejects_handle_longer_than_max() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let long_handle = "a".repeat((HANDLE_LEN_MAX + 1) as usize);
    let result = client.try_register_creator(&creator, &String::from_str(&env, &long_handle));
    assert_eq!(result, Err(Ok(ContractError::HandleTooLong)));
}

#[test]
fn test_register_creator_rejects_invalid_characters_in_handle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let invalid_handle = String::from_str(&env, "Alice-01");
    let result = client.try_register_creator(&creator, &invalid_handle);
    assert_eq!(result, Err(Ok(ContractError::InvalidHandleCharacter)));
}
