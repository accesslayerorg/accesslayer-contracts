//! Tests for `get_milestone_status` view accuracy.

mod contract_test_env;

use contract_test_env::{
    advance_time, enter_wallet, setup_with_schedule, test_env_with_auths, REWARD_30D,
};
use holder_milestones::{
    ContractError, DEFAULT_DURATION_30D, DEFAULT_DURATION_90D, DEFAULT_DURATION_180D,
    DEFAULT_DURATION_365D, MILESTONE_COUNT,
};

#[test]
fn test_status_returns_all_not_claimable_for_fresh_holder() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    let view = client.get_milestone_status(&wallet, &key_id).unwrap();
    assert_eq!(view.milestones.len(), MILESTONE_COUNT);
    for i in 0..MILESTONE_COUNT {
        let ms = view.milestones.get(i).unwrap();
        assert!(!ms.claimed, "milestone {i} should not be claimed");
        assert!(!ms.claimable, "milestone {i} should not yet be claimable");
    }
}

#[test]
fn test_status_marks_claimable_after_30d() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_30D);
    let view = client.get_milestone_status(&wallet, &key_id).unwrap();

    let ms0 = view.milestones.get(0).unwrap();
    assert!(!ms0.claimed);
    assert!(ms0.claimable, "30d milestone must be claimable after 30d");

    // Others not yet claimable.
    for i in 1..MILESTONE_COUNT {
        let ms = view.milestones.get(i).unwrap();
        assert!(!ms.claimable, "milestone {i} must not be claimable yet");
    }
}

#[test]
fn test_status_reflects_claimed_after_claim() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_30D);
    client.claim_milestone_reward(&wallet, &key_id, &0u32);

    let view = client.get_milestone_status(&wallet, &key_id).unwrap();
    let ms0 = view.milestones.get(0).unwrap();
    assert!(ms0.claimed);
    assert!(!ms0.claimable, "claimed milestone must not show as claimable");
}

#[test]
fn test_status_held_secs_matches_elapsed_time() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_90D);
    let view = client.get_milestone_status(&wallet, &key_id).unwrap();
    assert_eq!(view.held_secs, DEFAULT_DURATION_90D);
}

#[test]
fn test_status_shows_zero_held_secs_after_full_exit() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_90D);
    client.record_exit(&wallet, &key_id);

    let view = client.get_milestone_status(&wallet, &key_id).unwrap();
    assert_eq!(view.held_secs, 0, "held_secs must be 0 after exit");
    for i in 0..MILESTONE_COUNT {
        let ms = view.milestones.get(i).unwrap();
        assert!(!ms.claimable, "no milestone should be claimable when not holding");
    }
}

#[test]
fn test_status_preserves_claimed_milestones_across_exit_and_reentry() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    // Claim 30d milestone.
    advance_time(&env, DEFAULT_DURATION_30D);
    client.claim_milestone_reward(&wallet, &key_id, &0u32);

    // Exit and re-enter.
    client.record_exit(&wallet, &key_id);
    client.record_entry(&wallet, &key_id);

    let view = client.get_milestone_status(&wallet, &key_id).unwrap();
    // Milestone 0 still shows claimed even after re-entry.
    let ms0 = view.milestones.get(0).unwrap();
    assert!(ms0.claimed, "claimed flag must survive re-entry");
}

#[test]
fn test_status_returns_error_when_no_schedule() {
    let env = test_env_with_auths();
    let (client, _, _admin) = contract_test_env::setup_with_admin(&env);
    let key_id = soroban_sdk::testutils::Address::generate(&env);
    let wallet = soroban_sdk::testutils::Address::generate(&env);

    let result = client.try_get_milestone_status(&wallet, &key_id);
    assert_eq!(result, Err(Ok(ContractError::ScheduleNotFound)));
}

#[test]
fn test_status_reward_amounts_match_schedule() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    let view = client.get_milestone_status(&wallet, &key_id).unwrap();
    assert_eq!(view.milestones.get(0).unwrap().reward_amount, REWARD_30D);
}
