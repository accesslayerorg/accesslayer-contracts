//! Tests for the `get_protocol_fee_bps` read-only method (#79).

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_get_protocol_fee_bps_returns_configured_value() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.set_fee_config(&admin, &9000u32, &1000u32);

    assert_eq!(client.get_protocol_fee_bps(), 1000);
}

#[test]
fn test_get_protocol_fee_bps_returns_zero_when_unconfigured() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    assert_eq!(client.get_protocol_fee_bps(), 0);
}

#[test]
fn test_get_protocol_fee_bps_is_read_only() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.set_fee_config(&admin, &8000u32, &2000u32);

    let first = client.get_protocol_fee_bps();
    let second = client.get_protocol_fee_bps();

    assert_eq!(first, second);
}
