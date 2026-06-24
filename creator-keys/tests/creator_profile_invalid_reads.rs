//! Focused unit tests for invalid input paths of `get_creator` (the full-profile read).
//!
//! `get_creator` returns `Result<CreatorProfile, ContractError>`. Unlike
//! `get_creator_details`, it errors instead of returning a default view.
//!
//! Invalid paths covered:
//! - Unregistered address → `ContractError::NotRegistered`
//! - Address registered then deregistered (sell-back to zero) still returns the profile
//!   because registration is permanent (supply can reach zero but the profile remains).

mod contract_test_env;

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env};

// ── NotRegistered on a fresh address ─────────────────────────────────────────

#[test]
fn test_get_creator_fails_for_unregistered_address() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let result = client.try_get_creator(&creator);

    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "get_creator must return NotRegistered for an address that was never registered"
    );
}

// ── Two distinct unregistered addresses each produce NotRegistered ────────────

#[test]
fn test_get_creator_fails_independently_for_two_unregistered_addresses() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let addr_a = soroban_sdk::Address::generate(&env);
    let addr_b = soroban_sdk::Address::generate(&env);

    assert_eq!(
        client.try_get_creator(&addr_a),
        Err(Ok(ContractError::NotRegistered))
    );
    assert_eq!(
        client.try_get_creator(&addr_b),
        Err(Ok(ContractError::NotRegistered))
    );
}

// ── Registration is permanent: profile survives a full sell-back ──────────────

#[test]
fn test_get_creator_succeeds_after_supply_returns_to_zero() {
    let env = contract_test_env::test_env_with_auths();
    let (client, _) = contract_test_env::register_creator_keys(&env);
    let creator = contract_test_env::register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    contract_test_env::set_key_price_for_tests(&env, &client, 100_i128);
    client.buy_key(&creator, &buyer, &100_i128, &None);
    client.sell_key(&creator, &buyer, &None);

    // The creator profile must still exist even with supply == 0.
    let profile = client.get_creator(&creator);
    assert_eq!(profile.supply, 0, "supply must be 0 after a full sell-back");
    assert_eq!(
        profile.creator, creator,
        "creator address in profile must match"
    );
}
