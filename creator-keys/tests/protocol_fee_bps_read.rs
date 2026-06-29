//! Tests for the `get_protocol_fee_bps` read-only method.
//!
//! Verifies that the method returns the stored protocol fee bps value,
//! does not mutate contract state, and handles error paths correctly.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env};

#[test]
fn test_get_protocol_fee_bps_returns_stored_value() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_fee_config(&admin, &9000, &1000);

    let bps = client.get_protocol_fee_bps();
    assert_eq!(bps, 1000, "must return the stored protocol fee bps");
}

#[test]
fn test_get_protocol_fee_bps_returns_updated_value() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_fee_config(&admin, &9000, &1000);
    assert_eq!(client.get_protocol_fee_bps(), 1000);

    client.set_fee_config(&admin, &8000, &2000);
    assert_eq!(
        client.get_protocol_fee_bps(),
        2000,
        "must reflect updated protocol fee bps"
    );
}

#[test]
fn test_get_protocol_fee_bps_fails_when_fee_config_not_set() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let result = client.try_get_protocol_fee_bps();
    assert_eq!(
        result,
        Err(Ok(ContractError::FeeConfigNotSet)),
        "must return FeeConfigNotSet when no fee config has been stored"
    );
}

#[test]
fn test_get_protocol_fee_bps_does_not_mutate_state() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_fee_config(&admin, &9500, &500);

    let first = client.get_protocol_fee_bps();
    let second = client.get_protocol_fee_bps();
    let third = client.get_protocol_fee_bps();

    assert_eq!(first, 500);
    assert_eq!(first, second, "repeated reads must return the same value");
    assert_eq!(second, third, "repeated reads must return the same value");
}

#[test]
fn test_get_protocol_fee_bps_persists_across_operations() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_fee_config(&admin, &8500, &1500);

    let creator = soroban_sdk::Address::generate(&env);
    let buyer = soroban_sdk::Address::generate(&env);
    client.register_creator(
        &creator,
        &soroban_sdk::String::from_str(&env, "alice"),
        &None,
        &None,
        &None,
    );
    client.set_key_price(&admin, &100);
    client.buy_key(&creator, &buyer, &100, &None);

    let bps = client.get_protocol_fee_bps();
    assert_eq!(
        bps, 1500,
        "protocol fee bps must persist after buy operation"
    );
}
