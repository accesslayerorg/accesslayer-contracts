//! Integration tests for automatic TTL extension during buy transactions.
//!
//! Confirms that when a creator's storage TTL is near expiry, the next buy
//! extends it and emits a [`events::TTL_EXTENDED_EVENT_NAME`] event.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_key_price_for_tests};
use creator_keys::constants::storage;
use creator_keys::events::{self, ttl_extended_topics};
use creator_keys::CREATOR_TTL_LEDGERS;
use soroban_sdk::testutils::storage::Persistent;
use soroban_sdk::testutils::Ledger;
use soroban_sdk::{testutils::Address as _, testutils::Events, Address, IntoVal};

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

/// Advance the ledger far enough that the creator key's remaining TTL drops
/// below the extension threshold, then buy and confirm TTL was extended and
/// the extension event was emitted.
#[test]
fn test_buy_extends_creator_ttl() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator) = setup(&env);
    let holder = Address::generate(&env);

    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    let mut ledger = env.ledger().get();
    ledger.sequence_number += ttl_before.saturating_sub(1).max(1);
    env.ledger().set(ledger);

    let ttl_before_advance = creator_ttl_remaining(&env, &contract_id, &creator);

    let result = client.try_buy_key(&creator, &holder, &KEY_PRICE, &None);
    assert_eq!(result, Ok(Ok(1)), "buy should succeed");

    let events = env.events().all();
    let found = events
        .iter()
        .rev()
        .any(|(_, topics, _)| topics == ttl_extended_topics(&creator).into_val(&env));
    assert!(found, "TTL extension event should be emitted after buy");

    let ttl_after = creator_ttl_remaining(&env, &contract_id, &creator);

    assert!(
        ttl_after > ttl_before_advance,
        "TTL should increase after buy: before_advance={ttl_before_advance} after={ttl_after}"
    );
}

/// Confirm the TTL extension event has the expected topics and payload shape.
#[test]
fn test_ttl_extension_event_topics_and_payload() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator) = setup(&env);
    let holder = Address::generate(&env);

    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    let mut ledger = env.ledger().get();
    ledger.sequence_number += ttl_before.saturating_sub(1).max(1);
    env.ledger().set(ledger);

    let ledger_before_buy = env.ledger().sequence();

    let result = client.try_buy_key(&creator, &holder, &KEY_PRICE, &None);
    assert_eq!(result, Ok(Ok(1)), "buy should succeed");

    let events = env.events().all();
    let (topics, data) = events
        .iter()
        .rev()
        .find_map(|(_, topics, data)| {
            if topics == ttl_extended_topics(&creator).into_val(&env) {
                Some((topics, data))
            } else {
                None
            }
        })
        .expect("TTL extension event not found after buy");

    assert_eq!(topics.len(), 2, "topics tuple should have 2 elements");
    let topic0: soroban_sdk::Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(topic0, events::TTL_EXTENDED_EVENT_NAME);

    let topic1: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic1, creator);

    let extend_to: u32 = data.into_val(&env);
    let expected_extend_to = ledger_before_buy + CREATOR_TTL_LEDGERS;
    assert_eq!(extend_to, expected_extend_to);
}

/// Second buy on the same ledger does NOT further extend TTL
/// (confirmed by TTL remaining unchanged).
#[test]
fn test_ttl_not_extended_when_already_high() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator) = setup(&env);
    let holder = Address::generate(&env);

    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    let mut ledger = env.ledger().get();
    ledger.sequence_number += ttl_before.saturating_sub(1).max(1);
    env.ledger().set(ledger);

    let result = client.try_buy_key(&creator, &holder, &KEY_PRICE, &None);
    assert_eq!(result, Ok(Ok(1)), "first buy should succeed");

    let ttl_after_first = creator_ttl_remaining(&env, &contract_id, &creator);

    let holder2 = Address::generate(&env);
    let result2 = client.try_buy_key(&creator, &holder2, &KEY_PRICE, &None);
    assert_eq!(result2, Ok(Ok(2)), "second buy should succeed");

    let ttl_after_second = creator_ttl_remaining(&env, &contract_id, &creator);

    assert!(
        ttl_after_first > 0,
        "TTL should be positive after first buy"
    );
    assert_eq!(
        ttl_after_first, ttl_after_second,
        "TTL should remain unchanged after second buy on same ledger"
    );
}
