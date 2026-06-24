//! Focused unit tests for invalid input paths of `get_key_name` and `get_key_symbol`.
//!
//! Both methods delegate to the same underlying profile lookup and both require a
//! registered creator.  The only invalid path for each is an unregistered address.

mod contract_test_env;

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env};

// ── get_key_name ─────────────────────────────────────────────────────────────

#[test]
fn test_get_key_name_fails_for_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let result = client.try_get_key_name(&creator);

    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "get_key_name must return NotRegistered for an address that was never registered"
    );
}

// ── get_key_symbol ────────────────────────────────────────────────────────────

#[test]
fn test_get_key_symbol_fails_for_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let result = client.try_get_key_symbol(&creator);

    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "get_key_symbol must return NotRegistered for an address that was never registered"
    );
}

// ── Both fail for the same unregistered address ───────────────────────────────

#[test]
fn test_get_key_name_and_symbol_both_fail_for_same_unregistered_address() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);

    assert_eq!(
        client.try_get_key_name(&creator),
        Err(Ok(ContractError::NotRegistered))
    );
    assert_eq!(
        client.try_get_key_symbol(&creator),
        Err(Ok(ContractError::NotRegistered))
    );
}
