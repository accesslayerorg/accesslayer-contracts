//! Integration test: no TTL extension event when creator TTL is healthy.
//!
//! The `extend_creator_ttl` helper should be a no-op when the creator's
//! storage has a healthy remaining TTL (at or above `CREATOR_TTL_LEDGERS`).
//! This test registers a creator, buys a key immediately (while the TTL is
//! still at its maximum), and verifies:
//!
//! 1. No [`events::TTL_EXTENDED_EVENT_NAME`] event is emitted.
//! 2. A [`events::BUY_EVENT_NAME`] event IS emitted (the buy itself succeeded).
//! 3. The creator's storage TTL is unchanged after the buy.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_key_price_for_tests};
use creator_keys::constants::storage;
use creator_keys::events::{self, ttl_extended_topics};
use soroban_sdk::testutils::storage::Persistent;
use soroban_sdk::{testutils::Address as _, testutils::Events, Address, IntoVal, Symbol};

const KEY_PRICE: i128 = 100;

/// Read remaining TTL (ledgers until expiry) for a creator's profile key.
fn creator_ttl_remaining(env: &soroban_sdk::Env, contract_id: &Address, creator: &Address) -> u32 {
    let key = storage::creator(creator);
    env.as_contract(contract_id, || env.storage().persistent().get_ttl(&key))
}

fn setup(
    env: &soroban_sdk::Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    soroban_sdk::Address,
    soroban_sdk::Address,
) {
    let (client, contract_id) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    (client, contract_id, creator)
}

/// Buy a key while TTL is still at its maximum (just after registration)
/// and confirm NO TTL extension event is emitted — the extension is a no-op
/// when the TTL is healthy.
#[test]
fn test_no_ttl_extension_event_when_ttl_healthy() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator) = setup(&env);
    let holder = Address::generate(&env);

    // Capture remaining TTL before the buy — it should be at or above
    // CREATOR_TTL_LEDGERS since registration just happened.
    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);
    assert!(
        ttl_before >= creator_keys::CREATOR_TTL_LEDGERS,
        "TTL should be at its maximum right after registration: {ttl_before}"
    );

    // Execute a buy — this triggers extend_creator_ttl internally.
    let result = client.try_buy_key(&creator, &holder, &KEY_PRICE, &None);
    assert_eq!(result, Ok(Ok(1)), "buy should succeed");

    // Extract all events from the transaction result.
    let events = env.events().all();

    // 1. Assert NO TTL extension event is present.
    let ttl_ext_event_found = events.iter().rev().any(|(_, topics, _)| {
        topics == ttl_extended_topics(&creator).into_val(&env)
    });
    assert!(
        !ttl_ext_event_found,
        "TTL extension event should NOT be emitted when TTL is healthy"
    );

    // 2. Assert the buy event IS present (transaction succeeded).
    let buy_event_found = events.iter().rev().any(|(_, topics, _)| {
        topics
            .get(events::TOPIC_EVENT_NAME_INDEX)
            .map(|v| {
                let name: Symbol = v.into_val(&env);
                name == events::BUY_EVENT_NAME
            })
            .unwrap_or(false)
    });
    assert!(buy_event_found, "buy event should be present");

    // 3. Confirm the creator's storage TTL is unchanged.
    let ttl_after = creator_ttl_remaining(&env, &contract_id, &creator);
    assert_eq!(
        ttl_before, ttl_after,
        "TTL should remain unchanged after buy when it was already healthy: before={ttl_before} after={ttl_after}"
    );
}
