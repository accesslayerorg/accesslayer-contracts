//! Unit tests for structured log event emitted when creator key TTL extension is triggered (#553).
//!
//! Scope verified:
//! - Soroban contract event emitted on TTL extension carrying:
//!   `creator_id`, `extended_at_ledger`, and `new_expiry_ledger`.
//! - Dedicated topic (`TTL_EXTENDED_EVENT_NAME` = "ttl_ext") allowing indexers to filter without scanning all events.
//! - Event appears in the simulated transaction event log / result meta.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_key_price_for_tests};
use creator_keys::constants::storage;
use creator_keys::events::{self, ttl_extended_topics};
use creator_keys::CREATOR_TTL_LEDGERS;
use soroban_sdk::testutils::storage::Persistent;
use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{Address, IntoVal};

const KEY_PRICE: i128 = 100;

fn creator_ttl_remaining(env: &soroban_sdk::Env, contract_id: &Address, creator: &Address) -> u32 {
    let key = storage::creator(creator);
    env.as_contract(contract_id, || env.storage().persistent().get_ttl(&key))
}

#[test]
fn test_ttl_extension_structured_event_appears_in_transaction_meta() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();

    let (client, contract_id) = register_creator_keys(&env);
    env.deployer().extend_ttl(
        contract_id.clone(),
        CREATOR_TTL_LEDGERS,
        CREATOR_TTL_LEDGERS,
    );
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    // Drain TTL below threshold to trigger extension on next trade
    let mut ledger = env.ledger().get();
    ledger.sequence_number += ttl_before.saturating_sub(1).max(1);
    env.ledger().set(ledger);

    let current_ledger = env.ledger().sequence();
    let expected_new_expiry = current_ledger + CREATOR_TTL_LEDGERS;

    // Execute trade to trigger extension
    let res = client.try_buy_key(&creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(res, Ok(Ok(1)));

    // Verify structured event appears in transaction event meta
    let events = env.events().all();
    let matching_event = events
        .iter()
        .rev()
        .find(|(_, topics, _)| topics == &ttl_extended_topics(&creator).into_val(&env));

    assert!(
        matching_event.is_some(),
        "TTL extension event must appear in transaction meta under dedicated topic"
    );

    let (_, topics, data) = matching_event.unwrap();

    // Verify dedicated topic
    let topic_sym: soroban_sdk::Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(
        topic_sym,
        events::TTL_EXTENDED_EVENT_NAME,
        "first topic must be dedicated TTL_EXTENDED_EVENT_NAME"
    );

    let topic_creator: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(
        topic_creator, creator,
        "second topic must be the creator address"
    );

    // Verify event payload fields: creator_id, extended_at_ledger, new_expiry_ledger
    let payload: events::TtlExtendedEvent = data.into_val(&env);
    assert_eq!(payload.creator_id, creator, "creator_id field mismatch");
    assert_eq!(
        payload.extended_at_ledger, current_ledger,
        "extended_at_ledger field mismatch"
    );
    assert_eq!(
        payload.new_expiry_ledger, expected_new_expiry,
        "new_expiry_ledger field mismatch"
    );
}
