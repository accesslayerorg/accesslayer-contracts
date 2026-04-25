//! Tests for zero-amount quote normalization in buy and sell quote paths.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_stored_key_price, test_env_with_auths,
};
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_get_buy_quote_zero_amount_returns_noop_quote() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    set_stored_key_price(&env, &contract_id, 0);

    let quote = client.get_buy_quote(&creator);
    assert_eq!(quote.price, 0);
    assert_eq!(quote.creator_fee, 0);
    assert_eq!(quote.protocol_fee, 0);
    assert_eq!(quote.total_amount, 0);
}

#[test]
fn test_get_sell_quote_zero_amount_returns_noop_quote() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    let holder = Address::generate(&env);

    client.set_key_price(&admin, &100);
    let creator = register_test_creator(&env, &client, "alice");
    client.buy_key(&creator, &holder, &100);
    set_stored_key_price(&env, &contract_id, 0);

    let quote = client.get_sell_quote(&creator, &holder);
    assert_eq!(quote.price, 0);
    assert_eq!(quote.creator_fee, 0);
    assert_eq!(quote.protocol_fee, 0);
    assert_eq!(quote.total_amount, 0);
}
