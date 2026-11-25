//! Tests for `claim_milestone_reward` — happy path, duration gate, double-claim.

mod contract_test_env;

use contract_test_env::{
    advance_time, enter_wallet, setup_with_schedule, test_env_with_auths, REWARD_30D, REWARD_90D,
    REWARD_180D, REWARD_365D,
};
use holder_milestones::{
    ContractError, DEFAULT_DURATION_30D, DEFAULT_DURATION_90D, DEFAULT_DURATION_180D,
    DEFAULT_DURATION_365D,
};

#[test]
fn test_claim_30d_milestone_succeeds_after_duration() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_30D);
    let amount = client.claim_milestone_reward(&wallet, &key_id, &0u32);
    assert_eq!(amount, REWARD_30D);
}

#[test]
fn test_claim_all_four_milestones_in_order() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_365D);
    assert_eq!(client.claim_milestone_reward(&wallet, &key_id, &0u32), REWARD_30D);
    assert_eq!(client.claim_milestone_reward(&wallet, &key_id, &1u32), REWARD_90D);
    assert_eq!(client.claim_milestone_reward(&wallet, &key_id, &2u32), REWARD_180D);
    assert_eq!(client.claim_milestone_reward(&wallet, &key_id, &3u32), REWARD_365D);
}

#[test]
fn test_claim_blocked_before_duration_met() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    // One second short of the 30d threshold.
    advance_time(&env, DEFAULT_DURATION_30D - 1);
    let result = client.try_claim_milestone_reward(&wallet, &key_id, &0u32);
    assert_eq!(result, Err(Ok(ContractError::DurationNotMet)));
}

#[test]
fn test_claim_blocked_at_exactly_one_second_short() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_90D - 1);
    let result = client.try_claim_milestone_reward(&wallet, &key_id, &1u32);
    assert_eq!(result, Err(Ok(ContractError::DurationNotMet)));
}

#[test]
fn test_double_claim_is_rejected() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_30D);
    client.claim_milestone_reward(&wallet, &key_id, &0u32);

    let result = client.try_claim_milestone_reward(&wallet, &key_id, &0u32);
    assert_eq!(result, Err(Ok(ContractError::AlreadyClaimed)));
}

#[test]
fn test_claim_blocked_without_holding_record() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let stranger = soroban_sdk::testutils::Address::generate(&env);

    advance_time(&env, DEFAULT_DURATION_365D);
    let result = client.try_claim_milestone_reward(&stranger, &key_id, &0u32);
    assert_eq!(result, Err(Ok(ContractError::HoldingNotStarted)));
}

#[test]
fn test_claim_blocked_after_full_exit() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    // Hold long enough then exit fully.
    advance_time(&env, DEFAULT_DURATION_30D);
    client.record_exit(&wallet, &key_id);

    let result = client.try_claim_milestone_reward(&wallet, &key_id, &0u32);
    assert_eq!(result, Err(Ok(ContractError::HoldingNotStarted)));
}

#[test]
fn test_claim_blocked_with_invalid_milestone_index() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_365D);
    let result = client.try_claim_milestone_reward(&wallet, &key_id, &4u32);
    assert_eq!(result, Err(Ok(ContractError::InvalidMilestoneIndex)));
}

#[test]
fn test_claim_blocked_when_no_schedule_configured() {
    let env = test_env_with_auths();
    let (client, _, _admin) = contract_test_env::setup_with_admin(&env);
    let key_id = soroban_sdk::testutils::Address::generate(&env);
    let wallet = soroban_sdk::testutils::Address::generate(&env);
    client.record_entry(&wallet, &key_id);

    advance_time(&env, DEFAULT_DURATION_365D);
    let result = client.try_claim_milestone_reward(&wallet, &key_id, &0u32);
    assert_eq!(result, Err(Ok(ContractError::ScheduleNotFound)));
}

#[test]
fn test_later_milestones_claimable_without_claiming_earlier_ones() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    // Skip milestone 0 and 1; directly claim milestone 2 after 180d.
    advance_time(&env, DEFAULT_DURATION_180D);
    let amount = client.claim_milestone_reward(&wallet, &key_id, &2u32);
    assert_eq!(amount, REWARD_180D);
}
