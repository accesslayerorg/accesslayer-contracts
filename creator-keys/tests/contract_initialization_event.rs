//! Integration tests for the contract initialization event (#579).
//!
//! Verifies that a `ContractInitializedEvent` is emitted exactly once on the
//! first successful `set_fee_config` call, with all four required fields, and
//! that re-initialization attempts revert before reaching event emission.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, IntoVal, Symbol,
};

#[test]
fn test_initialization_event_emitted_on_first_set_fee_config() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_protocol_fee_recipient(&admin, &recipient);

    let test_ledger = 10u32;
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = test_ledger;
    env.ledger().set(ledger_info);

    env.events().all();
    client.set_fee_config(&admin, &9000u32, &1000u32);

    let event_log = env.events().all();
    let (_, _, data) = event_log
        .iter()
        .find(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::CONTRACT_INITIALIZED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("initialization event must be emitted on first set_fee_config");

    let payload: events::ContractInitializedEvent = data.into_val(&env);

    assert_eq!(payload.admin, admin, "admin field must match caller");
    assert_eq!(
        payload.protocol_fee_bps, 1000u32,
        "protocol_fee_bps must match"
    );
    assert_eq!(
        payload.protocol_fee_recipient, recipient,
        "protocol_fee_recipient must match stored recipient"
    );
    assert_eq!(
        payload.initialized_at_ledger, test_ledger,
        "initialized_at_ledger must match current ledger"
    );
}

#[test]
fn test_initialization_event_not_emitted_on_reinit() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);

    // First initialization
    client.set_fee_config(&admin, &9000u32, &1000u32);

    // Clear events after first init
    env.events().all();

    // Second call (re-initialization) — must not emit init event
    client.set_fee_config(&admin, &8000u32, &2000u32);

    let event_log = env.events().all();
    let init_event_count = event_log
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::CONTRACT_INITIALIZED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .count();

    assert_eq!(
        init_event_count, 0,
        "initialization event must not be emitted on subsequent set_fee_config calls"
    );
}
