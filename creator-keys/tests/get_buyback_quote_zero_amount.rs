//! Unit tests for `get_buyback_quote` with a zero amount input.
//!
//! Calling the view with `amount = 0` must return zero without reverting and must
//! not mutate any contract state. The actual `buyback` operation still rejects zero
//! via `NotPositiveAmount`; only the read-only quote path is permissive here.

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, register_creator_keys, register_test_creator, set_pricing_and_fees,
    test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

#[test]
fn test_get_buyback_quote_returns_zero_for_zero_amount() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    let result = client.get_buyback_quote(&creator, &0);

    assert_eq!(
        result, 0,
        "get_buyback_quote with amount zero must return zero"
    );
}

#[test]
fn test_get_buyback_quote_zero_amount_does_not_change_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    let buyer = soroban_sdk::Address::generate(&env);
    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let before = capture_snapshot(&client, &creator, &buyer);
    client.get_buyback_quote(&creator, &0);
    let after = capture_snapshot(&client, &creator, &buyer);

    before.assert_unchanged(&after);
}

#[test]
fn test_get_buyback_quote_zero_amount_does_not_change_holder_balance() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    let buyer = soroban_sdk::Address::generate(&env);
    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let balance_before = client.get_key_balance(&creator, &buyer);
    client.get_buyback_quote(&creator, &0);
    let balance_after = client.get_key_balance(&creator, &buyer);

    assert_eq!(
        balance_before, balance_after,
        "get_buyback_quote must not alter holder balance"
    );
}
