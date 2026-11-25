//! Tests for `trigger_payout` — threshold logic, expiry, double-claim, and cap.

mod contract_test_env;

use contract_test_env::{
    advance_time, buy_default_policy, setup_with_price, test_env_with_auths, DEFAULT_COVERAGE,
    DEFAULT_KEY_PRICE, DEFAULT_THRESHOLD_BPS,
};
use key_insurance::{ContractError, COVERAGE_PERIOD_SECS};
use soroban_sdk::testutils::Address as _;

// Price that sits below the 50% threshold (< 500 when purchase price = 1000).
const BELOW_THRESHOLD_PRICE: i128 = 400;

// Price that sits above the 50% threshold.
const ABOVE_THRESHOLD_PRICE: i128 = 600;

#[test]
fn test_trigger_payout_succeeds_when_price_below_threshold() {
    let env = test_env_with_auths();
    let (client, _, admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    // Drop the price below threshold.
    client.set_key_price(&admin, &key_id, &BELOW_THRESHOLD_PRICE);
    let payout = client.trigger_payout(&key_id, &wallet);
    assert_eq!(payout, DEFAULT_COVERAGE);
}

#[test]
fn test_trigger_payout_blocked_when_price_above_threshold() {
    let env = test_env_with_auths();
    let (client, _, admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    client.set_key_price(&admin, &key_id, &ABOVE_THRESHOLD_PRICE);
    let result = client.try_trigger_payout(&key_id, &wallet);
    assert_eq!(result, Err(Ok(ContractError::PriceAboveThreshold)));
}

#[test]
fn test_trigger_payout_blocked_at_exact_threshold() {
    let env = test_env_with_auths();
    let (client, _, admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    // Exactly at threshold (= purchase_price * threshold_bps / BPS_MAX = 500); not strictly below.
    let at_threshold: i128 = DEFAULT_KEY_PRICE * DEFAULT_THRESHOLD_BPS as i128 / 10_000;
    client.set_key_price(&admin, &key_id, &at_threshold);
    let result = client.try_trigger_payout(&key_id, &wallet);
    assert_eq!(result, Err(Ok(ContractError::PriceAboveThreshold)));
}

#[test]
fn test_trigger_payout_blocked_after_expiry() {
    let env = test_env_with_auths();
    let (client, _, admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    client.set_key_price(&admin, &key_id, &BELOW_THRESHOLD_PRICE);
    advance_time(&env, COVERAGE_PERIOD_SECS + 1);

    let result = client.try_trigger_payout(&key_id, &wallet);
    assert_eq!(result, Err(Ok(ContractError::PolicyExpired)));
}

#[test]
fn test_trigger_payout_blocked_when_no_policy() {
    let env = test_env_with_auths();
    let (client, _, admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let stranger = soroban_sdk::Address::generate(&env);

    client.set_key_price(&admin, &key_id, &BELOW_THRESHOLD_PRICE);
    let result = client.try_trigger_payout(&key_id, &stranger);
    assert_eq!(result, Err(Ok(ContractError::PolicyNotFound)));
}

#[test]
fn test_payout_does_not_exceed_coverage_amount() {
    let env = test_env_with_auths();
    let (client, _, admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    client.set_key_price(&admin, &key_id, &1i128); // price near zero
    let payout = client.trigger_payout(&key_id, &wallet);
    assert!(payout <= DEFAULT_COVERAGE);
    assert_eq!(payout, DEFAULT_COVERAGE);
}

#[test]
fn test_double_claim_is_blocked() {
    let env = test_env_with_auths();
    let (client, _, admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    client.set_key_price(&admin, &key_id, &BELOW_THRESHOLD_PRICE);
    client.trigger_payout(&key_id, &wallet); // first claim
    let result = client.try_trigger_payout(&key_id, &wallet); // second claim
    assert_eq!(result, Err(Ok(ContractError::PolicyNotFound)));
}

#[test]
fn test_trigger_payout_blocked_when_price_equals_purchase_price() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    // Price unchanged — still at purchase price, well above threshold.
    let result = client.try_trigger_payout(&key_id, &wallet);
    assert_eq!(result, Err(Ok(ContractError::PriceAboveThreshold)));
}
