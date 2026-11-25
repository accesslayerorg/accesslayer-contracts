//! Tests for `get_key_score` view: composite and per-metric accuracy.

mod contract_test_env;

use contract_test_env::{add_fresh_caller, setup_with_admin, test_env_with_auths};
use key_reputation::{
    METRIC_HOLDER_COUNT, METRIC_PRICE_STABILITY_BPS, METRIC_RATING_AVG, METRIC_TRADE_VOLUME,
    WEIGHT_TOTAL,
};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_get_key_score_returns_zeros_for_unknown_key() {
    let env = test_env_with_auths();
    let (client, _, _admin) = setup_with_admin(&env);
    let key_id = soroban_sdk::Address::generate(&env);

    let view = client.get_key_score(&key_id);
    assert_eq!(view.composite_score, 0);
    assert_eq!(view.metrics.trade_volume, 0);
    assert_eq!(view.metrics.holder_count, 0);
    assert_eq!(view.metrics.price_stability_bps, 0);
    assert_eq!(view.metrics.rating_avg, 0);
}

#[test]
fn test_get_key_score_full_metrics_returns_1000() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = add_fresh_caller(&env, &client, &admin);
    let key_id = soroban_sdk::Address::generate(&env);

    // All metrics at maximum.
    client.update_key_score(&caller, &key_id, &METRIC_TRADE_VOLUME, &1000u32);
    client.update_key_score(&caller, &key_id, &METRIC_HOLDER_COUNT, &1000u32);
    client.update_key_score(&caller, &key_id, &METRIC_PRICE_STABILITY_BPS, &1000u32);
    client.update_key_score(&caller, &key_id, &METRIC_RATING_AVG, &1000u32);

    let view = client.get_key_score(&key_id);
    assert_eq!(view.composite_score, 1000, "all-max inputs must yield composite 1000");
    assert_eq!(view.metrics.trade_volume, 1000);
    assert_eq!(view.metrics.holder_count, 1000);
    assert_eq!(view.metrics.price_stability_bps, 1000);
    assert_eq!(view.metrics.rating_avg, 1000);
}

#[test]
fn test_get_key_score_single_metric_matches_expected_weight() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = add_fresh_caller(&env, &client, &admin);
    let key_id = soroban_sdk::Address::generate(&env);

    // Only trade_volume set to 1000; default weight is 5000/10000 = 50%.
    client.update_key_score(&caller, &key_id, &METRIC_TRADE_VOLUME, &1000u32);

    let view = client.get_key_score(&key_id);
    let weights = client.get_metric_weights();
    let expected = (1000u64 * weights.trade_volume_weight as u64 / WEIGHT_TOTAL as u64) as u32;
    assert_eq!(view.composite_score, expected);
}

#[test]
fn test_get_key_score_per_metric_breakdowns_are_accurate() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = add_fresh_caller(&env, &client, &admin);
    let key_id = soroban_sdk::Address::generate(&env);

    client.update_key_score(&caller, &key_id, &METRIC_TRADE_VOLUME, &800u32);
    client.update_key_score(&caller, &key_id, &METRIC_HOLDER_COUNT, &600u32);
    client.update_key_score(&caller, &key_id, &METRIC_PRICE_STABILITY_BPS, &400u32);
    client.update_key_score(&caller, &key_id, &METRIC_RATING_AVG, &200u32);

    let view = client.get_key_score(&key_id);
    assert_eq!(view.metrics.trade_volume, 800);
    assert_eq!(view.metrics.holder_count, 600);
    assert_eq!(view.metrics.price_stability_bps, 400);
    assert_eq!(view.metrics.rating_avg, 200);
    assert!(view.composite_score <= 1000);
}
