//! Tests for optional max_supply cap per creator at registration.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_key_price_for_tests, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, String};

fn register_capped(
    env: &soroban_sdk::Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    handle: &str,
    cap: u32,
) -> Address {
    let creator = Address::generate(env);
    client.register_creator(&creator, &String::from_str(env, handle), &Some(cap));
    creator
}

fn register_uncapped(
    env: &soroban_sdk::Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    handle: &str,
) -> Address {
    let creator = Address::generate(env);
    client.register_creator(&creator, &String::from_str(env, handle), &None);
    creator
}

// ── None (uncapped) behavior ─────────────────────────────────────────────

#[test]
fn test_uncapped_buy_succeeds_without_limit() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100);

    let creator = register_uncapped(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Buy many keys — no cap should stop this.
    for expected in 1u32..=5 {
        let supply = client.buy_key(&creator, &buyer, &100);
        assert_eq!(supply, expected);
    }
}

#[test]
fn test_get_max_supply_returns_none_for_uncapped_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = register_uncapped(&env, &client, "alice");
    assert_eq!(client.get_max_supply(&creator), None);
}

// ── Some(n) cap validation ────────────────────────────────────────────────

#[test]
fn test_registration_with_zero_cap_is_rejected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let result =
        client.try_register_creator(&creator, &String::from_str(&env, "alice"), &Some(0u32));
    assert_eq!(result, Err(Ok(ContractError::InvalidMaxSupply)));
}

#[test]
fn test_get_max_supply_returns_cap_for_capped_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = register_capped(&env, &client, "alice", 5);
    assert_eq!(client.get_max_supply(&creator), Some(5));
}

// ── Capped buy within limit ───────────────────────────────────────────────

#[test]
fn test_capped_buy_within_limit_succeeds() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100);

    let creator = register_capped(&env, &client, "alice", 3);
    let buyer = Address::generate(&env);

    let supply = client.buy_key(&creator, &buyer, &100);
    assert_eq!(supply, 1);

    let supply = client.buy_key(&creator, &buyer, &100);
    assert_eq!(supply, 2);
}

// ── Capped buy at limit (last allowed key) ───────────────────────────────

#[test]
fn test_capped_buy_at_limit_succeeds() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100);

    let creator = register_capped(&env, &client, "alice", 3);
    let buyer = Address::generate(&env);

    // Buy up to the cap exactly.
    client.buy_key(&creator, &buyer, &100);
    client.buy_key(&creator, &buyer, &100);
    let supply = client.buy_key(&creator, &buyer, &100);
    assert_eq!(supply, 3);
}

// ── Capped buy exceeding limit ───────────────────────────────────────────

#[test]
fn test_capped_buy_exceeding_limit_is_rejected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100);

    let creator = register_capped(&env, &client, "alice", 3);
    let buyer = Address::generate(&env);

    client.buy_key(&creator, &buyer, &100);
    client.buy_key(&creator, &buyer, &100);
    client.buy_key(&creator, &buyer, &100);

    // One more buy must fail with SupplyCapExceeded.
    let result = client.try_buy_key(&creator, &buyer, &100);
    assert_eq!(result, Err(Ok(ContractError::SupplyCapExceeded)));
}

#[test]
fn test_capped_buy_one_key_cap_then_exceed_is_rejected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100);

    let creator = register_capped(&env, &client, "alice", 1);
    let buyer = Address::generate(&env);

    let supply = client.buy_key(&creator, &buyer, &100);
    assert_eq!(supply, 1);

    let result = client.try_buy_key(&creator, &buyer, &100);
    assert_eq!(result, Err(Ok(ContractError::SupplyCapExceeded)));
}

// ── Cap is immutable (no update function exists) ─────────────────────────

#[test]
fn test_max_supply_is_stable_after_buys() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100);

    let creator = register_capped(&env, &client, "alice", 5);
    let buyer = Address::generate(&env);

    client.buy_key(&creator, &buyer, &100);
    client.buy_key(&creator, &buyer, &100);

    // Cap must remain unchanged after purchases.
    assert_eq!(client.get_max_supply(&creator), Some(5));
}
