//! Tests for `get_buy_quote` idempotency confirming it returns the same value
//! across multiple calls at the same supply with no side effects.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::{Address as _, Ledger};

#[test]
fn test_buy_quote_idempotent_three_calls_at_supply_zero() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");

    let q1 = client.get_buy_quote(&creator);
    let q2 = client.get_buy_quote(&creator);
    let q3 = client.get_buy_quote(&creator);

    assert_eq!(q1, q2, "second call differs from first at supply 0");
    assert_eq!(q2, q3, "third call differs from second at supply 0");
}

#[test]
fn test_buy_quote_idempotent_three_calls_at_supply_ten() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    for _ in 0..10 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &buyer, &quote.total_amount, &None);
    }

    let q1 = client.get_buy_quote(&creator);
    let q2 = client.get_buy_quote(&creator);
    let q3 = client.get_buy_quote(&creator);

    assert_eq!(q1, q2, "second call differs from first at supply 10");
    assert_eq!(q2, q3, "third call differs from second at supply 10");
}

#[test]
fn test_buy_quote_idempotent_buy_between_calls_at_same_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");

    let q_before = client.get_buy_quote(&creator);

    let buyer_a = soroban_sdk::Address::generate(&env);
    let quote_for_buy = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer_a, &quote_for_buy.total_amount, &None);

    let buyer_b = soroban_sdk::Address::generate(&env);
    let quote_for_sell_back = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer_b, &quote_for_sell_back.total_amount, &None);
    env.ledger().with_mut(|l| l.sequence_number += 1);
    client.sell_key(&creator, &buyer_b, &None);

    let q_after = client.get_buy_quote(&creator);

    assert_eq!(
        q_before, q_after,
        "buy between two quote calls at the same supply changed the output"
    );
}

#[test]
fn test_buy_quote_no_storage_writes() {
    let env = test_env_with_auths();
    let (client, _contract_id) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");

    let supply_before = client.get_total_key_supply(&creator);

    for _ in 0..5 {
        let _ = client.get_buy_quote(&creator);
    }

    let supply_after = client.get_total_key_supply(&creator);
    assert_eq!(
        supply_before, supply_after,
        "supply changed after read-only quote calls"
    );

    let holder = soroban_sdk::Address::generate(&env);
    let balance_before = client.get_key_balance(&creator, &holder);
    for _ in 0..5 {
        let _ = client.get_buy_quote(&creator);
    }
    let balance_after = client.get_key_balance(&creator, &holder);
    assert_eq!(
        balance_before, balance_after,
        "holder balance changed after read-only quote calls"
    );
}
