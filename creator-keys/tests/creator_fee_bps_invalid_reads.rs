//! Focused unit tests for invalid input paths of `get_creator_fee_bps`.
//!
//! `get_creator_fee_bps` requires both a registered creator AND a stored fee
//! config.  Two independent failure conditions are covered:
//!
//! 1. `ContractError::NotRegistered`  — creator address was never registered.
//! 2. `ContractError::FeeConfigNotSet` — creator is registered but no fee config
//!    has been stored yet.

mod contract_test_env;

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env, String};

// ── NotRegistered ─────────────────────────────────────────────────────────────

#[test]
fn test_get_creator_fee_bps_fails_for_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let result = client.try_get_creator_fee_bps(&creator);

    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "get_creator_fee_bps must return NotRegistered when the creator has never registered"
    );
}

// ── FeeConfigNotSet ───────────────────────────────────────────────────────────

#[test]
fn test_get_creator_fee_bps_fails_when_fee_config_not_set() {
    // Creator is registered, but no set_fee_config call has been made.
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = soroban_sdk::Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"));

    let result = client.try_get_creator_fee_bps(&creator);
    assert_eq!(
        result,
        Err(Ok(ContractError::FeeConfigNotSet)),
        "get_creator_fee_bps must return FeeConfigNotSet when no fee config has been stored"
    );
}

// ── NotRegistered takes priority over missing fee config ──────────────────────

#[test]
fn test_get_creator_fee_bps_unregistered_errors_before_fee_config_check() {
    // Neither a creator profile nor a fee config exists.  The method must
    // short-circuit at the registration check, not the fee-config check.
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let result = client.try_get_creator_fee_bps(&creator);

    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "NotRegistered must be returned when neither creator nor fee config is present"
    );
}
