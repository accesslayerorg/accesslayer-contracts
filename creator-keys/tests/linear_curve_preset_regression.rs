//! Regression tests for the linear (flat) curve preset price parity (#407).
//!
//! The contract currently uses a flat fixed-price model. These tests lock in
//! the buy and sell quote output at supply levels 0, 1, 10, 100, and 1000 so
//! that any future introduction of curve-preset dispatch cannot silently alter
//! the default economics.
//!
//! Expected values are computed directly from the pre-preset formula:
//!   price        = KEY_PRICE
//!   protocol_fee = floor(KEY_PRICE * protocol_bps / 10_000)
//!   creator_fee  = KEY_PRICE - protocol_fee
//!   buy total    = price + creator_fee + protocol_fee  (= 2 * KEY_PRICE)
//!   sell total   = price - creator_fee - protocol_fee  (= 0)

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::QuoteResponse;
use soroban_sdk::{testutils::Address as _, Address};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

/// Computes the expected QuoteResponse using the pre-preset flat formula.
fn expected_buy_quote(key_price: i128, _creator_bps: u32, protocol_bps: u32) -> QuoteResponse {
    let protocol_fee = key_price * protocol_bps as i128 / 10_000;
    let creator_fee = key_price - protocol_fee;
    QuoteResponse {
        price: key_price,
        creator_fee,
        protocol_fee,
        total_amount: key_price + creator_fee + protocol_fee,
    }
}

/// Computes the expected sell QuoteResponse using the pre-preset flat formula.
fn expected_sell_quote(key_price: i128, _creator_bps: u32, protocol_bps: u32) -> QuoteResponse {
    let protocol_fee = key_price * protocol_bps as i128 / 10_000;
    let creator_fee = key_price - protocol_fee;
    QuoteResponse {
        price: key_price,
        creator_fee,
        protocol_fee,
        total_amount: key_price - creator_fee - protocol_fee,
    }
}

/// Advance supply to `target` by buying keys with a given buyer.
fn advance_supply_to(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
    target: u32,
) {
    let current = client.get_total_key_supply(creator);
    for _ in current..target {
        client.buy_key(creator, buyer, &(KEY_PRICE * 2), &None);
    }
}

#[test]
fn test_linear_preset_buy_quote_matches_formula_at_supply_0() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    // supply = 0 (no buys yet)
    let got = client.get_buy_quote(&creator);
    assert_eq!(
        got,
        expected_buy_quote(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS)
    );
}

#[test]
fn test_linear_preset_buy_quote_matches_formula_at_supply_1() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    advance_supply_to(&client, &creator, &buyer, 1);

    let got = client.get_buy_quote(&creator);
    assert_eq!(
        got,
        expected_buy_quote(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS)
    );
}

#[test]
fn test_linear_preset_buy_quote_matches_formula_at_supply_10() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    advance_supply_to(&client, &creator, &buyer, 10);

    let got = client.get_buy_quote(&creator);
    assert_eq!(
        got,
        expected_buy_quote(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS)
    );
}

#[test]
fn test_linear_preset_buy_quote_matches_formula_at_supply_100() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    advance_supply_to(&client, &creator, &buyer, 100);

    let got = client.get_buy_quote(&creator);
    assert_eq!(
        got,
        expected_buy_quote(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS)
    );
}

#[test]
fn test_linear_preset_buy_quote_matches_formula_at_supply_1000() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    advance_supply_to(&client, &creator, &buyer, 1000);

    let got = client.get_buy_quote(&creator);
    assert_eq!(
        got,
        expected_buy_quote(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS)
    );
}

#[test]
fn test_linear_preset_sell_quote_matches_formula_at_supply_1() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    advance_supply_to(&client, &creator, &buyer, 1);

    let got = client.get_sell_quote(&creator, &buyer);
    assert_eq!(
        got,
        expected_sell_quote(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS)
    );
}

#[test]
fn test_linear_preset_sell_quote_matches_formula_at_supply_10() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    advance_supply_to(&client, &creator, &buyer, 10);

    let got = client.get_sell_quote(&creator, &buyer);
    assert_eq!(
        got,
        expected_sell_quote(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS)
    );
}

#[test]
fn test_linear_preset_sell_quote_matches_formula_at_supply_100() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    advance_supply_to(&client, &creator, &buyer, 100);

    let got = client.get_sell_quote(&creator, &buyer);
    assert_eq!(
        got,
        expected_sell_quote(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS)
    );
}

#[test]
fn test_linear_preset_sell_quote_matches_formula_at_supply_1000() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    advance_supply_to(&client, &creator, &buyer, 1000);

    let got = client.get_sell_quote(&creator, &buyer);
    assert_eq!(
        got,
        expected_sell_quote(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS)
    );
}
