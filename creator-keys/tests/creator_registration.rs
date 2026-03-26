//! Tests for is_creator_registered view method (#28) and duplicate registration rejection (#31).

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
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
#[should_panic(expected = "creator already registered")]
fn test_register_creator_duplicate_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle);
    // Second registration with the same address should panic
    client.register_creator(&creator, &handle);
}

#[test]
#[should_panic(expected = "creator already registered")]
fn test_register_creator_duplicate_different_handle_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"));
    // Re-registering with a different handle should still panic
    client.register_creator(&creator, &String::from_str(&env, "alice_v2"));
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
