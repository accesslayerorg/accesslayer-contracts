//! Tests for `update_key_score` authorisation and basic behaviour.

mod contract_test_env;

use contract_test_env::{add_fresh_caller, setup_with_admin, test_env_with_auths};
use key_reputation::{ContractError, METRIC_HOLDER_COUNT, METRIC_TRADE_VOLUME};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_update_key_score_succeeds_for_authorised_caller() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = add_fresh_caller(&env, &client, &admin);
    let key_id = soroban_sdk::Address::generate(&env);

    let score = client.update_key_score(&caller, &key_id, &METRIC_TRADE_VOLUME, &500u32);
    assert!(score <= 1000, "composite score must be ≤ 1000");
}

#[test]
fn test_update_key_score_fails_for_unauthorised_caller() {
    let env = test_env_with_auths();
    let (client, _, _admin) = setup_with_admin(&env);
    let rogue = soroban_sdk::Address::generate(&env);
    let key_id = soroban_sdk::Address::generate(&env);

    let result = client.try_update_key_score(&rogue, &key_id, &METRIC_TRADE_VOLUME, &500u32);
    assert_eq!(result, Err(Ok(ContractError::Unauthorised)));
}

#[test]
fn test_update_key_score_rejects_value_above_max() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = add_fresh_caller(&env, &client, &admin);
    let key_id = soroban_sdk::Address::generate(&env);

    let result = client.try_update_key_score(&caller, &key_id, &METRIC_TRADE_VOLUME, &1001u32);
    assert_eq!(result, Err(Ok(ContractError::MetricOutOfRange)));
}

#[test]
fn test_update_key_score_rejects_unknown_metric() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = add_fresh_caller(&env, &client, &admin);
    let key_id = soroban_sdk::Address::generate(&env);

    let result = client.try_update_key_score(&caller, &key_id, &99u32, &500u32);
    assert_eq!(result, Err(Ok(ContractError::UnknownMetric)));
}

#[test]
fn test_update_multiple_metrics_accumulates_correctly() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = add_fresh_caller(&env, &client, &admin);
    let key_id = soroban_sdk::Address::generate(&env);

    client.update_key_score(&caller, &key_id, &METRIC_TRADE_VOLUME, &1000u32);
    client.update_key_score(&caller, &key_id, &METRIC_HOLDER_COUNT, &1000u32);

    let view = client.get_key_score(&key_id);
    // Both metrics at full, others zero; composite > first single metric result.
    assert!(view.composite_score > 0);
    assert!(view.composite_score <= 1000);
}
