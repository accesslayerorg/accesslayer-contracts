//! Tests for milestone schedule configuration and validation.

mod contract_test_env;

use contract_test_env::{setup_with_admin, setup_with_schedule, test_env_with_auths};
use holder_milestones::{ContractError, Milestone, DEFAULT_DURATION_30D, DEFAULT_DURATION_90D};
use soroban_sdk::{testutils::Address as _, vec};

#[test]
fn test_set_milestone_schedule_persists_correctly() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);

    let schedule = client.get_milestone_schedule(&key_id).expect("schedule must exist");
    assert_eq!(schedule.milestones.len(), 4);
    assert_eq!(schedule.milestones.get(0).unwrap().duration_secs, DEFAULT_DURATION_30D);
}

#[test]
fn test_set_milestone_schedule_fails_for_non_admin() {
    let env = test_env_with_auths();
    let (client, _, _admin) = setup_with_admin(&env);
    let key_id = soroban_sdk::Address::generate(&env);
    let rogue = soroban_sdk::Address::generate(&env);

    let result = client.try_set_milestone_schedule(&rogue, &key_id, &100, &200, &300, &400);
    assert_eq!(result, Err(Ok(ContractError::Unauthorised)));
}

#[test]
fn test_set_custom_schedule_succeeds_with_valid_ascending_durations() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let key_id = soroban_sdk::Address::generate(&env);

    let milestones = vec![
        &env,
        Milestone { duration_secs: 1_000,  reward_amount: 50  },
        Milestone { duration_secs: 2_000,  reward_amount: 100 },
        Milestone { duration_secs: 4_000,  reward_amount: 200 },
        Milestone { duration_secs: 8_000,  reward_amount: 400 },
    ];
    client.set_custom_schedule(&admin, &key_id, &milestones);

    let schedule = client.get_milestone_schedule(&key_id).unwrap();
    assert_eq!(schedule.milestones.get(0).unwrap().duration_secs, 1_000);
    assert_eq!(schedule.milestones.get(3).unwrap().reward_amount, 400);
}

#[test]
fn test_set_custom_schedule_fails_with_non_ascending_durations() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let key_id = soroban_sdk::Address::generate(&env);

    let milestones = vec![
        &env,
        Milestone { duration_secs: 2_000, reward_amount: 50  },
        Milestone { duration_secs: 1_000, reward_amount: 100 }, // ← not ascending
        Milestone { duration_secs: 4_000, reward_amount: 200 },
        Milestone { duration_secs: 8_000, reward_amount: 400 },
    ];
    let result = client.try_set_custom_schedule(&admin, &key_id, &milestones);
    assert_eq!(result, Err(Ok(ContractError::InvalidSchedule)));
}

#[test]
fn test_set_custom_schedule_fails_with_equal_durations() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let key_id = soroban_sdk::Address::generate(&env);

    let milestones = vec![
        &env,
        Milestone { duration_secs: DEFAULT_DURATION_30D, reward_amount: 50  },
        Milestone { duration_secs: DEFAULT_DURATION_30D, reward_amount: 100 }, // equal, not ascending
        Milestone { duration_secs: DEFAULT_DURATION_90D, reward_amount: 200 },
        Milestone { duration_secs: DEFAULT_DURATION_90D, reward_amount: 400 },
    ];
    let result = client.try_set_custom_schedule(&admin, &key_id, &milestones);
    assert_eq!(result, Err(Ok(ContractError::InvalidSchedule)));
}

#[test]
fn test_get_milestone_schedule_returns_none_for_unconfigured_key() {
    let env = test_env_with_auths();
    let (client, _, _admin) = setup_with_admin(&env);
    let key_id = soroban_sdk::Address::generate(&env);

    assert!(client.get_milestone_schedule(&key_id).is_none());
}

#[test]
fn test_admin_can_overwrite_existing_schedule() {
    let env = test_env_with_auths();
    let (client, _, admin, key_id) = setup_with_schedule(&env);

    // Overwrite with different rewards.
    client.set_milestone_schedule(&admin, &key_id, &10, &20, &30, &40);
    let schedule = client.get_milestone_schedule(&key_id).unwrap();
    assert_eq!(schedule.milestones.get(0).unwrap().reward_amount, 10);
}
