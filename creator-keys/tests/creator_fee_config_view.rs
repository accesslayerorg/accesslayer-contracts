//! Tests for the get_creator_fee_config read-only method.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env, String};

#[test]
fn test_get_creator_fee_config_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let view = client.get_creator_fee_config(&creator);

    assert!(!view.is_registered);
    assert!(!view.is_configured);
    assert_eq!(view.creator_bps, 0);
    assert_eq!(view.protocol_bps, 0);
}

#[test]
fn test_get_creator_fee_config_registered_no_fee_config() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let handle = String::from_str(&env, "test_creator");
    client.register_creator(&creator, &handle);

    let view = client.get_creator_fee_config(&creator);

    assert!(view.is_registered);
    assert!(!view.is_configured);
    assert_eq!(view.creator_bps, 0);
    assert_eq!(view.protocol_bps, 0);
}

#[test]
fn test_get_creator_fee_config_registered_with_fee_config() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let handle = String::from_str(&env, "test_creator");

    client.register_creator(&creator, &handle);
    client.set_fee_config(&admin, &9000u32, &1000u32);

    let view = client.get_creator_fee_config(&creator);

    assert!(view.is_registered);
    assert!(view.is_configured);
    assert_eq!(view.creator_bps, 9000);
    assert_eq!(view.protocol_bps, 1000);
}

#[test]
fn test_get_creator_fee_config_is_read_only() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let handle = String::from_str(&env, "test_creator");

    client.register_creator(&creator, &handle);
    client.set_fee_config(&admin, &8000u32, &2000u32);

    let v1 = client.get_creator_fee_config(&creator);
    let v2 = client.get_creator_fee_config(&creator);

    assert_eq!(v1.creator_bps, v2.creator_bps);
    assert_eq!(v1.protocol_bps, v2.protocol_bps);
    assert_eq!(v1.is_registered, v2.is_registered);
    assert_eq!(v1.is_configured, v2.is_configured);
}

#[test]
fn test_get_creator_fee_config_updates_after_fee_reconfiguration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let handle = String::from_str(&env, "test_creator");

    client.register_creator(&creator, &handle);
    client.set_fee_config(&admin, &9000u32, &1000u32);

    let v1 = client.get_creator_fee_config(&creator);
    assert_eq!(v1.protocol_bps, 1000);

    client.set_fee_config(&admin, &7000u32, &3000u32);

    let v2 = client.get_creator_fee_config(&creator);
    assert_eq!(v2.protocol_bps, 3000);
    assert_eq!(v2.creator_bps, 7000);
}

#[test]
fn test_get_creator_fee_config_multiple_creators_independent() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let creator1 = soroban_sdk::Address::generate(&env);
    let creator2 = soroban_sdk::Address::generate(&env);
    let handle1 = String::from_str(&env, "creator_one");
    let handle2 = String::from_str(&env, "creator_two");

    client.register_creator(&creator1, &handle1);
    client.register_creator(&creator2, &handle2);
    client.set_fee_config(&admin, &9000u32, &1000u32);

    let view1 = client.get_creator_fee_config(&creator1);
    let view2 = client.get_creator_fee_config(&creator2);

    assert!(view1.is_registered);
    assert!(view2.is_registered);
    assert_eq!(view1.creator_bps, view2.creator_bps);
    assert_eq!(view1.protocol_bps, view2.protocol_bps);
}

#[test]
fn test_get_creator_fee_config_unregistered_after_fee_config_set() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let unregistered_creator = soroban_sdk::Address::generate(&env);

    client.set_fee_config(&admin, &9000u32, &1000u32);

    let view = client.get_creator_fee_config(&unregistered_creator);

    assert!(!view.is_registered);
    assert!(!view.is_configured);
    assert_eq!(view.creator_bps, 0);
    assert_eq!(view.protocol_bps, 0);
}
