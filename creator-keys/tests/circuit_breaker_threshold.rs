//! Integration tests for the bonding-curve circuit breaker in `buy_key` /
//! `buy_key_with_referrer` (Issue #838).
//!
//! The circuit breaker compares the pre-buy and post-buy bonding-curve price
//! and rejects the trade with [`ContractError::CircuitBreakerTriggered`] when
//! the relative price jump is at or above a configurable threshold
//! (`threshold_pct`, expressed as a whole-number percentage). The threshold is
//! read from persistent storage and defaults to 30 when unset.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_curve_slope, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::{events, ContractError};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, Symbol,
};

const BASE_PRICE: i128 = 10_000;
const PAYMENT: i128 = 1_000_000;

#[test]
fn test_circuit_breaker_default_threshold_blocks_large_price_jump() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // 40% jump from supply 0 to 1 (10_000 -> 14_000); default threshold is 30%.
    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, 4_000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let result = client.try_buy_key(&creator, &buyer, &PAYMENT, &None);
    assert_eq!(result, Err(Ok(ContractError::CircuitBreakerTriggered)));

    // The rejected buy must not have mutated supply.
    assert_eq!(client.get_total_key_supply(&creator), 0);
}

#[test]
fn test_circuit_breaker_default_threshold_allows_small_price_jump() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // 1% jump from supply 0 to 1 (10_000 -> 10_100); well under the 30% default.
    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let supply = client.buy_key(&creator, &buyer, &PAYMENT, &None);
    assert_eq!(supply, 1);
}

#[test]
fn test_circuit_breaker_flat_curve_never_triggers() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // No curve slope configured: price is flat, so pre_price == post_price
    // and the circuit breaker's `post_price > pre_price` guard never fires.
    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");

    for i in 0..5 {
        let buyer = Address::generate(&env);
        let supply = client.buy_key(&creator, &buyer, &PAYMENT, &None);
        assert_eq!(supply, i + 1);
    }
}

#[test]
fn test_circuit_breaker_custom_threshold_relaxes_a_previously_blocked_jump() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // 40% jump, blocked under the 30% default.
    let admin = set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, 4_000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    assert_eq!(
        client.try_buy_key(&creator, &buyer, &PAYMENT, &None),
        Err(Ok(ContractError::CircuitBreakerTriggered))
    );

    // Raise the threshold above the 40% jump; the same trade now succeeds.
    client.set_circuit_breaker_threshold(&admin, &50u32);
    let supply = client.buy_key(&creator, &buyer, &PAYMENT, &None);
    assert_eq!(supply, 1);
}

#[test]
fn test_circuit_breaker_custom_threshold_still_blocks_larger_jumps() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // 60% jump; even a relaxed 50% threshold must still block it.
    let admin = set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, 6_000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    client.set_circuit_breaker_threshold(&admin, &50u32);

    let result = client.try_buy_key(&creator, &buyer, &PAYMENT, &None);
    assert_eq!(result, Err(Ok(ContractError::CircuitBreakerTriggered)));
}

#[test]
fn test_circuit_breaker_exact_threshold_boundary_triggers() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // price_change * 100 == pre_price * threshold_pct exactly: the check uses
    // `>=`, so an exact match at the boundary must still trigger.
    let admin = set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    client.set_circuit_breaker_threshold(&admin, &25u32);
    set_curve_slope(&env, &client, 2_500); // exactly 25% of 10_000
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let result = client.try_buy_key(&creator, &buyer, &PAYMENT, &None);
    assert_eq!(result, Err(Ok(ContractError::CircuitBreakerTriggered)));
}

#[test]
fn test_circuit_breaker_just_below_threshold_boundary_allows() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // One unit of slope below the exact 25% boundary must pass.
    let admin = set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    client.set_circuit_breaker_threshold(&admin, &25u32);
    set_curve_slope(&env, &client, 2_499);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let supply = client.buy_key(&creator, &buyer, &PAYMENT, &None);
    assert_eq!(supply, 1);
}

#[test]
fn test_circuit_breaker_triggered_event_reports_pre_and_post_price() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, 4_000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let result = client.try_buy_key(&creator, &buyer, &PAYMENT, &None);
    assert_eq!(result, Err(Ok(ContractError::CircuitBreakerTriggered)));

    let mut found = false;
    for (contract, topics, data) in env.events().all().iter() {
        if contract != contract_id {
            continue;
        }
        let name: Symbol = topics.get(0).unwrap().into_val(&env);
        if name == events::CIRCUIT_BREAKER_TRIGGERED_EVENT_NAME {
            let payload: events::CircuitBreakerTriggeredEvent = data.clone().into_val(&env);
            assert_eq!(payload.pre_price, BASE_PRICE);
            assert_eq!(payload.post_price, BASE_PRICE + 4_000);
            found = true;
        }
    }
    assert!(
        found,
        "expected a CircuitBreakerTriggered event with pre/post price payload"
    );
}

#[test]
fn test_circuit_breaker_does_not_block_second_buy_after_supply_advances_past_jump() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // First buy at a small, allowed jump (supply 0 -> 1).
    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, 200);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer1 = Address::generate(&env);
    let buyer2 = Address::generate(&env);

    let supply1 = client.buy_key(&creator, &buyer1, &PAYMENT, &None);
    assert_eq!(supply1, 1);

    // Second buy (supply 1 -> 2) is an equally small relative jump and must
    // also pass, confirming the breaker re-evaluates per trade rather than
    // latching after the first check.
    let supply2 = client.buy_key(&creator, &buyer2, &PAYMENT, &None);
    assert_eq!(supply2, 2);
}
