//! Tests for get_treasury_address read-only method (#29).

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_get_treasury_address_returns_none_when_unset() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    assert_eq!(client.get_treasury_address(), None);
}

#[test]
fn test_get_treasury_address_returns_configured_address() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.set_treasury_address(&admin, &treasury);

    let result = client.get_treasury_address();
    assert_eq!(result, Some(treasury));
}

#[test]
fn test_get_treasury_address_is_read_only() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.set_treasury_address(&admin, &treasury);

    // Multiple calls should return the same result without mutating state
    let r1 = client.get_treasury_address();
    let r2 = client.get_treasury_address();
    assert_eq!(r1, r2);
}

#[test]
fn test_get_treasury_address_updates_after_reconfiguration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    let treasury_a = Address::generate(&env);
    let treasury_b = Address::generate(&env);

    client.set_treasury_address(&admin, &treasury_a);
    assert_eq!(client.get_treasury_address(), Some(treasury_a));

    client.set_treasury_address(&admin, &treasury_b);
    assert_eq!(client.get_treasury_address(), Some(treasury_b));
}
