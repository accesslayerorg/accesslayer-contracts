//! Tests for the authorised-caller registry management.

mod contract_test_env;

use contract_test_env::{setup_with_admin, test_env_with_auths};
use key_reputation::{ContractError, METRIC_TRADE_VOLUME};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_add_authorised_caller_allows_score_updates() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = soroban_sdk::Address::generate(&env);
    client.add_authorised_caller(&admin, &caller);

    assert!(client.is_authorised_caller(&caller));

    let key_id = soroban_sdk::Address::generate(&env);
    let score = client.update_key_score(&caller, &key_id, &METRIC_TRADE_VOLUME, &300u32);
    assert!(score <= 1000);
}

#[test]
fn test_remove_authorised_caller_prevents_score_updates() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = soroban_sdk::Address::generate(&env);
    client.add_authorised_caller(&admin, &caller);
    client.remove_authorised_caller(&admin, &caller);

    assert!(!client.is_authorised_caller(&caller));

    let key_id = soroban_sdk::Address::generate(&env);
    let result = client.try_update_key_score(&caller, &key_id, &METRIC_TRADE_VOLUME, &300u32);
    assert_eq!(result, Err(Ok(ContractError::Unauthorised)));
}

#[test]
fn test_add_authorised_caller_fails_for_non_admin() {
    let env = test_env_with_auths();
    let (client, _, _admin) = setup_with_admin(&env);
    let rogue = soroban_sdk::Address::generate(&env);
    let target = soroban_sdk::Address::generate(&env);

    let result = client.try_add_authorised_caller(&rogue, &target);
    assert_eq!(result, Err(Ok(ContractError::Unauthorised)));
}

#[test]
fn test_is_authorised_caller_returns_false_for_unknown() {
    let env = test_env_with_auths();
    let (client, _, _admin) = setup_with_admin(&env);
    let unknown = soroban_sdk::Address::generate(&env);
    assert!(!client.is_authorised_caller(&unknown));
}
