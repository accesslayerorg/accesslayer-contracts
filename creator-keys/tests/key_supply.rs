//! Tests for get_total_key_supply view method (#18) and zero-amount validation (#20).

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &id);
    let admin = Address::generate(env);
    client.set_key_price(&admin, &100_i128);
    (client, admin)
}

// ── get_total_key_supply tests (#18) ────────────────────────────────────

#[test]
fn test_get_total_key_supply_returns_zero_for_new_creator() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "alice"));

    assert_eq!(client.get_total_key_supply(&creator), 0);
}

#[test]
fn test_get_total_key_supply_returns_zero_for_unregistered() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let unknown = Address::generate(&env);
    assert_eq!(client.get_total_key_supply(&unknown), 0);
}

#[test]
fn test_get_total_key_supply_increments_after_buy() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "alice"));

    assert_eq!(client.get_total_key_supply(&creator), 0);

    client.buy_key(&creator, &buyer, &100_i128);
    assert_eq!(client.get_total_key_supply(&creator), 1);

    let buyer2 = Address::generate(&env);
    client.buy_key(&creator, &buyer2, &100_i128);
    assert_eq!(client.get_total_key_supply(&creator), 2);
}

#[test]
fn test_get_total_key_supply_is_read_only() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "alice"));

    // Call multiple times — should not change state
    let s1 = client.get_total_key_supply(&creator);
    let s2 = client.get_total_key_supply(&creator);
    let s3 = client.get_total_key_supply(&creator);
    assert_eq!(s1, s2);
    assert_eq!(s2, s3);
}

// ── Zero-amount purchase validation tests (#20) ─────────────────────────

#[test]
fn test_buy_key_zero_payment_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "alice"));

    let result = client.try_buy_key(&creator, &buyer, &0_i128);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn test_buy_key_negative_payment_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "alice"));

    let result = client.try_buy_key(&creator, &buyer, &-50_i128);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn test_buy_key_positive_payment_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "alice"));

    let supply = client.buy_key(&creator, &buyer, &100_i128);
    assert_eq!(supply, 1);
}

// ── Shared helper used in set_key_price (#21) ───────────────────────────

#[test]
fn test_set_key_price_zero_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &id);
    let admin = Address::generate(&env);

    let result = client.try_set_key_price(&admin, &0_i128);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn test_set_key_price_negative_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &id);
    let admin = Address::generate(&env);

    let result = client.try_set_key_price(&admin, &-10_i128);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}
