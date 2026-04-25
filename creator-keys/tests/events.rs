//! Tests for registration, buy, and sell event payloads using reusable fixtures.

use creator_keys::{events, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol,
};

/// Reusable fixture for contract event assertions.
///
/// This helper encapsulates the boilerplate of fetching, filtering, and
/// deserializing contract events for consistent testing across different actions.
pub struct EventsFixture<'a> {
    env: &'a Env,
}

impl<'a> EventsFixture<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }

    /// Asserts that a creator registration event was emitted with the expected payload.
    pub fn assert_registration_event(
        &self,
        creator: &Address,
        expected_handle: &String,
        expected_supply: u32,
        expected_holder_count: u32,
    ) {
        let last_event = self.get_last_event_by_name(events::REGISTER_EVENT_NAME);
        let (_, topics, data) = last_event;

        // Topic 1: Creator address
        let event_creator: Address = topics.get(1).unwrap().into_val(self.env);
        assert_eq!(event_creator, *creator);

        // Data: CreatorRegisteredEvent struct
        let payload: events::CreatorRegisteredEvent = data.into_val(self.env);
        assert_eq!(payload.creator, *creator);
        assert_eq!(payload.handle, *expected_handle);
        assert_eq!(payload.supply, expected_supply);
        assert_eq!(payload.holder_count, expected_holder_count);
    }

    /// Asserts that a buy event was emitted with the expected topics and data.
    pub fn assert_buy_event(
        &self,
        creator: &Address,
        buyer: &Address,
        expected_supply: u32,
        expected_payment: i128,
    ) {
        let last_event = self.get_last_event_by_name(events::BUY_EVENT_NAME);
        let (_, topics, data) = last_event;

        // Topic 1: Creator
        let event_creator: Address = topics.get(1).unwrap().into_val(self.env);
        assert_eq!(event_creator, *creator);

        // Topic 2: Buyer
        let event_buyer: Address = topics.get(2).unwrap().into_val(self.env);
        assert_eq!(event_buyer, *buyer);

        // Data: (supply, payment)
        let (supply, payment): (u32, i128) = data.into_val(self.env);
        assert_eq!(supply, expected_supply);
        assert_eq!(payment, expected_payment);
    }

    /// Asserts that a sell event was emitted with the expected topics and data.
    pub fn assert_sell_event(&self, creator: &Address, seller: &Address, expected_supply: u32) {
        let last_event = self.get_last_event_by_name(events::SELL_EVENT_NAME);
        let (_, topics, data) = last_event;

        // Topic 1: Creator
        let event_creator: Address = topics.get(1).unwrap().into_val(self.env);
        assert_eq!(event_creator, *creator);

        // Topic 2: Seller
        let event_seller: Address = topics.get(2).unwrap().into_val(self.env);
        assert_eq!(event_seller, *seller);

        // Data: (supply,)
        let (supply,): (u32,) = data.into_val(self.env);
        assert_eq!(supply, expected_supply);
    }

    fn get_last_event_by_name(
        &self,
        expected_name: Symbol,
    ) -> (
        soroban_sdk::Address,
        soroban_sdk::Vec<soroban_sdk::Val>,
        soroban_sdk::Val,
    ) {
        let events = self.env.events().all();
        let filtered: Vec<_> = events
            .iter()
            .filter(|(_, topics, _)| {
                if let Some(topic_val) = topics.get(0) {
                    let name: Symbol = topic_val.into_val(self.env);
                    name == expected_name
                } else {
                    false
                }
            })
            .collect();

        assert!(
            !filtered.is_empty(),
            "No event with name {:?} found",
            expected_name
        );
        filtered.last().unwrap().clone()
    }
}

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

    let topic: soroban_sdk::Symbol = topics
        .get(events::TOPIC_EVENT_NAME_INDEX)
        .unwrap()
        .into_val(&env);
    assert_eq!(topic, events::REGISTER_EVENT_NAME);

    let event_creator: Address = topics
        .get(events::TOPIC_CREATOR_INDEX)
        .unwrap()
        .into_val(&env);
    assert_eq!(event_creator, creator);
}

#[test]
fn test_register_creator_emits_correct_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let fixture = EventsFixture::new(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle);

    fixture.assert_registration_event(&creator, &handle, 0, 0);
    let events = env.events().all();
    let last = events.last().unwrap();
    let payload: events::CreatorRegisteredEvent = last.2.into_val(&env);

    assert_eq!(payload.creator, creator);
    assert_eq!(payload.handle, handle);
    assert_eq!(payload.supply, 0);
    assert_eq!(payload.holder_count, 0);
}

#[test]
fn test_register_creator_event_payload_field_order_is_documented() {
    assert_eq!(
        events::REGISTER_EVENT_DATA_FIELDS,
        ["creator", "handle", "supply", "holder_count"]
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
    client.register_creator(&creator, &String::from_str(&env, "bob"));
    let after = env.events().all().len();

    assert_eq!(after - before, 1, "register should emit exactly one event");
}

#[test]
fn test_buy_key_emits_correct_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let fixture = EventsFixture::new(&env);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"));
    client.buy_key(&creator, &buyer, &100_i128);

    fixture.assert_buy_event(&creator, &buyer, 1, 100);
}

#[test]
fn test_sell_key_emits_correct_event() {
    let events = env.events().all();
    let last = events.last().unwrap();
    let (_, topics, _) = last;

    // Topics: (events::BUY_EVENT_NAME, creator, buyer)
    let event_sym: soroban_sdk::Symbol = topics
        .get(events::TOPIC_EVENT_NAME_INDEX)
        .unwrap()
        .into_val(&env);
    assert_eq!(event_sym, events::BUY_EVENT_NAME);

    let event_creator: Address = topics
        .get(events::TOPIC_CREATOR_INDEX)
        .unwrap()
        .into_val(&env);
    assert_eq!(event_creator, creator);

    let event_buyer: Address = topics
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
fn test_buy_key_event_payload_field_order_is_documented() {
    assert_eq!(events::BUY_EVENT_DATA_FIELDS, ["supply", "payment"]);
}

#[test]
fn test_buy_key_event_present_after_purchase() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let fixture = EventsFixture::new(&env);

    let creator = Address::generate(&env);
    let holder = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"));
    client.buy_key(&creator, &holder, &100_i128);
    client.sell_key(&creator, &holder);

    fixture.assert_sell_event(&creator, &holder, 0);
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
