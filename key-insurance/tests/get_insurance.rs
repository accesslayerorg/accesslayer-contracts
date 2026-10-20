//! Tests for `get_insurance` view accuracy.

mod contract_test_env;

use contract_test_env::{
    buy_default_policy, setup_with_price, test_env_with_auths, DEFAULT_COVERAGE,
    DEFAULT_KEY_PRICE, DEFAULT_THRESHOLD_BPS,
};
use key_insurance::{BPS_MAX, COVERAGE_PERIOD_SECS, PREMIUM_RATE_BPS};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_get_insurance_returns_none_for_unknown_wallet() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let stranger = soroban_sdk::Address::generate(&env);

    assert!(client.get_insurance(&stranger, &key_id).is_none());
}

#[test]
fn test_get_insurance_returns_correct_fields_after_purchase() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let now = env.ledger().timestamp();
    let wallet = buy_default_policy(&env, &client, &key_id);

    let view = client.get_insurance(&wallet, &key_id).expect("policy must exist");

    assert_eq!(view.wallet, wallet);
    assert_eq!(view.key_id, key_id);
    assert_eq!(view.coverage_amount, DEFAULT_COVERAGE);
    assert_eq!(view.threshold_bps, DEFAULT_THRESHOLD_BPS);
    assert_eq!(view.purchase_price, DEFAULT_KEY_PRICE);
    assert_eq!(view.premium, DEFAULT_COVERAGE * PREMIUM_RATE_BPS as i128 / BPS_MAX as i128);
    assert_eq!(view.expiry, now + COVERAGE_PERIOD_SECS);
    assert!(!view.paid_out);
}

#[test]
fn test_get_insurance_reflects_paid_out_flag_after_payout() {
    let env = test_env_with_auths();
    let (client, _, admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    // Drop price below threshold and trigger payout.
    client.set_key_price(&admin, &key_id, &100i128);
    client.trigger_payout(&key_id, &wallet);

    let view = client.get_insurance(&wallet, &key_id).expect("policy must still exist");
    assert!(view.paid_out);
}

#[test]
fn test_get_insurance_is_read_only() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    // Multiple reads must return identical results.
    let v1 = client.get_insurance(&wallet, &key_id).unwrap();
    let v2 = client.get_insurance(&wallet, &key_id).unwrap();
    assert_eq!(v1.coverage_amount, v2.coverage_amount);
    assert_eq!(v1.expiry, v2.expiry);
    assert_eq!(v1.premium, v2.premium);
}

#[test]
fn test_get_insurance_coverage_and_premium_scale_proportionally() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = soroban_sdk::Address::generate(&env);
    let coverage = 50_000i128;

    client.purchase_insurance(&wallet, &key_id, &coverage, &DEFAULT_THRESHOLD_BPS);
    let view = client.get_insurance(&wallet, &key_id).unwrap();

    let expected_premium = coverage * PREMIUM_RATE_BPS as i128 / BPS_MAX as i128;
    assert_eq!(view.premium, expected_premium);
    assert_eq!(view.coverage_amount, coverage);
}
