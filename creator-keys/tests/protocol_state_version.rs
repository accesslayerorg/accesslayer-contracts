//! Tests for get_protocol_state_version read-only method.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_get_protocol_state_version_returns_expected_value() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    assert_eq!(client.get_protocol_state_version(), 1);
}

#[test]
fn test_get_protocol_state_version_is_read_only() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let v1 = client.get_protocol_state_version();
    let v2 = client.get_protocol_state_version();
    assert_eq!(v1, v2);
    assert_eq!(v1, 1);
}

#[test]
fn test_get_protocol_state_version_stable_after_state_changes() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"));
    client.buy_key(&creator, &buyer, &100i128);
    client.set_treasury_address(&admin, &Address::generate(&env));

    assert_eq!(client.get_protocol_state_version(), 1);
}
