//! Tests for the `get_creator_fee_bps` read-only method.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_get_creator_fee_bps_returns_configured_value() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"));
    client.set_fee_config(&admin, &9000u32, &1000u32);

    assert_eq!(client.get_creator_fee_bps(&creator), 9000);
}

#[test]
fn test_get_creator_fee_bps_is_read_only() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"));
    client.set_fee_config(&admin, &7500u32, &2500u32);

    let first = client.get_creator_fee_bps(&creator);
    let second = client.get_creator_fee_bps(&creator);

    assert_eq!(first, second);
}
