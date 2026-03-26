//! Deterministic tests for fee split logic and contract integration.

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env};

#[test]
fn test_set_and_get_fee_config() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_fee_config(&admin, &9000u32, &1000u32);

    let config = client.get_fee_config();
    let config = config.unwrap();
    assert_eq!(config.creator_bps, 9000);
    assert_eq!(config.protocol_bps, 1000);
}

#[test]
fn test_compute_fees_for_payment() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = soroban_sdk::Address::generate(&env);

    client.set_fee_config(&admin, &9000u32, &1000u32);

    let (creator, protocol) = client.compute_fees_for_payment(&1000i128);
    assert_eq!(creator, 900);
    assert_eq!(protocol, 100);
    assert_eq!(creator + protocol, 1000);
}

#[test]
fn test_set_fee_config_invalid_sum_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = soroban_sdk::Address::generate(&env);

    let result = client.try_set_fee_config(&admin, &8000u32, &1000u32);
    assert_eq!(result, Err(Ok(ContractError::InvalidFeeConfig)));
}

#[test]
fn test_set_fee_config_max_protocol_bps_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = soroban_sdk::Address::generate(&env);

    client.set_fee_config(&admin, &5000u32, &5000u32);
    let config = client.get_fee_config().unwrap();
    assert_eq!(config.creator_bps, 5000);
    assert_eq!(config.protocol_bps, 5000);
}

#[test]
fn test_set_fee_config_protocol_bps_above_max_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = soroban_sdk::Address::generate(&env);

    let result = client.try_set_fee_config(&admin, &4999u32, &5001u32);
    assert_eq!(result, Err(Ok(ContractError::InvalidFeeConfig)));
}

#[test]
fn test_compute_fees_without_config_fails() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let result = client.try_compute_fees_for_payment(&1000i128);
    assert_eq!(result, Err(Ok(ContractError::FeeConfigNotSet)));
}
