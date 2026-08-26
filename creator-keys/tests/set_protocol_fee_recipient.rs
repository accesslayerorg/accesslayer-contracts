//! Tests for the `set_protocol_fee_recipient` entrypoint.
//!
//! Verifies that:
//! - a zero address is rejected with `ContractError::ZeroAddress`
//! - a valid non-zero address is accepted and stored

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_set_protocol_fee_recipient_rejects_zero_address() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let zero_str = String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero_addr = Address::from_string(&zero_str);

    let result = client.try_set_protocol_fee_recipient(&admin, &zero_addr);
    assert_eq!(
        result,
        Err(Ok(ContractError::ZeroAddress)),
        "zero address must be rejected"
    );

    // Confirm nothing was stored.
    assert_eq!(
        client.get_protocol_fee_recipient(),
        None,
        "protocol fee recipient should remain unset after rejection"
    );
}

#[test]
fn test_set_protocol_fee_recipient_accepts_valid_address() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let recipient = Address::generate(&env);

    let result = client.try_set_protocol_fee_recipient(&admin, &recipient);
    assert_eq!(result, Ok(Ok(())), "valid address should be accepted");

    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(recipient),
        "stored recipient should match"
    );
}

#[test]
fn test_set_protocol_fee_recipient_idempotent() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let recipient = Address::generate(&env);

    client.set_protocol_fee_recipient(&admin, &recipient);
    // Setting the same value again should be a no-op.
    client.set_protocol_fee_recipient(&admin, &recipient);

    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(recipient),
        "recipient unchanged after idempotent set"
    );
}

#[test]
fn test_set_protocol_fee_recipient_emits_event_on_update() {
    use creator_keys::events;
    use soroban_sdk::{testutils::Events, IntoVal};

    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let old_recipient = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    client.set_protocol_fee_recipient(&admin, &old_recipient);
    client.set_protocol_fee_recipient(&admin, &new_recipient);

    let all_events = env.events().all();
    let update_events: Vec<_> = all_events
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let sym: soroban_sdk::Symbol = v.into_val(&env);
                    sym == events::PROTOCOL_FEE_RECIPIENT_UPDATED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .collect();

    assert_eq!(
        update_events.len(),
        1,
        "updating fee recipient via set_protocol_fee_recipient must emit exactly one event"
    );

    let (_, _, data) = update_events.last().unwrap();
    let payload: events::ProtocolFeeRecipientUpdatedEvent = data.into_val(&env);

    assert_eq!(payload.old_recipient, old_recipient);
    assert_eq!(payload.new_recipient, new_recipient);
}
