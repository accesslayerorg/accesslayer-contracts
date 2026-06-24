//! Focused unit tests for invalid input paths of `get_creator_fee_balance`.
//!
//! `get_creator_fee_balance` requires a registered creator and returns the
//! accrued fee balance (defaulting to `0` when no fees have been collected).
//! The single invalid path is an unregistered creator address.

mod contract_test_env;

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env};

// ── NotRegistered ─────────────────────────────────────────────────────────────

#[test]
fn test_get_creator_fee_balance_fails_for_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let result = client.try_get_creator_fee_balance(&creator);

    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "get_creator_fee_balance must return NotRegistered for an address that was never registered"
    );
}

// ── Two distinct unregistered addresses each fail independently ───────────────

#[test]
fn test_get_creator_fee_balance_fails_independently_for_two_unregistered_addresses() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let addr_a = soroban_sdk::Address::generate(&env);
    let addr_b = soroban_sdk::Address::generate(&env);

    assert_eq!(
        client.try_get_creator_fee_balance(&addr_a),
        Err(Ok(ContractError::NotRegistered))
    );
    assert_eq!(
        client.try_get_creator_fee_balance(&addr_b),
        Err(Ok(ContractError::NotRegistered))
    );
}
