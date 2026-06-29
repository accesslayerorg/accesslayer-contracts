//! Focused unit tests for invalid input paths when reading creator details.
//!
//! Each test targets a single failure condition of `get_creator_details`:
//! - `is_registered` is `false` for an unregistered address
//! - `handle` defaults to the empty string for an unregistered address
//! - `supply` defaults to `0` for an unregistered address
//! - `registered_at` defaults to `0` for an unregistered address
//! - Two distinct unregistered addresses produce independent default views
//!
//! These tests use the minimal setup necessary — no price, no fee config, no auth —
//! because `get_creator_details` never returns an error and never reads those fields.

mod contract_test_env;

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env, String};

// ── is_registered ────────────────────────────────────────────────────────────

#[test]
fn test_get_creator_details_unregistered_is_registered_is_false() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let details = client.get_creator_details(&creator);

    assert!(
        !details.is_registered,
        "is_registered must be false for an address that was never registered"
    );
}

// ── handle ───────────────────────────────────────────────────────────────────

#[test]
fn test_get_creator_details_unregistered_handle_is_empty_string() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let details = client.get_creator_details(&creator);

    assert_eq!(
        details.handle,
        String::from_str(&env, ""),
        "handle must be an empty string for an unregistered address"
    );
}

// ── supply ───────────────────────────────────────────────────────────────────

#[test]
fn test_get_creator_details_unregistered_supply_is_zero() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let details = client.get_creator_details(&creator);

    assert_eq!(
        details.supply, 0,
        "supply must be 0 for an unregistered address"
    );
}

// ── registered_at ─────────────────────────────────────────────────────────────

#[test]
fn test_get_creator_details_unregistered_registered_at_is_zero() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let details = client.get_creator_details(&creator);

    assert_eq!(
        details.registered_at, 0,
        "registered_at must be 0 for an unregistered address"
    );
}

// ── creator address echo ──────────────────────────────────────────────────────

#[test]
fn test_get_creator_details_unregistered_echoes_input_address() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let details = client.get_creator_details(&creator);

    assert_eq!(
        details.creator, creator,
        "the creator address must be echoed back even when the address is not registered"
    );
}

// ── isolation between unregistered addresses ─────────────────────────────────

#[test]
fn test_get_creator_details_two_unregistered_addresses_are_independent() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let addr_a = soroban_sdk::Address::generate(&env);
    let addr_b = soroban_sdk::Address::generate(&env);

    let details_a = client.get_creator_details(&addr_a);
    let details_b = client.get_creator_details(&addr_b);

    // Both must be unregistered, but the echoed address must differ.
    assert!(!details_a.is_registered);
    assert!(!details_b.is_registered);
    assert_ne!(
        details_a.creator, details_b.creator,
        "each unregistered address must be echoed back independently"
    );
}

// ── read is non-mutating for unregistered addresses ───────────────────────────

#[test]
fn test_get_creator_details_read_on_unregistered_does_not_mutate_state() {
    let env = contract_test_env::test_env_with_auths();
    let (client, _) = contract_test_env::register_creator_keys(&env);

    // Use a registered creator + sentinel holder as the state anchor.
    let registered = contract_test_env::register_test_creator(&env, &client, "anchor");
    let sentinel = soroban_sdk::Address::generate(&env);
    let before = contract_test_env::capture_snapshot(&client, &registered, &sentinel);

    // Read details of an entirely different, never-registered address.
    let unknown = soroban_sdk::Address::generate(&env);
    client.get_creator_details(&unknown);
    client.get_creator_details(&unknown);

    let after = contract_test_env::capture_snapshot(&client, &registered, &sentinel);
    before.assert_unchanged(&after);
}
