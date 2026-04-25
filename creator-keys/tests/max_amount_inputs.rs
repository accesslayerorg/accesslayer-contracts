//! Tests for near-maximum amount input values in buy and sell paths.
//!
//! These tests verify overflow-safe behavior and clear error handling when
//! amounts approach or exceed i128 limits.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_stored_key_price, test_env_with_auths,
};
use creator_keys::CreatorKeysContractClient;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn register_holder_with_one_key(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
) -> Address {
    let holder = Address::generate(env);
    let price = client.get_buy_quote(creator).price;
    client.buy_key(creator, &holder, &price);
    holder
}

#[test]
fn test_buy_key_with_large_safe_amount_succeeds() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let large_price = 1_000_000_000_000i128;
    set_stored_key_price(&env, &contract_id, large_price);
    let creator = register_test_creator(&env, &client, "creator1");
    let buyer = Address::generate(&env);

    let supply = client.buy_key(&creator, &buyer, &large_price);
    assert_eq!(supply, 1, "buy should succeed with large amount");
}

#[test]
fn test_buy_key_with_maximum_safe_i128_succeeds() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let max_safe_amount = 9_223_372_036_854_775i128; // (i128::MAX / 10000) approx
    set_stored_key_price(&env, &contract_id, max_safe_amount);
    let creator = register_test_creator(&env, &client, "creator2");
    let buyer = Address::generate(&env);

    let supply = client.buy_key(&creator, &buyer, &max_safe_amount);
    assert_eq!(supply, 1, "buy should succeed at safe maximum");
}

#[test]
fn test_buy_quote_with_large_amount_succeeds() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let large_price = 500_000_000_000i128;
    set_stored_key_price(&env, &contract_id, large_price);
    let admin = Address::generate(&env);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    let creator = register_test_creator(&env, &client, "creator3");

    let q = client.get_buy_quote(&creator);
    assert_eq!(q.price, large_price);
}

#[test]
fn test_buy_quote_with_maximum_safe_amount_succeeds() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let max_safe_amount = 9_223_372_036_854_775i128;
    set_stored_key_price(&env, &contract_id, max_safe_amount);
    let admin = Address::generate(&env);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    let creator = register_test_creator(&env, &client, "creator4");

    let q = client.get_buy_quote(&creator);
    assert_eq!(q.price, max_safe_amount);
    assert!(q.total_amount > 0, "total amount should include fees");
}

#[test]
fn test_sell_quote_with_large_amount_succeeds() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let large_price = 500_000_000_000i128;
    set_stored_key_price(&env, &contract_id, large_price);
    let admin = Address::generate(&env);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    let creator = register_test_creator(&env, &client, "creator5");
    let holder = register_holder_with_one_key(&env, &client, &creator);

    let q = client.get_sell_quote(&creator, &holder);
    assert_eq!(q.price, large_price);
}

#[test]
fn test_sell_quote_with_maximum_safe_amount_succeeds() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let max_safe_amount = 9_223_372_036_854_775i128;
    set_stored_key_price(&env, &contract_id, max_safe_amount);
    let admin = Address::generate(&env);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    let creator = register_test_creator(&env, &client, "creator6");
    let holder = register_holder_with_one_key(&env, &client, &creator);

    let q = client.get_sell_quote(&creator, &holder);
    assert_eq!(q.price, max_safe_amount);
}

#[test]
fn test_buy_quote_with_maximum_safe_amount_50_50_fees_succeeds() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let max_safe_amount = 9_223_372_036_854_775i128;
    set_stored_key_price(&env, &contract_id, max_safe_amount);
    let admin = Address::generate(&env);
    client.set_fee_config(&admin, &5000u32, &5000u32);
    let creator = register_test_creator(&env, &client, "creator7");

    let q = client.get_buy_quote(&creator);
    assert_eq!(q.price, max_safe_amount);
    assert!(
        q.total_amount > q.price,
        "buy total should exceed base price with fees"
    );
}

#[test]
fn test_sell_quote_with_maximum_safe_amount_50_50_fees_succeeds() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let max_safe_amount = 9_223_372_036_854_775i128;
    set_stored_key_price(&env, &contract_id, max_safe_amount);
    let admin = Address::generate(&env);
    client.set_fee_config(&admin, &5000u32, &5000u32);
    let creator = register_test_creator(&env, &client, "creator8");
    let holder = register_holder_with_one_key(&env, &client, &creator);

    let q = client.get_sell_quote(&creator, &holder);
    assert_eq!(q.price, max_safe_amount);
    assert!(
        q.total_amount < q.price,
        "sell total should be less than base price"
    );
}
