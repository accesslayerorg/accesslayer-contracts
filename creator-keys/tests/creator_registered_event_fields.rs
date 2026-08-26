//! Integration tests for issue #706 — CreatorRegistered event emitted on
//! successful registration must carry all required fields with correct values
//! and types.
//!
//! Acceptance criteria:
//! - All four required fields present on successful registration
//! - No event emitted on failed registration (duplicate wallet)
//! - Field types are correct (Address, String, Address, u32)
//! - `registered_at_ledger` matches current ledger sequence at registration time

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_ledger_sequence, test_env_with_auths};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, String, Symbol,
};

/// Decode the last `CreatorRegisteredEvent` from the event log.
fn last_registration_event(env: &soroban_sdk::Env) -> events::CreatorRegisteredEvent {
    let log = env.events().all();
    let (_, _, data) = log
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(env);
                    name == events::REGISTER_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("no CreatorRegistered event in log");
    data.into_val(env)
}

/// Count `CreatorRegistered` events in the log.
fn registration_event_count(env: &soroban_sdk::Env) -> usize {
    env.events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(env);
                    name == events::REGISTER_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .count()
}

fn register_creator(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    env: &soroban_sdk::Env,
    creator: &Address,
    handle: &str,
) {
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

// ── AC1: all four required fields present with correct values ─────────────────

#[test]
fn test_creator_registered_event_emitted_on_successful_registration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    register_creator(&client, &env, &creator, "alice");

    let count = registration_event_count(&env);
    assert_eq!(
        count, 1,
        "exactly one CreatorRegistered event should be emitted"
    );
}

#[test]
fn test_creator_registered_event_creator_wallet_field_matches_registered_address() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    register_creator(&client, &env, &creator, "bob");

    let payload = last_registration_event(&env);
    assert_eq!(
        payload.creator, creator,
        "creator field must equal the registered wallet address"
    );
}

#[test]
fn test_creator_registered_event_display_name_field_matches_handle() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle_str = "carol_handle";
    register_creator(&client, &env, &creator, handle_str);

    let payload = last_registration_event(&env);
    assert_eq!(
        payload.handle,
        String::from_str(&env, handle_str),
        "handle field must equal the display name passed at registration"
    );
}

#[test]
fn test_creator_registered_event_fee_recipient_defaults_to_creator_wallet() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    register_creator(&client, &env, &creator, "dave");

    let payload = last_registration_event(&env);
    assert_eq!(
        payload.fee_recipient, creator,
        "fee_recipient must default to the creator wallet at registration"
    );
}

#[test]
fn test_creator_registered_event_registered_at_ledger_matches_current_ledger() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let expected_ledger: u32 = 42;
    set_ledger_sequence(&env, expected_ledger);

    let creator = Address::generate(&env);
    register_creator(&client, &env, &creator, "eve");

    let payload = last_registration_event(&env);
    assert_eq!(
        payload.registered_at_ledger, expected_ledger,
        "registered_at_ledger must equal the ledger sequence at registration time"
    );
}

#[test]
fn test_creator_registered_event_all_four_fields_present_in_one_registration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let ledger_seq: u32 = 100;
    set_ledger_sequence(&env, ledger_seq);

    let creator = Address::generate(&env);
    let handle_str = "frank";
    register_creator(&client, &env, &creator, handle_str);

    let payload = last_registration_event(&env);

    assert_eq!(
        payload.creator, creator,
        "creator (creator_wallet) field mismatch"
    );
    assert_eq!(
        payload.handle,
        String::from_str(&env, handle_str),
        "handle (display_name) field mismatch"
    );
    assert_eq!(
        payload.fee_recipient, creator,
        "fee_recipient field mismatch — must default to creator wallet"
    );
    assert_eq!(
        payload.registered_at_ledger, ledger_seq,
        "registered_at_ledger field mismatch"
    );
}

// ── AC2: no event emitted on failed registration (duplicate wallet) ───────────

#[test]
fn test_no_creator_registered_event_emitted_on_duplicate_registration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);

    // First registration succeeds and emits exactly one event
    register_creator(&client, &env, &creator, "grace");
    assert_eq!(
        registration_event_count(&env),
        1,
        "successful registration must emit exactly one CreatorRegistered event"
    );

    // Duplicate registration must fail — event log resets per call
    let result = client.try_register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "grace_duplicate"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    assert!(result.is_err(), "duplicate registration must fail");

    // After the failed call the event log reflects only that call — no registration event
    assert_eq!(
        registration_event_count(&env),
        0,
        "no CreatorRegistered event must be emitted in a failed registration call"
    );
}

// ── AC3: field types are correct ──────────────────────────────────────────────

#[test]
fn test_creator_registered_event_creator_field_is_address_type() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    register_creator(&client, &env, &creator, "heidi");

    // Decoding into CreatorRegisteredEvent succeeds — Address fields compile-checked
    let payload = last_registration_event(&env);
    // Compile-time proof: these are Address values
    let _: Address = payload.creator;
    let _: Address = payload.fee_recipient;
}

#[test]
fn test_creator_registered_event_handle_field_is_string_type() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    register_creator(&client, &env, &creator, "ivan");

    let payload = last_registration_event(&env);
    let _: soroban_sdk::String = payload.handle;
}

#[test]
fn test_creator_registered_event_registered_at_ledger_field_is_u32_type() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    register_creator(&client, &env, &creator, "judy");

    let payload = last_registration_event(&env);
    let _: u32 = payload.registered_at_ledger;
}

// ── AC4: registered_at_ledger reflects the ledger at registration time ────────

#[test]
fn test_registered_at_ledger_reflects_registration_ledger_not_later_buy_ledger() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let registration_ledger: u32 = 50;
    set_ledger_sequence(&env, registration_ledger);

    let creator = Address::generate(&env);
    register_creator(&client, &env, &creator, "kevin");

    // Capture the event immediately — event log is scoped to the current call
    let reg_payload = last_registration_event(&env);
    assert_eq!(
        reg_payload.registered_at_ledger, registration_ledger,
        "registered_at_ledger must equal the ledger sequence at registration time"
    );

    // Advance ledger and perform a buy in a separate call
    let buy_ledger: u32 = registration_ledger + 10;
    set_ledger_sequence(&env, buy_ledger);
    let admin = Address::generate(&env);
    client.set_key_price(&admin, &1_000);
    client.buy_key(&creator, &Address::generate(&env), &1_000, &None);

    // The buy event carries the advanced ledger, proving each event snapshots its
    // own call's ledger independently of previous registrations
    let buy_payload: events::KeysBoughtEvent = env
        .events()
        .all()
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::BUY_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .map(|(_, _, data)| data.into_val(&env))
        .expect("buy event must be present after buy_key call");

    assert_eq!(
        buy_payload.ledger, buy_ledger,
        "buy event ledger must reflect the ledger at buy time"
    );
    assert_ne!(
        buy_payload.ledger, registration_ledger,
        "buy ledger and registration ledger must differ"
    );
}
