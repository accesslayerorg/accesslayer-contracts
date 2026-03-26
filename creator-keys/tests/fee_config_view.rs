//! Tests for the get_protocol_fee_view read-only method (#19).

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env};

#[test]
fn test_get_protocol_fee_view_unconfigured_returns_defaults() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let view = client.get_protocol_fee_view();
    assert!(!view.is_configured);
    assert_eq!(view.creator_bps, 0);
    assert_eq!(view.protocol_bps, 0);
}

#[test]
fn test_get_protocol_fee_view_returns_configured_values() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = soroban_sdk::Address::generate(&env);

    client.set_fee_config(&admin, &9000u32, &1000u32);

    let view = client.get_protocol_fee_view();
    assert!(view.is_configured);
    assert_eq!(view.creator_bps, 9000);
    assert_eq!(view.protocol_bps, 1000);
}

#[test]
fn test_get_protocol_fee_view_is_read_only() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = soroban_sdk::Address::generate(&env);

    client.set_fee_config(&admin, &8000u32, &2000u32);

    let v1 = client.get_protocol_fee_view();
    let v2 = client.get_protocol_fee_view();

    assert_eq!(v1.creator_bps, v2.creator_bps);
    assert_eq!(v1.protocol_bps, v2.protocol_bps);
    assert_eq!(v1.is_configured, v2.is_configured);
}

#[test]
fn test_get_protocol_fee_view_updates_after_reconfiguration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = soroban_sdk::Address::generate(&env);

    client.set_fee_config(&admin, &9000u32, &1000u32);
    let v1 = client.get_protocol_fee_view();
    assert_eq!(v1.protocol_bps, 1000);

    client.set_fee_config(&admin, &8000u32, &2000u32);
    let v2 = client.get_protocol_fee_view();
    assert_eq!(v2.protocol_bps, 2000);
    assert_eq!(v2.creator_bps, 8000);
}
