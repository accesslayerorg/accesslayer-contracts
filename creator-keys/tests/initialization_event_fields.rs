//! Tests for the initialization event confirming all fields match the
//! initialization arguments.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, String,
};

#[test]
fn test_initialization_event_admin_matches_argument() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "test_handle");

    client.set_fee_config(&admin, &9_000u32, &1_000u32);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: handle.clone(),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let all_events = env.events().all();
    let registration_event = all_events
        .last()
        .expect("should have emitted a registration event");

    let payload: events::CreatorRegisteredEvent = registration_event.2.into_val(&env);

    assert_eq!(payload.creator, creator, "admin field mismatch");
}

#[test]
fn test_initialization_event_protocol_fee_bps_matches_argument() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "test_handle");

    let expected_creator_bps = 8_500u32;
    let expected_protocol_bps = 1_500u32;
    client.set_fee_config(&admin, &expected_creator_bps, &expected_protocol_bps);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: handle.clone(),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let all_events = env.events().all();
    let registration_event = all_events
        .last()
        .expect("should have emitted a registration event");

    let payload: events::CreatorRegisteredEvent = registration_event.2.into_val(&env);

    assert_eq!(
        payload.protocol_bps, expected_protocol_bps,
        "protocol_fee_bps field mismatch"
    );
}

#[test]
fn test_initialization_event_protocol_fee_recipient_matches_argument() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "test_handle");

    client.set_fee_config(&admin, &9_000u32, &1_000u32);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: handle.clone(),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let all_events = env.events().all();
    let registration_event = all_events
        .last()
        .expect("should have emitted a registration event");

    let payload: events::CreatorRegisteredEvent = registration_event.2.into_val(&env);

    assert_eq!(
        payload.creator, creator,
        "protocol_fee_recipient field mismatch"
    );
}

#[test]
fn test_initialization_event_initialized_at_ledger_matches_current() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "test_handle");

    let current_ledger = env.ledger().sequence();

    client.set_fee_config(&admin, &9_000u32, &1_000u32);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: handle.clone(),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let all_events = env.events().all();
    let registration_event = all_events
        .last()
        .expect("should have emitted a registration event");

    let payload: events::CreatorRegisteredEvent = registration_event.2.into_val(&env);

    assert_eq!(payload.supply, 0, "supply should be 0 at initialization");
    assert_eq!(
        payload.holder_count, 0,
        "holder_count should be 0 at initialization"
    );

    let profile = client.get_creator(&creator);
    assert_eq!(profile.registered_at, current_ledger);
}

#[test]
fn test_initialization_event_only_one_emitted() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "test_handle");

    let events_before = env.events().all().len();

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: handle.clone(),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let events_after = env.events().all().len();
    assert_eq!(
        events_after - events_before,
        1,
        "registration should emit exactly one event"
    );
}
