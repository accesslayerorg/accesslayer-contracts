//! Detailed field value assertions for creator registration events.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, String,
};

#[test]
fn test_register_creator_event_field_values_match_fixtures() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // 1. Setup deterministic fixtures
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let handle_str = "fixture_handle";
    let handle = String::from_str(&env, handle_str);

    // 2. Set global fee config so event has non-zero values
    let expected_creator_bps = 9500;
    let expected_protocol_bps = 500;
    client.set_fee_config(&admin, &expected_creator_bps, &expected_protocol_bps);

    // 3. Trigger registration
    client.register_creator(&creator, &handle, &None, &None);

    // 4. Capture the emitted event
    let all_events = env.events().all();
    let registration_event = all_events
        .last()
        .expect("should have emitted a registration event");

    // 5. Decode payload
    let payload: events::CreatorRegisteredEvent = registration_event.2.into_val(&env);

    // 6. Assert each field individually for clear failure messages
    assert_eq!(
        payload.creator, creator,
        "Registration event 'creator' field mismatch"
    );

    assert_eq!(
        payload.handle, handle,
        "Registration event 'handle' field mismatch"
    );
    // Verify the handle matches the original string value
    assert_eq!(
        payload.handle,
        String::from_str(&env, handle_str),
        "Registration event 'handle' does not match fixture string"
    );

    assert_eq!(
        payload.supply, 0,
        "Registration event 'supply' should be 0 at registration"
    );

    assert_eq!(
        payload.holder_count, 0,
        "Registration event 'holder_count' should be 0 at registration"
    );

    assert_eq!(
        payload.creator_bps, expected_creator_bps,
        "Registration event 'creator_bps' mismatch"
    );

    assert_eq!(
        payload.protocol_bps, expected_protocol_bps,
        "Registration event 'protocol_bps' mismatch"
    );
}

#[test]
fn test_register_creator_event_fields_update_with_fee_config() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);

    // First registration with one config
    client.set_fee_config(&admin, &9000, &1000);
    let creator1 = Address::generate(&env);
    client.register_creator(
        &creator1,
        &String::from_str(&env, "creator_1"),
        &None,
        &None,
    );

    let event1: events::CreatorRegisteredEvent =
        env.events().all().last().unwrap().2.into_val(&env);
    assert_eq!(event1.creator_bps, 9000);
    assert_eq!(event1.protocol_bps, 1000);

    // Second registration after fee config update
    client.set_fee_config(&admin, &8000, &2000);
    let creator2 = Address::generate(&env);
    client.register_creator(
        &creator2,
        &String::from_str(&env, "creator_2"),
        &None,
        &None,
    );

    let event2: events::CreatorRegisteredEvent =
        env.events().all().last().unwrap().2.into_val(&env);
    assert_eq!(event2.creator_bps, 8000);
    assert_eq!(event2.protocol_bps, 2000);
}
