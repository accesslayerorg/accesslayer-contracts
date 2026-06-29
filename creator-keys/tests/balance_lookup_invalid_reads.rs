//! Tests for invalid balance lookup input paths.
//!
//! Covers edge cases for `get_key_balance` and `get_holder_key_count`
//! with unregistered creators and zero-balance holders.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::{testutils::Address as _, Env};

// ── get_key_balance: unregistered creator ────────────────────────────────────

#[test]
fn test_get_key_balance_returns_zero_for_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(creator_keys::CreatorKeysContract, ());
    let client = creator_keys::CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = soroban_sdk::Address::generate(&env);
    let wallet = soroban_sdk::Address::generate(&env);

    let balance = client.get_key_balance(&unregistered_creator, &wallet);
    assert_eq!(balance, 0, "unregistered creator must return zero balance");
}

#[test]
fn test_get_key_balance_returns_zero_for_unregistered_creator_and_holder() {
    let env = Env::default();
    let contract_id = env.register(creator_keys::CreatorKeysContract, ());
    let client = creator_keys::CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = soroban_sdk::Address::generate(&env);
    let unregistered_holder = soroban_sdk::Address::generate(&env);

    let balance = client.get_key_balance(&unregistered_creator, &unregistered_holder);
    assert_eq!(balance, 0, "both unregistered must return zero balance");
}

// ── get_key_balance: registered creator, holder with no keys ─────────────────

#[test]
fn test_get_key_balance_returns_zero_for_holder_with_no_keys() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let holder_with_no_keys = soroban_sdk::Address::generate(&env);

    let balance = client.get_key_balance(&creator, &holder_with_no_keys);
    assert_eq!(balance, 0, "holder with no keys must return zero balance");
}

// ── get_key_balance: multiple distinct holders return independent balances ────

#[test]
fn test_get_key_balance_independent_per_holder() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let holder_a = soroban_sdk::Address::generate(&env);
    let holder_b = soroban_sdk::Address::generate(&env);

    client.buy_key(&creator, &holder_a, &100, &None);
    client.buy_key(&creator, &holder_a, &100, &None);
    client.buy_key(&creator, &holder_b, &100, &None);

    assert_eq!(client.get_key_balance(&creator, &holder_a), 2);
    assert_eq!(client.get_key_balance(&creator, &holder_b), 1);
}

// ── get_holder_key_count: unregistered creator ───────────────────────────────

#[test]
fn test_get_holder_key_count_creator_not_registered() {
    let env = Env::default();
    let contract_id = env.register(creator_keys::CreatorKeysContract, ());
    let client = creator_keys::CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = soroban_sdk::Address::generate(&env);
    let holder = soroban_sdk::Address::generate(&env);

    let view = client.get_holder_key_count(&unregistered_creator, &holder);
    assert!(!view.creator_exists, "creator_exists must be false");
    assert_eq!(view.key_count, 0, "key_count must be zero");
}

// ── get_holder_key_count: registered creator, holder with no keys ────────────

#[test]
fn test_get_holder_key_count_holder_with_no_keys() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = soroban_sdk::Address::generate(&env);

    let view = client.get_holder_key_count(&creator, &holder);
    assert!(view.creator_exists, "creator_exists must be true");
    assert_eq!(
        view.key_count, 0,
        "key_count must be zero for holder with no keys"
    );
}

// ── get_holder_key_count: correct count after buys ──────────────────────────

#[test]
fn test_get_holder_key_count_reflects_actual_balance() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = soroban_sdk::Address::generate(&env);

    client.buy_key(&creator, &holder, &100, &None);
    client.buy_key(&creator, &holder, &100, &None);
    client.buy_key(&creator, &holder, &100, &None);

    let view = client.get_holder_key_count(&creator, &holder);
    assert!(view.creator_exists);
    assert_eq!(view.key_count, 3, "key_count must match actual balance");
}

// ── get_total_key_supply: unregistered creator ───────────────────────────────

#[test]
fn test_get_total_key_supply_returns_zero_for_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(creator_keys::CreatorKeysContract, ());
    let client = creator_keys::CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered = soroban_sdk::Address::generate(&env);
    assert_eq!(
        client.get_total_key_supply(&unregistered),
        0,
        "unregistered creator must return zero supply"
    );
}

// ── get_creator_supply: unregistered creator ─────────────────────────────────

#[test]
fn test_get_creator_supply_fails_for_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(creator_keys::CreatorKeysContract, ());
    let client = creator_keys::CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered = soroban_sdk::Address::generate(&env);
    let result = client.try_get_creator_supply(&unregistered);
    assert_eq!(
        result,
        Err(Ok(creator_keys::ContractError::NotRegistered)),
        "must return NotRegistered for unregistered creator"
    );
}
