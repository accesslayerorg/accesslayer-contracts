//! Tests for admin weight updates and score-range invariants.

mod contract_test_env;

use contract_test_env::{add_fresh_caller, setup_with_admin, test_env_with_auths};
use key_reputation::{ContractError, METRIC_TRADE_VOLUME, WEIGHT_TOTAL};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_set_metric_weights_succeeds_with_valid_sum() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);

    // Equal split.
    client.set_metric_weights(&admin, &2500u32, &2500u32, &2500u32, &2500u32);
    let w = client.get_metric_weights();
    assert_eq!(w.trade_volume_weight, 2500);
    assert_eq!(
        w.trade_volume_weight + w.holder_count_weight + w.price_stability_weight + w.rating_avg_weight,
        WEIGHT_TOTAL
    );
}

#[test]
fn test_set_metric_weights_fails_when_sum_is_wrong() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);

    let result = client.try_set_metric_weights(&admin, &3000u32, &3000u32, &3000u32, &3000u32);
    assert_eq!(result, Err(Ok(ContractError::WeightOverflow)));
}

#[test]
fn test_set_metric_weights_fails_for_non_admin() {
    let env = test_env_with_auths();
    let (client, _, _admin) = setup_with_admin(&env);
    let rogue = soroban_sdk::Address::generate(&env);

    let result = client.try_set_metric_weights(&rogue, &2500u32, &2500u32, &2500u32, &2500u32);
    assert_eq!(result, Err(Ok(ContractError::Unauthorised)));
}

#[test]
fn test_score_range_invariant_holds_after_weight_update() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = add_fresh_caller(&env, &client, &admin);
    let key_id = soroban_sdk::Address::generate(&env);

    // Set all metrics to max.
    for metric in [0u32, 1u32, 2u32, 3u32] {
        client.update_key_score(&caller, &key_id, &metric, &1000u32);
    }

    // Change weights and verify score stays ≤ 1000.
    client.set_metric_weights(&admin, &4000u32, &2000u32, &2000u32, &2000u32);
    let view = client.get_key_score(&key_id);
    assert!(view.composite_score <= 1000);
}

#[test]
fn test_score_is_zero_when_all_metrics_are_zero() {
    let env = test_env_with_auths();
    let (client, _, _admin) = setup_with_admin(&env);
    let key_id = soroban_sdk::Address::generate(&env);

    let view = client.get_key_score(&key_id);
    assert_eq!(view.composite_score, 0);
}

#[test]
fn test_single_metric_at_max_does_not_exceed_1000() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = add_fresh_caller(&env, &client, &admin);
    let key_id = soroban_sdk::Address::generate(&env);

    let score = client.update_key_score(&caller, &key_id, &METRIC_TRADE_VOLUME, &1000u32);
    assert!(score <= 1000);
}
