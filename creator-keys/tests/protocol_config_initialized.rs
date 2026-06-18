//! Tests for the is_protocol_config_initialized read-only method.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_protocol_fee_bps, test_env_with_auths};
use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env};

#[test]
fn test_is_protocol_config_initialized_returns_false_before_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    // Before any initialization, the flag should be false
    assert!(!client.is_protocol_config_initialized());
}

#[test]
fn test_is_protocol_config_initialized_returns_true_after_fee_config_is_set() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = soroban_sdk::Address::generate(&env);

    client.set_fee_config(&admin, &9000u32, &1000u32);

    assert!(client.is_protocol_config_initialized());
}

#[test]
fn test_is_protocol_config_initialized_is_read_only() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = soroban_sdk::Address::generate(&env);

    client.set_fee_config(&admin, &8000u32, &2000u32);

    let first_read = client.is_protocol_config_initialized();
    let second_read = client.is_protocol_config_initialized();

    assert_eq!(first_read, second_read);
    assert!(first_read);
}

#[test]
fn test_protocol_config_state_is_unchanged_after_rejected_reinitialization() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);

    let result = client.try_set_fee_config(&admin, &8000u32, &1000u32);
    assert_eq!(result, Err(Ok(ContractError::InvalidFeeConfig)));

    let config = client.get_fee_config().unwrap();
    assert_eq!(config.creator_bps, 9000);
    assert_eq!(config.protocol_bps, 1000);
}
