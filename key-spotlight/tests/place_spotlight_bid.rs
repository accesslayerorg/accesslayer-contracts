//! Tests for `place_spotlight_bid` — validation, outbid logic, leader tracking.

mod contract_test_env;

use contract_test_env::{
    advance_time, future_start, setup_full, test_env_with_auths, DEFAULT_BID,
    DEFAULT_DURATION_DAYS, FUTURE_OFFSET_SECS,
};
use key_spotlight::{ContractError, MAX_DURATION_DAYS, SECS_PER_DAY, day_floor};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_first_bid_becomes_slot_leader() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let key_id = soroban_sdk::Address::generate(&env);
    let start = future_start(&env);

    let leader = client.place_spotlight_bid(&key_id, &creator, &start, &DEFAULT_DURATION_DAYS, &DEFAULT_BID);

    assert_eq!(leader.key_id, key_id);
    assert_eq!(leader.bid_amount, DEFAULT_BID);
    assert_eq!(leader.duration_days, DEFAULT_DURATION_DAYS);
}

#[test]
fn test_higher_bid_replaces_leader() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let start = future_start(&env);
    let slot_id = day_floor(start);

    let creator_a = soroban_sdk::Address::generate(&env);
    let key_a = soroban_sdk::Address::generate(&env);
    let creator_b = soroban_sdk::Address::generate(&env);
    let key_b = soroban_sdk::Address::generate(&env);

    client.place_spotlight_bid(&key_a, &creator_a, &start, &DEFAULT_DURATION_DAYS, &DEFAULT_BID);
    let leader = client.place_spotlight_bid(&key_b, &creator_b, &start, &DEFAULT_DURATION_DAYS, &(DEFAULT_BID + 1));

    assert_eq!(leader.key_id, key_b, "higher bid must win");
    assert_eq!(leader.bid_amount, DEFAULT_BID + 1);

    // Both bids appear in the bids list.
    let bids = client.get_spotlight_bids(&slot_id);
    assert_eq!(bids.len(), 2);
}

#[test]
fn test_equal_bid_does_not_replace_leader() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let start = future_start(&env);

    let creator_a = soroban_sdk::Address::generate(&env);
    let key_a = soroban_sdk::Address::generate(&env);
    let creator_b = soroban_sdk::Address::generate(&env);
    let key_b = soroban_sdk::Address::generate(&env);

    client.place_spotlight_bid(&key_a, &creator_a, &start, &DEFAULT_DURATION_DAYS, &DEFAULT_BID);
    let result = client.try_place_spotlight_bid(&key_b, &creator_b, &start, &DEFAULT_DURATION_DAYS, &DEFAULT_BID);
    assert_eq!(result, Err(Ok(ContractError::BidTooLow)));
}

#[test]
fn test_bid_lower_than_leader_is_rejected() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let start = future_start(&env);

    let creator_a = soroban_sdk::Address::generate(&env);
    let key_a = soroban_sdk::Address::generate(&env);
    let creator_b = soroban_sdk::Address::generate(&env);
    let key_b = soroban_sdk::Address::generate(&env);

    client.place_spotlight_bid(&key_a, &creator_a, &start, &DEFAULT_DURATION_DAYS, &1_000);
    let result = client.try_place_spotlight_bid(&key_b, &creator_b, &start, &DEFAULT_DURATION_DAYS, &999);
    assert_eq!(result, Err(Ok(ContractError::BidTooLow)));
}

#[test]
fn test_bid_in_past_is_rejected() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let key_id = soroban_sdk::Address::generate(&env);

    // Advance time so any timestamp ≤ now is in the past.
    advance_time(&env, SECS_PER_DAY * 2);
    let past_start = env.ledger().timestamp() - 1;

    let result = client.try_place_spotlight_bid(&key_id, &creator, &past_start, &DEFAULT_DURATION_DAYS, &DEFAULT_BID);
    assert_eq!(result, Err(Ok(ContractError::StartTimeInPast)));
}

#[test]
fn test_zero_duration_is_rejected() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let key_id = soroban_sdk::Address::generate(&env);

    let result = client.try_place_spotlight_bid(&key_id, &creator, &future_start(&env), &0u32, &DEFAULT_BID);
    assert_eq!(result, Err(Ok(ContractError::InvalidDuration)));
}

#[test]
fn test_duration_exceeding_max_is_rejected() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let key_id = soroban_sdk::Address::generate(&env);

    let result = client.try_place_spotlight_bid(
        &key_id, &creator, &future_start(&env), &(MAX_DURATION_DAYS + 1), &DEFAULT_BID,
    );
    assert_eq!(result, Err(Ok(ContractError::InvalidDuration)));
}

#[test]
fn test_bid_on_started_slot_is_rejected() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let key_id = soroban_sdk::Address::generate(&env);
    let start = future_start(&env);
    let slot_id = day_floor(start);

    // Advance ledger past the slot start.
    advance_time(&env, FUTURE_OFFSET_SECS + SECS_PER_DAY);

    let result = client.try_place_spotlight_bid(&key_id, &creator, &(slot_id + 1), &DEFAULT_DURATION_DAYS, &DEFAULT_BID);
    assert_eq!(result, Err(Ok(ContractError::SlotAlreadyStarted)));
}

#[test]
fn test_multiple_bids_from_same_creator_all_recorded() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let key_id = soroban_sdk::Address::generate(&env);
    let start = future_start(&env);
    let slot_id = day_floor(start);

    client.place_spotlight_bid(&key_id, &creator, &start, &DEFAULT_DURATION_DAYS, &100);
    client.place_spotlight_bid(&key_id, &creator, &start, &DEFAULT_DURATION_DAYS, &200);
    client.place_spotlight_bid(&key_id, &creator, &start, &DEFAULT_DURATION_DAYS, &300);

    let bids = client.get_spotlight_bids(&slot_id);
    assert_eq!(bids.len(), 3);

    let leader = client.get_slot_leader(&slot_id).unwrap();
    assert_eq!(leader.bid_amount, 300);
}
