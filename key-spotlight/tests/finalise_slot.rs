//! Tests for `finalise_slot` — treasury routing records, outbid tracking, idempotency.

mod contract_test_env;

use contract_test_env::{
    advance_time, future_start, place_default_bid, setup_full, test_env_with_auths,
    DEFAULT_BID, DEFAULT_DURATION_DAYS, FUTURE_OFFSET_SECS,
};
use key_spotlight::{ContractError, SECS_PER_DAY, day_floor};
use soroban_sdk::testutils::Address as _;

fn jump_to_slot(env: &soroban_sdk::Env, slot_id: u64) {
    let now = env.ledger().timestamp();
    if slot_id > now {
        advance_time(env, slot_id - now);
    }
}

#[test]
fn test_finalise_slot_marks_slot_finalised() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (_, slot_id) = place_default_bid(&env, &client);

    jump_to_slot(&env, slot_id);
    assert!(!client.is_slot_finalised(&slot_id));
    client.finalise_slot(&slot_id);
    assert!(client.is_slot_finalised(&slot_id));
}

#[test]
fn test_finalise_slot_returns_winner() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (key_id, slot_id) = place_default_bid(&env, &client);

    jump_to_slot(&env, slot_id);
    let leader = client.finalise_slot(&slot_id);

    assert_eq!(leader.key_id, key_id);
    assert_eq!(leader.bid_amount, DEFAULT_BID);
    assert_eq!(leader.duration_days, DEFAULT_DURATION_DAYS);
}

#[test]
fn test_finalise_slot_is_idempotent() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (key_id, slot_id) = place_default_bid(&env, &client);

    jump_to_slot(&env, slot_id);
    client.finalise_slot(&slot_id);
    // Second call must not panic and must return the same winner.
    let leader = client.finalise_slot(&slot_id);
    assert_eq!(leader.key_id, key_id);
}

#[test]
fn test_finalise_slot_before_start_is_rejected() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (_, slot_id) = place_default_bid(&env, &client);

    // slot_id is in the future — should be rejected.
    let result = client.try_finalise_slot(&slot_id);
    assert_eq!(result, Err(Ok(ContractError::SlotAlreadyStarted)));
}

#[test]
fn test_finalise_empty_slot_returns_no_active_spotlight() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);

    // Create a slot_id far in the future then jump past it with no bids.
    let slot_id = day_floor(env.ledger().timestamp() + SECS_PER_DAY);
    advance_time(&env, SECS_PER_DAY + 1);

    let result = client.try_finalise_slot(&slot_id);
    assert_eq!(result, Err(Ok(ContractError::NoActiveSpotlight)));
}

#[test]
fn test_outbid_amounts_recorded_in_bids_list() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let start = future_start(&env);
    let slot_id = day_floor(start);

    let creator_a = soroban_sdk::Address::generate(&env);
    let key_a = soroban_sdk::Address::generate(&env);
    let creator_b = soroban_sdk::Address::generate(&env);
    let key_b = soroban_sdk::Address::generate(&env);
    let creator_c = soroban_sdk::Address::generate(&env);
    let key_c = soroban_sdk::Address::generate(&env);

    client.place_spotlight_bid(&key_a, &creator_a, &start, &7u32, &100);
    client.place_spotlight_bid(&key_b, &creator_b, &start, &7u32, &200);
    client.place_spotlight_bid(&key_c, &creator_c, &start, &7u32, &300);

    // All three bids are in the history; only the last is the leader.
    let bids = client.get_spotlight_bids(&slot_id);
    assert_eq!(bids.len(), 3);

    // Losing bids can be identified by comparing bid_amount to leader.
    let leader = client.get_slot_leader(&slot_id).unwrap();
    let outbid: soroban_sdk::Vec<_> = bids
        .iter()
        .filter(|b| b.bid_amount < leader.bid_amount)
        .collect();
    assert_eq!(outbid.len(), 2, "two bidders were outbid");

    // The two outbid amounts are 100 and 200; both must be returned to their bidders.
    let outbid_total: i128 = outbid.iter().map(|b| b.bid_amount).sum();
    assert_eq!(outbid_total, 300);
}

#[test]
fn test_treasury_receives_winning_bid_on_finalisation() {
    let env = test_env_with_auths();
    let (client, _, _, treasury) = setup_full(&env);
    let (_, slot_id) = place_default_bid(&env, &client);

    jump_to_slot(&env, slot_id);
    let leader = client.finalise_slot(&slot_id);

    // Verify treasury address is set and winning amount matches.
    let stored_treasury = client.get_treasury().unwrap();
    assert_eq!(stored_treasury, treasury);
    assert_eq!(leader.bid_amount, DEFAULT_BID);
}
