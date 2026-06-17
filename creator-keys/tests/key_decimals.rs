//! Tests for get_key_decimals read-only method.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_get_key_decimals_returns_expected_value() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    assert_eq!(client.get_key_decimals(), 7);
}

#[test]
fn test_get_key_decimals_is_read_only() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let d1 = client.get_key_decimals();
    let d2 = client.get_key_decimals();
    assert_eq!(d1, d2);
    assert_eq!(d1, 7);
}

#[test]
fn test_get_key_decimals_stable_after_state_changes() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"), &None);
    client.buy_key(&creator, &buyer, &100i128);

    assert_eq!(client.get_key_decimals(), 7);
}
