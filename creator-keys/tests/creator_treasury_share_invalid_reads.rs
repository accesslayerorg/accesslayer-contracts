//! Focused unit tests for invalid input paths of `get_creator_treasury_share`.
//!
//! `get_creator_treasury_share` delegates to `get_creator_fee_bps`, which requires
//! both a registered creator and a stored fee config.  Two failure conditions exist:
//!
//! 1. `ContractError::NotRegistered`   — creator was never registered.
//! 2. `ContractError::FeeConfigNotSet` — creator is registered but no fee config
//!    has been stored yet.

mod contract_test_env;

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env, String};

// ── NotRegistered ─────────────────────────────────────────────────────────────

#[test]
fn test_get_creator_treasury_share_fails_for_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let result = client.try_get_creator_treasury_share(&creator);

    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "get_creator_treasury_share must return NotRegistered when the creator has never registered"
    );
}

// ── FeeConfigNotSet ───────────────────────────────────────────────────────────

#[test]
fn test_get_creator_treasury_share_fails_when_fee_config_not_set() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = soroban_sdk::Address::generate(&env);

    // Register creator WITHOUT calling set_fee_config.
    client.register_creator(
        &creator,
        &String::from_str(&env, "alice"),
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let result = client.try_get_creator_treasury_share(&creator);
    assert_eq!(
        result,
        Err(Ok(ContractError::FeeConfigNotSet)),
        "get_creator_treasury_share must return FeeConfigNotSet when no fee config has been stored"
    );
}
