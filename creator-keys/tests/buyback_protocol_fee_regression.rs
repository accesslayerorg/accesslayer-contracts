//! Regression tests for protocol fee applied during creator buyback (#469).
//!
//! Covers: treasury balance increases by the correct protocol fee on buyback,
//! creator is charged bonding curve price plus protocol fee, and creator fee
//! recipient balance is unchanged (creator fee is waived on buyback).

mod contract_test_env;

use contract_test_env::{
    compute_expected_protocol_fee, register_creator_keys, register_test_creator,
    set_pricing_and_fees, test_env_with_auths,
};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

// ---------------------------------------------------------------------------
// Treasury receives protocol fee on buyback
// ---------------------------------------------------------------------------

#[test]
fn test_treasury_balance_increases_by_protocol_fee_on_buyback() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    // Creator self-buys 2 keys
    for _ in 0..2 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &creator, &quote.total_amount, &None);
    }

    let treasury_before = client.get_protocol_recipient_balance();
    let expected_protocol_fee = compute_expected_protocol_fee(KEY_PRICE, PROTOCOL_BPS);

    let total_cost = client.get_buyback_quote(&creator, &1);
    client.buyback(&creator, &creator, &1, &total_cost, &None);

    let treasury_after = client.get_protocol_recipient_balance();
    assert_eq!(
        treasury_after,
        treasury_before + expected_protocol_fee,
        "treasury must increase by exactly the protocol fee on a single-key buyback"
    );
}

#[test]
fn test_treasury_balance_increases_by_protocol_fee_on_multi_key_buyback() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    // Creator self-buys 3 keys
    for _ in 0..3 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &creator, &quote.total_amount, &None);
    }

    let treasury_before = client.get_protocol_recipient_balance();
    let buyback_amount: u32 = 2;
    let base_price = KEY_PRICE * buyback_amount as i128;
    let expected_protocol_fee = compute_expected_protocol_fee(base_price, PROTOCOL_BPS);

    let total_cost = client.get_buyback_quote(&creator, &buyback_amount);
    client.buyback(&creator, &creator, &buyback_amount, &total_cost, &None);

    let treasury_after = client.get_protocol_recipient_balance();
    assert_eq!(
        treasury_after,
        treasury_before + expected_protocol_fee,
        "treasury must increase by protocol fee on a {}-key buyback",
        buyback_amount,
    );
}

// ---------------------------------------------------------------------------
// Creator is charged bonding curve price plus protocol fee (not creator fee)
// ---------------------------------------------------------------------------

#[test]
fn test_buyback_quote_equals_bonding_curve_price_plus_protocol_fee() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    for _ in 0..2 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &creator, &quote.total_amount, &None);
    }

    let expected_base = KEY_PRICE;
    let expected_protocol_fee = compute_expected_protocol_fee(expected_base, PROTOCOL_BPS);
    let expected_total = expected_base + expected_protocol_fee;

    let actual_quote = client.get_buyback_quote(&creator, &1);
    assert_eq!(
        actual_quote, expected_total,
        "buyback quote must equal bonding curve price plus protocol fee (no creator fee)"
    );
}

// ---------------------------------------------------------------------------
// Creator fee recipient balance unchanged — creator fee waived on buyback
// ---------------------------------------------------------------------------

#[test]
fn test_creator_fee_recipient_balance_unchanged_on_buyback() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    for _ in 0..2 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &creator, &quote.total_amount, &None);
    }

    let creator_fee_before = client.get_creator_fee_balance(&creator);
    let total_cost = client.get_buyback_quote(&creator, &1);
    client.buyback(&creator, &creator, &1, &total_cost, &None);

    let creator_fee_after = client.get_creator_fee_balance(&creator);
    assert_eq!(
        creator_fee_after, creator_fee_before,
        "creator fee recipient balance must not change on buyback — creator fee is waived"
    );
}

// ---------------------------------------------------------------------------
// Treasury balance is unchanged when buyback reverts (zero amount)
// ---------------------------------------------------------------------------

#[test]
fn test_treasury_balance_unchanged_when_buyback_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    // Buy one key so the creator has a position
    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &creator, &quote.total_amount, &None);

    let treasury_before = client.get_protocol_recipient_balance();

    // Buyback with zero amount reverts
    let result = client.try_buyback(&creator, &creator, &0, &1, &None);
    assert!(result.is_err(), "buyback with zero amount must revert");

    let treasury_after = client.get_protocol_recipient_balance();
    assert_eq!(
        treasury_after, treasury_before,
        "treasury must be unchanged after a reverted buyback"
    );
}
