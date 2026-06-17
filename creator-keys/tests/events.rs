//! Tests for registration and buy event payloads.

use creator_keys::{events, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol, Val, Vec,
};

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &id);
    let admin = Address::generate(env);
    client.set_key_price(&admin, &100_i128);
    (client, admin)
}

fn assert_event_topic_matches(env: &Env, event: &(Address, Vec<Val>, Val), expected_topic: Symbol) {
    let actual_topic: Symbol = event
        .1
        .get(events::TOPIC_EVENT_NAME_INDEX)
        .expect("event topic should be present")
        .into_val(env);

    assert_eq!(
        actual_topic, expected_topic,
        "event topic should match expected contract identifier"
    );
}

// ── Registration event tests ────────────────────────────────────────────

#[test]
fn test_register_creator_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle, &None);

    let events = env.events().all();
    assert!(!events.is_empty(), "should emit at least one event");

    let last = events.last().unwrap();
    assert_event_topic_matches(&env, &last, events::REGISTER_EVENT_NAME);

    let event_creator: Address = last
        .1
        .get(events::TOPIC_CREATOR_INDEX)
        .unwrap()
        .into_val(&env);
    assert_eq!(event_creator, creator);
}

#[test]
fn test_register_creator_event_data_is_indexer_friendly() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle, &None);

    let events = env.events().all();
    let last = events.last().unwrap();
    let payload: events::CreatorRegisteredEvent = last.2.into_val(&env);

    assert_eq!(payload.creator, creator);
    assert_eq!(payload.handle, handle);
    assert_eq!(payload.supply, 0);
    assert_eq!(payload.holder_count, 0);
    assert_eq!(payload.creator_bps, 0); // Default when not set in setup
    assert_eq!(payload.protocol_bps, 0);
}

#[test]
fn test_register_creator_event_payload_field_order_is_documented() {
    assert_eq!(
        events::REGISTER_EVENT_DATA_FIELDS,
        [
            "creator",
            "handle",
            "supply",
            "holder_count",
            "creator_bps",
            "protocol_bps"
        ]
    );
}

#[test]
fn test_register_creator_event_fires_once() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);

    // Count events before and after
    let before = env.events().all().len();
    client.register_creator(&creator, &String::from_str(&env, "bob"), &None);
    let after = env.events().all().len();

    assert_eq!(after - before, 1, "register should emit exactly one event");
}

#[test]
#[should_panic(expected = "event topic should match expected contract identifier")]
fn test_assert_event_topic_matches_rejects_unexpected_identifier() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    client.register_creator(&creator, &String::from_str(&env, "alice"), &None);

    let events = env.events().all();
    let last = events.last().unwrap();

    assert_event_topic_matches(&env, &last, events::BUY_EVENT_NAME);
}

// ── Buy event tests ─────────────────────────────────────────────────────

#[test]
fn test_buy_key_emits_event_with_correct_topics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None);
    client.buy_key(&creator, &buyer, &100_i128);

    let events = env.events().all();
    let last = events.last().unwrap();
    assert_event_topic_matches(&env, &last, events::BUY_EVENT_NAME);

    let event_creator: Address = last
        .1
        .get(events::TOPIC_CREATOR_INDEX)
        .unwrap()
        .into_val(&env);
    assert_eq!(event_creator, creator);

    let event_buyer: Address = last
        .1
        .get(events::TOPIC_BUYER_INDEX)
        .unwrap()
        .into_val(&env);
    assert_eq!(event_buyer, buyer);
}

#[test]
fn test_buy_key_event_data_is_new_supply() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let buyer1 = Address::generate(&env);
    let buyer2 = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None);

    // First buy → supply = 1, payment = 100
    client.buy_key(&creator, &buyer1, &100_i128);
    let events = env.events().all();
    let (_, _, data) = events.last().unwrap();
    let (supply, payment): (u32, i128) = data.into_val(&env);
    assert_eq!(supply, 1);
    assert_eq!(payment, 100);

    // Second buy → supply = 2, payment = 100
    client.buy_key(&creator, &buyer2, &100_i128);
    let events = env.events().all();
    let (_, _, data) = events.last().unwrap();
    let (supply, payment): (u32, i128) = data.into_val(&env);
    assert_eq!(supply, 2);
    assert_eq!(payment, 100);
}

#[test]
fn test_buy_key_event_payload_field_order_is_documented() {
    assert_eq!(events::BUY_EVENT_DATA_FIELDS, ["supply", "payment"]);
}

#[test]
fn test_buy_key_event_present_after_purchase() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None);
    client.buy_key(&creator, &buyer, &100_i128);

    // Verify the buy event is present in the event log
    let events = env.events().all();
    let has_buy_event = events.iter().any(|(_, topics, _)| {
        if let Some(v) = topics.get(events::TOPIC_EVENT_NAME_INDEX) {
            let sym: soroban_sdk::Symbol = v.into_val(&env);
            sym == events::BUY_EVENT_NAME
        } else {
            false
        }
    });
    assert!(has_buy_event, "buy event should be present");
}
