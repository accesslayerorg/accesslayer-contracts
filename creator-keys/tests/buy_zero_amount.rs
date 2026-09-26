//! Unit and integration tests for rejecting a buy of zero keys (#722).
//!
//! A buy request with a payment of 0 should panic immediately with
//! `NotPositiveAmount` rather than executing and leaving a no-op
//! transaction on-chain.
//!
//! Acceptance criteria:
//!   1. Buying 0 keys panics with `NotPositiveAmount`
//!   2. Supply unchanged after the panic
//!   3. No `KeyPurchased` / `BUY_EVENT_NAME` event emitted

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, register_creator_keys, register_test_creator, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::{events, ContractError};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, Symbol,
};

// ---------------------------------------------------------------------------
// Acceptance criteria #1: Buying 0 keys panics with NotPositiveAmount
// ---------------------------------------------------------------------------

/// Buying 0 keys (payment = 0) must revert with NotPositiveAmount.
#[test]
fn test_buy_zero_keys_reverts_with_not_positive_amount() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100_i128, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let result = client.try_buy_key(&creator, &buyer, &0_i128, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::NotPositiveAmount)),
        "buying 0 keys must panic with NotPositiveAmount"
    );
}

/// Buying 0 keys via `buy_key_with_referrer` must also revert with NotPositiveAmount.
#[test]
fn test_buy_zero_keys_with_referrer_reverts_with_not_positive_amount() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100_i128, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    let referrer = Address::generate(&env);

    let result =
        client.try_buy_key_with_referrer(&creator, &buyer, &0_i128, &None, &Some(referrer));
    assert_eq!(
        result,
        Err(Ok(ContractError::NotPositiveAmount)),
        "buying 0 keys with referrer must also panic with NotPositiveAmount"
    );
}

/// Negative payment must also revert with NotPositiveAmount.
#[test]
fn test_buy_negative_payment_reverts_with_not_positive_amount() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100_i128, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let result = client.try_buy_key(&creator, &buyer, &(-1_i128), &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::NotPositiveAmount)),
        "negative payment must also panic with NotPositiveAmount"
    );
}

// ---------------------------------------------------------------------------
// Acceptance criteria #2: Supply unchanged after the panic
// ---------------------------------------------------------------------------

/// Buying 0 keys must not mutate supply, holder count, or buyer balance.
#[test]
fn test_buy_zero_keys_leaves_supply_and_balance_unchanged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100_i128, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Seed existing supply with a valid buyer so state is non-trivial
    let existing_buyer = Address::generate(&env);
    client.buy_key(&creator, &existing_buyer, &100_i128, &None);
    assert_eq!(client.get_total_key_supply(&creator), 1);

    let before = capture_snapshot(&client, &creator, &buyer);
    assert_eq!(before.supply, 1, "initial supply should be 1");
    assert_eq!(before.key_balance, 0, "buyer balance should be 0");

    // Attempt to buy with 0 payment
    let result = client.try_buy_key(&creator, &buyer, &0_i128, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::NotPositiveAmount)),
        "buying 0 keys must fail with NotPositiveAmount"
    );

    let after = capture_snapshot(&client, &creator, &buyer);
    before.assert_unchanged(&after);
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);
}

// ---------------------------------------------------------------------------
// Acceptance criteria #3: No event emitted
// ---------------------------------------------------------------------------

/// Buying 0 keys must emit no `KeyPurchased` / `BUY_EVENT_NAME` event.
#[test]
fn test_buy_zero_keys_emits_no_key_purchased_event() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1000_i128, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Clear event log before the buy attempt
    env.events().all();

    let result = client.try_buy_key(&creator, &buyer, &0_i128, &None);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));

    let event_log = env.events().all();
    let buy_event_count = event_log
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::BUY_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .count();

    assert_eq!(
        buy_event_count, 0,
        "no KeyPurchased / buy event must be emitted when buying zero keys"
    );
}
