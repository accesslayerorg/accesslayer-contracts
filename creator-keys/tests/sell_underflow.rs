//! Targeted tests for the explicit SellUnderflow error variant on the sell path (#174).
//!
//! Verifies that checked-subtraction failures in sell_key map to
//! ContractError::SellUnderflow and that the discriminant is stable.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup(env: &Env, price: i128) -> (creator_keys::CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, price);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

// ── Discriminant stability ────────────────────────────────────────────────

#[test]
fn test_sell_underflow_discriminant_is_10() {
    assert_eq!(ContractError::SellUnderflow as u32, 10);
}

#[test]
fn test_sell_underflow_does_not_alias_overflow() {
    assert_ne!(
        ContractError::SellUnderflow as u32,
        ContractError::Overflow as u32
    );
}

#[test]
fn test_sell_underflow_does_not_alias_insufficient_balance() {
    assert_ne!(
        ContractError::SellUnderflow as u32,
        ContractError::InsufficientBalance as u32
    );
}

// ── Sell with zero balance produces InsufficientBalance (not SellUnderflow) ──

#[test]
fn test_sell_with_no_keys_returns_insufficient_balance() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);
    let seller = Address::generate(&env);

    // seller never bought — balance is 0
    let result = client.try_sell_key(&creator, &seller);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}

#[test]
fn test_sell_second_key_after_selling_last_returns_insufficient_balance() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);
    let seller = Address::generate(&env);

    client.buy_key(&creator, &seller, &100);
    client.sell_key(&creator, &seller);

    // No keys left — should be InsufficientBalance, not SellUnderflow
    let result = client.try_sell_key(&creator, &seller);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}

// ── Normal sell path uses SellUnderflow only for arithmetic underflow ─────

#[test]
fn test_sell_after_buy_succeeds_without_underflow_error() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);
    let seller = Address::generate(&env);

    client.buy_key(&creator, &seller, &100);
    let result = client.try_sell_key(&creator, &seller);

    assert!(result.is_ok(), "expected Ok but got {:?}", result);
}

#[test]
fn test_sell_two_keys_succeeds_without_underflow_error() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);
    let seller = Address::generate(&env);

    client.buy_key(&creator, &seller, &100);
    client.buy_key(&creator, &seller, &100);
    client.sell_key(&creator, &seller);

    let result = client.try_sell_key(&creator, &seller);
    assert!(
        result.is_ok(),
        "second sell should succeed, got {:?}",
        result
    );
}

// ── Supply and balance remain consistent after sell ───────────────────────

#[test]
fn test_supply_and_balance_decremented_correctly_after_sell() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);
    let seller = Address::generate(&env);

    client.buy_key(&creator, &seller, &100);
    client.buy_key(&creator, &seller, &100);
    client.sell_key(&creator, &seller);

    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &seller), 1);
}

// ── Error constant identifier ─────────────────────────────────────────────

#[test]
fn test_sell_underflow_error_constant_value() {
    use creator_keys::quote_view_errors::ERR_SELL_UNDERFLOW;
    assert_eq!(ERR_SELL_UNDERFLOW, "sell_underflow");
}
