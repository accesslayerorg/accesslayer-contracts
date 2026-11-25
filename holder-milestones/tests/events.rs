//! Tests for MilestoneRewardClaimed event emission.

mod contract_test_env;

use contract_test_env::{advance_time, enter_wallet, setup_with_schedule, test_env_with_auths, REWARD_30D};
use holder_milestones::{
    events::{MilestoneRewardClaimedEvent, MILESTONE_REWARD_CLAIMED_NAME},
    DEFAULT_DURATION_30D,
};
use soroban_sdk::{testutils::{Address as _, Events as _}, IntoVal};

#[test]
fn test_milestone_reward_claimed_event_is_emitted() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_30D);
    client.claim_milestone_reward(&wallet, &key_id, &0u32);

    let events = env.events().all();
    let matching: soroban_sdk::Vec<_> = events
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(0)
                .map(|t| t == MILESTONE_REWARD_CLAIMED_NAME.into_val(&env))
                .unwrap_or(false)
        })
        .collect();

    assert!(!matching.is_empty(), "MilestoneRewardClaimed event must be emitted");
}

#[test]
fn test_event_payload_contains_correct_wallet_key_milestone_and_amount() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, DEFAULT_DURATION_30D);
    client.claim_milestone_reward(&wallet, &key_id, &0u32);

    let events = env.events().all();
    let (_, _, data) = events.last().expect("at least one event");
    let payload: MilestoneRewardClaimedEvent = data.into_val(&env);

    assert_eq!(payload.wallet, wallet);
    assert_eq!(payload.key_id, key_id);
    assert_eq!(payload.milestone_index, 0u32);
    assert_eq!(payload.amount, REWARD_30D);
}

#[test]
fn test_separate_events_emitted_for_each_milestone() {
    let env = test_env_with_auths();
    let (client, _, _, key_id) = setup_with_schedule(&env);
    let wallet = enter_wallet(&env, &client, &key_id);

    advance_time(&env, holder_milestones::DEFAULT_DURATION_365D);
    for i in 0u32..4u32 {
        client.claim_milestone_reward(&wallet, &key_id, &i);
    }

    let events = env.events().all();
    let claimed_events: soroban_sdk::Vec<_> = events
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(0)
                .map(|t| t == MILESTONE_REWARD_CLAIMED_NAME.into_val(&env))
                .unwrap_or(false)
        })
        .collect();

    assert_eq!(claimed_events.len(), 4, "one event per milestone claim");
}
