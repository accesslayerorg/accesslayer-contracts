//! Tests for get_protocol_state_version read-only method.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_get_protocol_state_version_returns_initial_value() {
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
fn test_protocol_state_version_increments_on_fee_config_update() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // Read initial version
    let version_before = client.get_protocol_state_version();
    assert_eq!(version_before, 1);

    // Update fee config
    client.set_fee_config(&admin, &9000u32, &1000u32);

    // Assert version incremented
    let version_after = client.get_protocol_state_version();
    assert_eq!(version_after, 2);
    assert!(version_after > version_before);

    // Update fee config again
    client.set_fee_config(&admin, &7500u32, &2500u32);

    // Assert version incremented again
    let version_after_second = client.get_protocol_state_version();
    assert_eq!(version_after_second, 3);
    assert!(version_after_second > version_after);
}

#[test]
fn test_protocol_state_version_monotonically_increasing() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    let mut previous_version = client.get_protocol_state_version();

    // Perform multiple config updates
    for i in 2..=5 {
        let creator_bps = 10000 - (i * 1000);
        let protocol_bps = i * 1000;
        client.set_fee_config(&admin, &creator_bps, &protocol_bps);

        let current_version = client.get_protocol_state_version();
        assert_eq!(current_version, i);
        assert!(current_version > previous_version);
        previous_version = current_version;
    }
}

#[test]
fn test_get_protocol_state_version_increments_only_on_config_updates() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Initial version
    let initial_version = client.get_protocol_state_version();
    assert_eq!(initial_version, 1);

    // Fee config update should increment version
    client.set_fee_config(&admin, &9000u32, &1000u32);
    assert_eq!(client.get_protocol_state_version(), 2);

    // Other state changes should not increment version
    client.set_key_price(&admin, &100i128);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.buy_key(&creator, &buyer, &100i128, &None);
    client.set_treasury_address(&admin, &Address::generate(&env));

    // Version should still be 2 (only incremented by fee config update)
    assert_eq!(client.get_protocol_state_version(), 2);
}
