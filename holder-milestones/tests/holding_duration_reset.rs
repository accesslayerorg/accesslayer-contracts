//! Tests for the holding duration reset on full exit and re-entry.

mod contract_test_env;

use contract_test_env::{
    advance_time, enter_wallet, setup_with_schedule, test_env_with_auths,
};
use holder_milestones::{ContractError, DEFAULT_DURATION_30D};

#[test]
fn test_reentry_after_exit_resets_clock() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    // Accumulate 29 days then exit (not enough for 30d milestone).
    advance_time(&env, DEFAULT_DURATION_30D - 1);
    client.record_exit(&wallet, &key_id);

    // Re-enter and advance another day — only 1 day of holding in new period.
    client.record_entry(&wallet, &key_id);
    advance_time(&env, 1);

    // Total wall-clock time ≥ 30d but continuous hold since re-entry = 1s.
    let result = client.try_claim_milestone_reward(&wallet, &key_id, &0u32);
    assert_eq!(
        result,
        Err(Ok(ContractError::DurationNotMet)),
        "clock must reset on re-entry; prior hold time must not carry over"
    );
}

#[test]
fn test_claim_succeeds_after_full_30d_hold_post_reentry() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    // Exit immediately.
    client.record_exit(&wallet, &key_id);

    // Re-enter and hold a full 30d.
    client.record_entry(&wallet, &key_id);
    advance_time(&env, DEFAULT_DURATION_30D);

    let amount = client.claim_milestone_reward(&wallet, &key_id, &0u32);
    assert!(amount > 0);
}

#[test]
fn test_record_entry_is_idempotent_for_active_holder() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    let view_before = client.get_milestone_status(&wallet, &key_id).unwrap();

    // Calling record_entry again must not overwrite the existing timestamp.
    advance_time(&env, 1000);
    client.record_entry(&wallet, &key_id);

    let view_after = client.get_milestone_status(&wallet, &key_id).unwrap();
    // held_secs grows normally — entry timestamp was not reset.
    assert!(
        view_after.held_secs > view_before.held_secs,
        "timestamp must not be reset for an active holder"
    );
}

#[test]
fn test_exit_without_prior_entry_is_safe() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = soroban_sdk::testutils::Address::generate(&env);

    // record_exit with no prior entry must not panic.
    client.record_exit(&wallet, &key_id);
}

#[test]
fn test_milestone_remains_claimed_after_exit_and_reentry() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_30D);
    client.claim_milestone_reward(&wallet, &key_id, &0u32);

    // Full exit → re-entry.
    client.record_exit(&wallet, &key_id);
    client.record_entry(&wallet, &key_id);

    // Trying to claim the same milestone again must still fail.
    advance_time(&env, DEFAULT_DURATION_30D);
    let result = client.try_claim_milestone_reward(&wallet, &key_id, &0u32);
    assert_eq!(result, Err(Ok(ContractError::AlreadyClaimed)));
}
