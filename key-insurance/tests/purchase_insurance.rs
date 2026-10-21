//! Tests for `purchase_insurance` happy path, validation, and duplicate rejection.

mod contract_test_env;

use contract_test_env::{
    buy_default_policy, setup_with_price, test_env_with_auths, DEFAULT_COVERAGE,
    DEFAULT_KEY_PRICE, DEFAULT_THRESHOLD_BPS,
};
use key_insurance::{ContractError, BPS_MAX, COVERAGE_PERIOD_SECS, PREMIUM_RATE_BPS};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_purchase_insurance_succeeds_and_returns_correct_view() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = soroban_sdk::Address::generate(&env);

    let view = client.purchase_insurance(&wallet, &key_id, &DEFAULT_COVERAGE, &DEFAULT_THRESHOLD_BPS);

    assert_eq!(view.wallet, wallet);
    assert_eq!(view.key_id, key_id);
    assert_eq!(view.coverage_amount, DEFAULT_COVERAGE);
    assert_eq!(view.threshold_bps, DEFAULT_THRESHOLD_BPS);
    assert_eq!(view.purchase_price, DEFAULT_KEY_PRICE);
    assert!(!view.paid_out);
}

#[test]
fn test_premium_computed_as_one_percent_of_coverage() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = soroban_sdk::Address::generate(&env);

    let view = client.purchase_insurance(&wallet, &key_id, &DEFAULT_COVERAGE, &DEFAULT_THRESHOLD_BPS);

    let expected_premium = DEFAULT_COVERAGE * PREMIUM_RATE_BPS as i128 / BPS_MAX as i128;
    assert_eq!(view.premium, expected_premium);
}

#[test]
fn test_expiry_is_30_days_from_purchase() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = soroban_sdk::Address::generate(&env);

    let now = env.ledger().timestamp();
    let view = client.purchase_insurance(&wallet, &key_id, &DEFAULT_COVERAGE, &DEFAULT_THRESHOLD_BPS);

    assert_eq!(view.expiry, now + COVERAGE_PERIOD_SECS);
}

#[test]
fn test_purchase_insurance_fails_with_zero_coverage() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = soroban_sdk::Address::generate(&env);

    let result = client.try_purchase_insurance(&wallet, &key_id, &0i128, &DEFAULT_THRESHOLD_BPS);
    assert_eq!(result, Err(Ok(ContractError::InvalidCoverageAmount)));
}

#[test]
fn test_purchase_insurance_fails_with_zero_threshold() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = soroban_sdk::Address::generate(&env);

    let result = client.try_purchase_insurance(&wallet, &key_id, &DEFAULT_COVERAGE, &0u32);
    assert_eq!(result, Err(Ok(ContractError::InvalidThresholdBps)));
}

#[test]
fn test_purchase_insurance_fails_with_threshold_at_bps_max() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = soroban_sdk::Address::generate(&env);

    let result = client.try_purchase_insurance(&wallet, &key_id, &DEFAULT_COVERAGE, &BPS_MAX);
    assert_eq!(result, Err(Ok(ContractError::InvalidThresholdBps)));
}

#[test]
fn test_purchase_insurance_fails_when_no_key_price_set() {
    let env = test_env_with_auths();
    let (client, _, _admin) = contract_test_env::setup_with_admin(&env);
    let key_id = soroban_sdk::Address::generate(&env); // no price set
    let wallet = soroban_sdk::Address::generate(&env);

    let result = client.try_purchase_insurance(&wallet, &key_id, &DEFAULT_COVERAGE, &DEFAULT_THRESHOLD_BPS);
    assert_eq!(result, Err(Ok(ContractError::KeyPriceNotSet)));
}

#[test]
fn test_duplicate_active_policy_is_rejected() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    let result = client.try_purchase_insurance(&wallet, &key_id, &DEFAULT_COVERAGE, &DEFAULT_THRESHOLD_BPS);
    assert_eq!(result, Err(Ok(ContractError::PolicyAlreadyExists)));
}

#[test]
fn test_purchase_insurance_allowed_after_expiry() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    // Advance past expiry.
    contract_test_env::advance_time(&env, COVERAGE_PERIOD_SECS + 1);

    // Re-purchase should succeed on the expired slot.
    let view = client.purchase_insurance(&wallet, &key_id, &DEFAULT_COVERAGE, &DEFAULT_THRESHOLD_BPS);
    assert!(!view.paid_out);
}
