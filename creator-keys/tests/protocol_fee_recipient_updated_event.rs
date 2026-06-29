//! Field-level assertions for the `ProtocolFeeRecipientUpdated` event.
//!
//! Each test asserts exactly one field of the event payload so that a regression
//! on any single field produces a focused, descriptive failure.
//!
//! Event shape emitted by `update_protocol_fee_recipient`:
//! - topics: `(PROTOCOL_FEE_RECIPIENT_UPDATED_EVENT_NAME, admin)`
//! - data:   `ProtocolFeeRecipientUpdatedEvent { old_recipient, new_recipient }`

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal,
};

fn setup_update(
    env: &Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    Address,
    Address,
    Address,
) {
    let (client, _) = register_creator_keys(env);
    let admin = Address::generate(env);
    let old_recipient = Address::generate(env);
    let new_recipient = Address::generate(env);

    client.set_protocol_admin(&admin, &admin);
    client.set_protocol_fee_recipient(&admin, &old_recipient);
    client.update_protocol_fee_recipient(&admin, &new_recipient);

    (client, admin, old_recipient, new_recipient)
}

fn last_event_data(env: &Env) -> events::ProtocolFeeRecipientUpdatedEvent {
    let event_log = env.events().all();
    let (_, _, data) = event_log.last().expect("at least one event must exist");
    data.into_val(env)
}

#[test]
fn test_protocol_fee_recipient_updated_event_old_recipient_field() {
    let env = test_env_with_auths();
    let (_, _, old_recipient, _) = setup_update(&env);

    let payload = last_event_data(&env);
    assert_eq!(
        payload.old_recipient, old_recipient,
        "old_recipient field must match the address stored before the update"
    );
}

#[test]
fn test_protocol_fee_recipient_updated_event_new_recipient_field() {
    let env = test_env_with_auths();
    let (_, _, _, new_recipient) = setup_update(&env);

    let payload = last_event_data(&env);
    assert_eq!(
        payload.new_recipient, new_recipient,
        "new_recipient field must match the address passed to update_protocol_fee_recipient"
    );
}

#[test]
fn test_protocol_fee_recipient_updated_event_emitted_once_per_update() {
    let env = test_env_with_auths();
    let (_, _, old_recipient, _) = setup_update(&env);

    let all_events = env.events().all();
    let update_event_count = all_events
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
        .count();

    assert_eq!(
        update_event_count, 1,
        "update_protocol_fee_recipient must emit exactly one event per update call; \
         old_recipient={old_recipient:?}"
    );
}
