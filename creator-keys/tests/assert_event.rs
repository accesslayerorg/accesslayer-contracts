//! Unit tests for the `assert_event` test helper.
//!
//! Confirms the helper:
//! - Passes silently when contract ID, topic, and all fields match.
//! - Panics when the topic does not match (filters by topic).
//! - Panics when the contract ID does not match (filters by contract).
//! - Panics when a field value mismatches, displaying the topic, expected, and actual fields.
//! - Panics when no events have been emitted at all.

mod contract_test_env;

use contract_test_env::{assert_event, register_creator_keys, test_env_with_auths};
use creator_keys::events;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    Address, Env,
};
use std::panic;

fn setup_initialized_contract(
    env: &Env,
) -> (Address, Address, Address, events::ContractInitializedEvent) {
    let (client, contract_id) = register_creator_keys(env);
    let admin = Address::generate(env);
    let recipient = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    client.set_protocol_fee_recipient(&admin, &recipient);

    let test_ledger = 42u32;
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = test_ledger;
    env.ledger().set(ledger_info);

    env.events().all();
    client.set_fee_config(&admin, &9000u32, &1000u32);

    let expected = events::ContractInitializedEvent {
        admin,
        protocol_fee_bps: 1000u32,
        protocol_fee_recipient: recipient,
        initialized_at_ledger: test_ledger,
    };

    (
        contract_id,
        expected.admin.clone(),
        expected.protocol_fee_recipient.clone(),
        expected,
    )
}

#[test]
fn test_assert_event_matching_succeeds() {
    let env = test_env_with_auths();
    let (contract_id, _, _, expected) = setup_initialized_contract(&env);

    assert_event(
        &env,
        &contract_id,
        events::CONTRACT_INITIALIZED_EVENT_NAME,
        &expected,
    );
}

#[test]
fn test_assert_event_filters_out_wrong_topic() {
    let env = test_env_with_auths();
    let (contract_id, _, _, expected) = setup_initialized_contract(&env);

    let wrong_topic = symbol_short!("wrong_top");

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        assert_event(&env, &contract_id, wrong_topic, &expected);
    }));

    assert!(
        result.is_err(),
        "assert_event should panic when topic does not match"
    );
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<std::string::String>()
        .cloned()
        .or_else(|| {
            err.downcast_ref::<&str>()
                .map(|s| std::string::String::from(*s))
        })
        .unwrap_or_default();

    assert!(
        msg.contains("no matching event found"),
        "panic message should state that no matching event was found: {msg}"
    );
    assert!(
        msg.contains("wrong_top"),
        "panic message should mention the queried topic: {msg}"
    );
}

#[test]
fn test_assert_event_filters_out_wrong_contract() {
    let env = test_env_with_auths();
    let (_contract_id, _, _, expected) = setup_initialized_contract(&env);
    let wrong_contract_id = Address::generate(&env);

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        assert_event(
            &env,
            &wrong_contract_id,
            events::CONTRACT_INITIALIZED_EVENT_NAME,
            &expected,
        );
    }));

    assert!(
        result.is_err(),
        "assert_event should panic when contract_id does not match"
    );
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<std::string::String>()
        .cloned()
        .or_else(|| {
            err.downcast_ref::<&str>()
                .map(|s| std::string::String::from(*s))
        })
        .unwrap_or_default();

    assert!(
        msg.contains("no matching event found"),
        "panic message should state that no matching event was found: {msg}"
    );
}

#[test]
fn test_assert_event_fails_on_field_mismatch() {
    let env = test_env_with_auths();
    let (contract_id, _, _, mut expected) = setup_initialized_contract(&env);

    // Tamper with one field
    expected.protocol_fee_bps = 9999u32;

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        assert_event(
            &env,
            &contract_id,
            events::CONTRACT_INITIALIZED_EVENT_NAME,
            &expected,
        );
    }));

    assert!(
        result.is_err(),
        "assert_event should panic on field value mismatch"
    );
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<std::string::String>()
        .cloned()
        .or_else(|| {
            err.downcast_ref::<&str>()
                .map(|s| std::string::String::from(*s))
        })
        .unwrap_or_default();

    assert!(
        msg.contains("event field mismatch"),
        "panic message should indicate field mismatch: {msg}"
    );
    assert!(
        msg.contains("init"),
        "panic message should include the topic: {msg}"
    );
    assert!(
        msg.contains("9999"),
        "panic message should include expected fields with the tampered value: {msg}"
    );
    assert!(
        msg.contains("1000"),
        "panic message should include actual fields with the real value: {msg}"
    );
}

#[test]
fn test_assert_event_panics_when_no_events_at_all() {
    let env = test_env_with_auths();
    let contract_id = Address::generate(&env);
    let expected = events::ContractInitializedEvent {
        admin: Address::generate(&env),
        protocol_fee_bps: 1000u32,
        protocol_fee_recipient: Address::generate(&env),
        initialized_at_ledger: 1,
    };

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        assert_event(
            &env,
            &contract_id,
            events::CONTRACT_INITIALIZED_EVENT_NAME,
            &expected,
        );
    }));

    assert!(
        result.is_err(),
        "assert_event should panic when no events exist"
    );
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<std::string::String>()
        .cloned()
        .or_else(|| {
            err.downcast_ref::<&str>()
                .map(|s| std::string::String::from(*s))
        })
        .unwrap_or_default();

    assert!(
        msg.contains("no matching event found"),
        "panic message should state that no matching event was found: {msg}"
    );
}
