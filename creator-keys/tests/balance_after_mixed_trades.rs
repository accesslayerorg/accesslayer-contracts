//! Tests for key balance tracking through mixed buy and sell sequences.
//!
//! Uses the `compute_expected_balance_after_trades` helper to verify that
//! balance tracking remains correct regardless of trade order.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, TradeOperation,
    compute_expected_balance_after_trades, test_env_with_auths,
};
use soroban_sdk::Address;

#[test]
fn test_balance_after_sequence_of_buys_and_sells() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_key_price_for_tests(&env, &client, 100i128);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Execute a mixed sequence: buy, buy, sell, buy, sell, sell
    let trades = vec![
        TradeOperation::Buy,
        TradeOperation::Buy,
        TradeOperation::Sell,
        TradeOperation::Buy,
        TradeOperation::Sell,
        TradeOperation::Sell,
    ];

    // Initial balance is 0, so final expected balance is 0
    let expected = compute_expected_balance_after_trades(0, &trades);
    assert_eq!(expected, 0);

    // Execute trades
    client.buy_key(&creator, &buyer, &100i128);
    client.buy_key(&creator, &buyer, &100i128);
    client.sell_key(&creator, &buyer);
    client.buy_key(&creator, &buyer, &100i128);
    client.sell_key(&creator, &buyer);
    client.sell_key(&creator, &buyer);

    // Verify actual balance matches expected
    let actual = client.get_key_balance(&creator, &buyer);
    assert_eq!(actual, expected);
}

#[test]
fn test_balance_after_buys_then_sells() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Execute 5 buys followed by 2 sells: (5 - 2) = 3 remaining
    let trades = vec![
        TradeOperation::Buy,
        TradeOperation::Buy,
        TradeOperation::Buy,
        TradeOperation::Buy,
        TradeOperation::Buy,
        TradeOperation::Sell,
        TradeOperation::Sell,
    ];

    let expected = compute_expected_balance_after_trades(0, &trades);
    assert_eq!(expected, 3);

    for _ in 0..5 {
        client.buy_key(&creator, &buyer, &100i128);
    }
    for _ in 0..2 {
        client.sell_key(&creator, &buyer);
    }

    let actual = client.get_key_balance(&creator, &buyer);
    assert_eq!(actual, expected);
}

#[test]
fn test_balance_with_non_zero_initial() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Buy 4 keys first (initial balance = 4)
    for _ in 0..4 {
        client.buy_key(&creator, &buyer, &100i128);
    }

    // Then apply additional trades: buy, sell, sell, buy, buy
    let additional_trades = vec![
        TradeOperation::Buy,
        TradeOperation::Sell,
        TradeOperation::Sell,
        TradeOperation::Buy,
        TradeOperation::Buy,
    ];

    // Starting from 4: 4+1-1-1+1+1 = 5
    let expected = compute_expected_balance_after_trades(4, &additional_trades);
    assert_eq!(expected, 5);

    client.buy_key(&creator, &buyer, &100i128);
    client.sell_key(&creator, &buyer);
    client.sell_key(&creator, &buyer);
    client.buy_key(&creator, &buyer, &100i128);
    client.buy_key(&creator, &buyer, &100i128);

    let actual = client.get_key_balance(&creator, &buyer);
    assert_eq!(actual, expected);
}
