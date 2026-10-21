//! Integration tests for `get_key_balance` returning the correct key count
//! after sequences of buy and sell operations.
//!
//! Tests the `get_key_balance` view function through a progression of buys
//! and sells, asserting the exact key count at each step.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address};

#[test]
fn test_get_balance_after_buy_and_sell_sequence() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _ = set_key_price_for_tests(&env, &client, 100i128);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    let never_transacted = Address::generate(&env);

    // Buy 5 keys and assert get_key_balance returns 5
    for _ in 0..5 {
        client.buy_key(&creator, &buyer, &100i128, &None);
    }
    assert_eq!(
        client.get_key_balance(&creator, &buyer),
        5,
        "balance should be 5 after buying 5 keys"
    );

    // Buy 3 more keys and assert get_key_balance returns 8
    for _ in 0..3 {
        client.buy_key(&creator, &buyer, &100i128, &None);
    }
    assert_eq!(
        client.get_key_balance(&creator, &buyer),
        8,
        "balance should be 8 after buying 3 more keys (total 8)"
    );

    // Sell 4 keys and assert get_key_balance returns 4
    for _ in 0..4 {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator, &buyer, &None);
    }
    assert_eq!(
        client.get_key_balance(&creator, &buyer),
        4,
        "balance should be 4 after selling 4 keys (8 - 4)"
    );

    // Sell the remaining 4 keys and assert get_key_balance returns 0
    for _ in 0..4 {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator, &buyer, &None);
    }
    assert_eq!(
        client.get_key_balance(&creator, &buyer),
        0,
        "balance should be 0 after selling the remaining 4 keys"
    );

    // Assert a wallet that has never transacted returns 0
    assert_eq!(
        client.get_key_balance(&creator, &never_transacted),
        0,
        "wallet that has never transacted should have balance 0"
    );
}
