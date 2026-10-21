//! Unit and integration tests for rejecting a sell of zero keys.
//!
//! Scope:
//! - Test selling 0 keys panics / reverts before any payout calculation occurs
//! - Assert total supply is unchanged after the panic / failure
//! - Assert no `KeySold` / `SELL_EVENT_NAME` event is emitted
//! - Assert state invariants hold across multiple zero-sell scenarios:
//!   - Unregistered holder with zero balance
//!   - Trader after full exit (zero remaining balance)
//!   - Holder with zero liquid balance (all keys staked)

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, register_creator_keys, register_test_creator, set_key_price_for_tests,
    set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{events, ContractError};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    Address, IntoVal, Symbol,
};

/// Calling `sell_key` directly when seller holds 0 keys must panic.
#[test]
#[should_panic]
fn test_sell_zero_keys_panics_on_direct_call() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100_i128);
    let creator = register_test_creator(&env, &client, "alice");
    let zero_seller = Address::generate(&env);

    // Direct invocation panics because seller has zero keys
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &zero_seller, &None);
}

/// Selling zero keys fails with InsufficientBalance and leaves supply and balance unchanged.
#[test]
fn test_sell_zero_keys_leaves_supply_and_balance_unchanged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100_i128);
    let creator = register_test_creator(&env, &client, "alice");

    // Seed existing supply with a valid buyer
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100_i128, &None);
    assert_eq!(client.get_total_key_supply(&creator), 1);

    let zero_seller = Address::generate(&env);
    let before = capture_snapshot(&client, &creator, &zero_seller);
    assert_eq!(before.supply, 1, "initial supply should be 1");
    assert_eq!(before.key_balance, 0, "seller balance should be 0");

    // Attempt to sell with 0 keys
    let result = client.try_sell_key(&creator, &zero_seller, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::InsufficientBalance)),
        "selling zero keys must fail with InsufficientBalance"
    );

    let after = capture_snapshot(&client, &creator, &zero_seller);
    before.assert_unchanged(&after);
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &zero_seller), 0);
}

/// Selling zero keys must emit no `KeySold` / `SELL_EVENT_NAME` event.
#[test]
fn test_sell_zero_keys_emits_no_key_sold_event() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1000_i128, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let zero_seller = Address::generate(&env);

    // Clear event log before the sell attempt
    env.events().all();

    let result = client.try_sell_key(&creator, &zero_seller, &None);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));

    let event_log = env.events().all();
    let sell_event_count = event_log
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::SELL_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .count();

    assert_eq!(
        sell_event_count, 0,
        "no KeySold / sell event must be emitted when selling zero keys"
    );
}

/// Selling zero keys after full position exit reverts, leaves supply at zero, and emits no event.
#[test]
fn test_sell_zero_keys_after_full_exit_reverts_and_emits_no_event() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100_i128, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    // Trader buys 1 key and sells 1 key (position fully closed)
    client.buy_key(&creator, &trader, &100_i128, &None);
    assert_eq!(client.get_key_balance(&creator, &trader), 1);
    assert_eq!(client.get_total_key_supply(&creator), 1);

    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &trader, &None);
    assert_eq!(client.get_key_balance(&creator, &trader), 0);
    assert_eq!(client.get_total_key_supply(&creator), 0);

    let before_second_sell = capture_snapshot(&client, &creator, &trader);

    // Clear events
    env.events().all();

    // Trader attempts to sell another key while holding 0 keys
    let result = client.try_sell_key(&creator, &trader, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::InsufficientBalance)),
        "subsequent sell with 0 keys must fail"
    );

    let after_second_sell = capture_snapshot(&client, &creator, &trader);
    before_second_sell.assert_unchanged(&after_second_sell);
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &trader), 0);

    let event_log = env.events().all();
    let sell_event_count = event_log
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::SELL_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .count();

    assert_eq!(
        sell_event_count, 0,
        "no sell event must be emitted on zero-balance sell after full exit"
    );
}

/// Selling when liquid balance is zero (all keys staked) reverts, preserves supply, and emits no event.
#[test]
fn test_sell_zero_liquid_keys_when_all_staked_reverts_and_emits_no_event() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100_i128, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    // Holder buys 2 keys and stakes all 2 keys
    client.buy_key(&creator, &holder, &100_i128, &None);
    client.buy_key(&creator, &holder, &100_i128, &None);
    client.stake_keys(&creator, &holder, &2);

    assert_eq!(client.get_key_balance(&creator, &holder), 2);
    assert_eq!(client.get_staked_balance(&creator, &holder), 2);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 0);
    assert_eq!(client.get_total_key_supply(&creator), 2);

    let before = capture_snapshot(&client, &creator, &holder);

    // Clear event log
    env.events().all();

    // Holder attempts to sell when liquid balance is 0
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    let result = client.try_sell_key(&creator, &holder, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::InsufficientBalance)),
        "sell must fail when liquid keys count is 0"
    );

    let after = capture_snapshot(&client, &creator, &holder);
    before.assert_unchanged(&after);
    assert_eq!(client.get_total_key_supply(&creator), 2);
    assert_eq!(client.get_staked_balance(&creator, &holder), 2);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 0);

    let event_log = env.events().all();
    let sell_event_count = event_log
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::SELL_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .count();

    assert_eq!(
        sell_event_count, 0,
        "no sell event must be emitted when selling zero liquid keys"
    );
}
