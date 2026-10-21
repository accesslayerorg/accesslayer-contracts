//! Tests for `get_active_spotlight` — active window, expiry, remaining time.

mod contract_test_env;

use contract_test_env::{
    advance_time, future_start, place_default_bid, setup_full, test_env_with_auths,
    DEFAULT_DURATION_DAYS, FUTURE_OFFSET_SECS,
};
use key_spotlight::{ContractError, SECS_PER_DAY, day_floor};
use soroban_sdk::testutils::Address as _;

/// Advance the ledger to exactly the slot start.
fn jump_to_slot(env: &soroban_sdk::Env, slot_id: u64) {
    let now = env.ledger().timestamp();
    if slot_id > now {
        advance_time(env, slot_id - now);
    }
}

#[test]
fn test_get_active_spotlight_returns_error_before_finalisation() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (_, slot_id) = place_default_bid(&env, &client);

    // Not yet finalised.
    let result = client.try_get_active_spotlight(&slot_id);
    assert_eq!(result, Err(Ok(ContractError::NoActiveSpotlight)));
}

#[test]
fn test_get_active_spotlight_succeeds_after_finalisation() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (key_id, slot_id) = place_default_bid(&env, &client);

    jump_to_slot(&env, slot_id);
    client.finalise_slot(&slot_id);

    let view = client.get_active_spotlight(&slot_id);
    assert_eq!(view.key_id, key_id);
    assert_eq!(view.slot_id, slot_id);
    assert_eq!(view.duration_days, DEFAULT_DURATION_DAYS);
    assert!(view.remaining_secs > 0);
}

#[test]
fn test_remaining_secs_decreases_with_time() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (_, slot_id) = place_default_bid(&env, &client);

    jump_to_slot(&env, slot_id);
    client.finalise_slot(&slot_id);

    let view_before = client.get_active_spotlight(&slot_id);
    advance_time(&env, SECS_PER_DAY);
    let view_after = client.get_active_spotlight(&slot_id);

    assert!(
        view_after.remaining_secs < view_before.remaining_secs,
        "remaining_secs must decrease as time passes"
    );
}

#[test]
fn test_get_active_spotlight_returns_error_after_window_expires() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let (_, slot_id) = place_default_bid(&env, &client);

    jump_to_slot(&env, slot_id);
    client.finalise_slot(&slot_id);

    // Advance past the full duration window.
    advance_time(&env, DEFAULT_DURATION_DAYS as u64 * SECS_PER_DAY + 1);

    let result = client.try_get_active_spotlight(&slot_id);
    assert_eq!(result, Err(Ok(ContractError::NoActiveSpotlight)));
}

#[test]
fn test_get_active_spotlight_winner_is_highest_bidder() {
    let env = test_env_with_auths();
    let (client, _, _, _) = setup_full(&env);
    let start = future_start(&env);
    let slot_id = day_floor(start);

    let creator_a = soroban_sdk::Address::generate(&env);
    let key_a = soroban_sdk::Address::generate(&env);
    let creator_b = soroban_sdk::Address::generate(&env);
    let key_b = soroban_sdk::Address::generate(&env);

    client.place_spotlight_bid(&key_a, &creator_a, &start, &7u32, &500);
    client.place_spotlight_bid(&key_b, &creator_b, &start, &7u32, &750);

    jump_to_slot(&env, slot_id);
    client.finalise_slot(&slot_id);

    let view = client.get_active_spotlight(&slot_id);
    assert_eq!(view.key_id, key_b, "highest bidder must win");
    assert_eq!(view.bid_amount, 750);
}
