//! Tests for SpotlightBidPlaced and SpotlightAwarded event emission.

mod contract_test_env;

use contract_test_env::{
    advance_time, future_start, place_default_bid, setup_full, test_env_with_auths,
    DEFAULT_BID, DEFAULT_DURATION_DAYS, FUTURE_OFFSET_SECS,
};
use key_spotlight::{
    events::{
        SpotlightAwardedEvent, SpotlightBidPlacedEvent, SPOTLIGHT_AWARDED_NAME,
        SPOTLIGHT_BID_PLACED_NAME,
    },
    day_floor, SECS_PER_DAY,
};
use soroban_sdk::{testutils::{Address as _, Events as _}, IntoVal};

fn jump_to_slot(env: &soroban_sdk::Env, slot_id: u64) {
    let now = env.ledger().timestamp();
    if slot_id > now {
        advance_time(env, slot_id - now);
    }
}

#[test]
fn test_spotlight_bid_placed_event_emitted() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (_, _slot_id) = place_default_bid(&env, &client);

    let events = env.events().all();
    let matching: soroban_sdk::Vec<_> = events
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(0)
                .map(|t| t == SPOTLIGHT_BID_PLACED_NAME.into_val(&env))
                .unwrap_or(false)
        })
        .collect();

    assert!(!matching.is_empty(), "SpotlightBidPlaced must be emitted");
}

#[test]
fn test_spotlight_bid_placed_event_data_is_correct() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let key_id = soroban_sdk::Address::generate(&env);
    let start = future_start(&env);

    client.place_spotlight_bid(&key_id, &creator, &start, &DEFAULT_DURATION_DAYS, &DEFAULT_BID);

    let events = env.events().all();
    let (_, _, data) = events.last().unwrap();
    let payload: SpotlightBidPlacedEvent = data.into_val(&env);

    assert_eq!(payload.key_id, key_id);
    assert_eq!(payload.bid_amount, DEFAULT_BID);
    assert_eq!(payload.bidder, creator);
    assert_eq!(payload.slot_id, day_floor(start));
}

#[test]
fn test_spotlight_awarded_event_emitted_on_finalise() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (_, slot_id) = place_default_bid(&env, &client);

    jump_to_slot(&env, slot_id);
    client.finalise_slot(&slot_id);

    let events = env.events().all();
    let matching: soroban_sdk::Vec<_> = events
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(0)
                .map(|t| t == SPOTLIGHT_AWARDED_NAME.into_val(&env))
                .unwrap_or(false)
        })
        .collect();

    assert!(!matching.is_empty(), "SpotlightAwarded must be emitted");
}

#[test]
fn test_spotlight_awarded_event_data_is_correct() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (key_id, slot_id) = place_default_bid(&env, &client);

    jump_to_slot(&env, slot_id);
    client.finalise_slot(&slot_id);

    let events = env.events().all();
    let (_, _, data) = events.last().unwrap();
    let payload: SpotlightAwardedEvent = data.into_val(&env);

    assert_eq!(payload.key_id, key_id);
    assert_eq!(payload.slot_id, slot_id);
    assert_eq!(payload.winning_amount, DEFAULT_BID);
    assert_eq!(payload.duration_days, DEFAULT_DURATION_DAYS);
}

#[test]
fn test_spotlight_awarded_not_emitted_twice_on_idempotent_finalise() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (_, slot_id) = place_default_bid(&env, &client);

    jump_to_slot(&env, slot_id);
    client.finalise_slot(&slot_id);

    let count_after_first = env
        .events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(0)
                .map(|t| t == SPOTLIGHT_AWARDED_NAME.into_val(&env))
                .unwrap_or(false)
        })
        .count();

    client.finalise_slot(&slot_id); // idempotent second call

    let count_after_second = env
        .events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(0)
                .map(|t| t == SPOTLIGHT_AWARDED_NAME.into_val(&env))
                .unwrap_or(false)
        })
        .count();

    assert_eq!(
        count_after_first, count_after_second,
        "SpotlightAwarded must not be emitted again on idempotent finalise"
    );
}
