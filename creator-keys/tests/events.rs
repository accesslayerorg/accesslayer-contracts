//! Tests for registration and buy event payloads (#22).

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String,
};

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &id);
    let admin = Address::generate(env);
    client.set_key_price(&admin, &100_i128);
    (client, admin)
}

// ── Registration event tests ────────────────────────────────────────────

#[test]
fn test_register_creator_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle);

    let events = env.events().all();
    assert!(!events.is_empty(), "should emit at least one event");

    let last = events.last().unwrap();
    let (_, topics, _data) = last;

    // First topic should be the "register" symbol
    let topic: soroban_sdk::Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(topic, symbol_short!("register"));
}

#[test]
fn test_register_creator_event_fires_once() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);

    // Count events before and after
    let before = env.events().all().len();
    client.register_creator(&creator, &String::from_str(&env, "bob"));
    let after = env.events().all().len();

    assert_eq!(after - before, 1, "register should emit exactly one event");
}

// ── Buy event tests ─────────────────────────────────────────────────────

#[test]
fn test_buy_key_emits_event_with_correct_topics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"));
    client.buy_key(&creator, &buyer, &100_i128);

    let events = env.events().all();
    let last = events.last().unwrap();
    let (_, topics, _) = last;

    // Topics: (symbol_short!("buy"), creator, buyer)
    let event_sym: soroban_sdk::Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(event_sym, symbol_short!("buy"));

    let event_creator: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(event_creator, creator);

    let event_buyer: Address = topics.get(2).unwrap().into_val(&env);
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

    client.register_creator(&creator, &String::from_str(&env, "alice"));

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
fn test_buy_key_event_present_after_purchase() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"));
    client.buy_key(&creator, &buyer, &100_i128);

    // Verify the buy event is present in the event log
    let events = env.events().all();
    let has_buy_event = events.iter().any(|(_, topics, _)| {
        if let Some(v) = topics.get(0) {
            let sym: soroban_sdk::Symbol = v.into_val(&env);
            sym == symbol_short!("buy")
        } else {
            false
        }
    });
    assert!(has_buy_event, "buy event should be present");
}
