//! Read-method coverage for invalid creator/holder balance lookups.
//!
//! Sparse balance entries are expected to behave predictably: unseen creators or
//! holders should return zero rather than panicking or surfacing storage errors.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_key_price_for_tests, test_env_with_auths};
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn get_key_balance_returns_zero_for_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);

    let missing_creator = Address::generate(&env);
    let holder = Address::generate(&env);

    assert_eq!(client.get_key_balance(&missing_creator, &holder), 0);
}

#[test]
fn get_key_balance_returns_zero_for_holder_without_keys() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "creator"));

    let holder_without_keys = Address::generate(&env);

    assert_eq!(client.get_key_balance(&creator, &holder_without_keys), 0);
}

#[test]
fn get_key_balance_does_not_leak_other_holder_balance() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);
    let _admin = set_key_price_for_tests(&env, &client, 100);

    let creator = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "creator"));

    let buyer = Address::generate(&env);
    let other_holder = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100i128);

    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
    assert_eq!(client.get_key_balance(&creator, &other_holder), 0);
}
