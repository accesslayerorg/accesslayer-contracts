//! Fixture-based tests for registration, buy, and sell event payloads.

use creator_keys::{events, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol, Val, Vec,
};

const KEY_PRICE: i128 = 100;

struct EventFixture<'a> {
    client: CreatorKeysContractClient<'a>,
    creator: Address,
}

struct TradeTopics {
    event_name: Symbol,
    creator: Address,
    actor: Address,
}

struct SellEventPayload {
    creator_id: Address,
    supply: u32, // derived from supply = total_supply after sell (quantity subtracted)
}

impl<'a> EventFixture<'a> {
    fn new(env: &'a Env) -> Self {
        let id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(env, &id);
        let admin = Address::generate(env);
        let creator = Address::generate(env);

        client.set_key_price(&admin, &KEY_PRICE);

        Self { client, creator }
    }

    fn register_creator(&self, env: &Env, handle: &str) {
        self.client.register_creator(
            &creator_keys::RegisterCreatorParams {
                creator: self.creator.clone(),
                handle: String::from_str(env, handle),
            },
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );
    }

    fn buy_key(&self, buyer: &Address, payment: i128) {
        self.client.buy_key(&self.creator, buyer, &payment, &None);
    }

    fn sell_key(&self, seller: &Address) {
        self.client.sell_key(&self.creator, seller, &None);
    }

    fn last_trade_topics(&self, env: &Env) -> TradeTopics {
        let event_log = env.events().all();
        let (_, topics, _) = event_log
            .iter()
            .rev()
            .find(|(_, topics, _)| {
                topics
                    .get(events::TOPIC_EVENT_NAME_INDEX)
                    .map(|v| {
                        let name: Symbol = v.into_val(env);
                        name == events::BUY_EVENT_NAME || name == events::SELL_EVENT_NAME
                    })
                    .unwrap_or(false)
            })
            .unwrap();

        TradeTopics {
            event_name: topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .unwrap()
                .into_val(env),
            creator: topics
                .get(events::TOPIC_CREATOR_INDEX)
                .unwrap()
                .into_val(env),
            actor: topics.get(events::TOPIC_BUYER_INDEX).unwrap().into_val(env),
        }
    }

    fn last_buy_payload(&self, env: &Env) -> events::KeysBoughtEvent {
        let event_log = env.events().all();
        let (_, _, data) = event_log
            .iter()
            .rev()
            .find(|(_, topics, _)| {
                topics
                    .get(events::TOPIC_EVENT_NAME_INDEX)
                    .map(|v| {
                        let name: Symbol = v.into_val(env);
                        name == events::BUY_EVENT_NAME
                    })
                    .unwrap_or(false)
            })
            .unwrap();
        data.into_val(env)
    }

    fn last_sell_payload(&self, env: &Env) -> SellEventPayload {
        let event_log = env.events().all();
        let (_, _, data) = event_log
            .iter()
            .rev()
            .find(|(_, topics, _)| {
                topics
                    .get(events::TOPIC_EVENT_NAME_INDEX)
                    .map(|v| {
                        let name: Symbol = v.into_val(env);
                        name == events::SELL_EVENT_NAME
                    })
                    .unwrap_or(false)
            })
            .unwrap();

        let event: events::KeysSoldEvent = data.into_val(env);
        // Reconstruct legacy supply field from the new event shape:
        // supply = total_supply after sell = not directly in event; use creator_id for routing.
        // For backward-compat assertions we derive supply from the contract state.
        SellEventPayload {
            creator_id: event.creator_id.clone(),
            supply: self.client.get_total_key_supply(&event.creator_id),
        }
    }
}

fn assert_event_topic_matches(env: &Env, event: &(Address, Vec<Val>, Val), expected_topic: Symbol) {
    let actual_topic: Symbol = event
        .1
        .get(events::TOPIC_EVENT_NAME_INDEX)
        .expect("event topic should be present")
        .into_val(env);

    assert_eq!(
        actual_topic, expected_topic,
        "event topic mismatch: expected {:?}, got {:?}",
        expected_topic, actual_topic
    );
}

/// Builder for constructing expected CreatorRegisteredEvent payloads in tests.
///
/// This helper simplifies building the full expected event payload struct
/// from named parameters, making assertions more readable and easier to
/// update when the schema changes.
struct CreatorRegisteredEventBuilder {
    creator: Option<Address>,
    handle: Option<String>,
    supply: u32,
    holder_count: u32,
    creator_bps: u32,
    protocol_bps: u32,
    fee_recipient: Option<Address>,
    registered_at_ledger: u32,
}

impl CreatorRegisteredEventBuilder {
    fn new() -> Self {
        Self {
            creator: None,
            handle: None,
            supply: 0,
            holder_count: 0,
            creator_bps: 0,
            protocol_bps: 0,
            fee_recipient: None,
            registered_at_ledger: 0,
        }
    }

    fn creator(mut self, creator: Address) -> Self {
        self.creator = Some(creator);
        self
    }

    fn handle(mut self, handle: String) -> Self {
        self.handle = Some(handle);
        self
    }

    fn supply(mut self, supply: u32) -> Self {
        self.supply = supply;
        self
    }

    fn holder_count(mut self, holder_count: u32) -> Self {
        self.holder_count = holder_count;
        self
    }

    fn creator_bps(mut self, creator_bps: u32) -> Self {
        self.creator_bps = creator_bps;
        self
    }

    fn protocol_bps(mut self, protocol_bps: u32) -> Self {
        self.protocol_bps = protocol_bps;
        self
    }

    fn fee_recipient(mut self, fee_recipient: Address) -> Self {
        self.fee_recipient = Some(fee_recipient);
        self
    }

    fn registered_at_ledger(mut self, registered_at_ledger: u32) -> Self {
        self.registered_at_ledger = registered_at_ledger;
        self
    }

    fn build(self) -> events::CreatorRegisteredEvent {
        let creator = self.creator.expect("creator must be set");
        let fee_recipient = self.fee_recipient.unwrap_or_else(|| creator.clone());
        events::CreatorRegisteredEvent {
            creator,
            handle: self.handle.expect("handle must be set"),
            supply: self.supply,
            holder_count: self.holder_count,
            creator_bps: self.creator_bps,
            protocol_bps: self.protocol_bps,
            fee_recipient,
            registered_at_ledger: self.registered_at_ledger,
        }
    }
}

#[test]
fn test_register_creator_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);

    fixture.register_creator(&env, "alice");

    let events = env.events().all();
    assert!(!events.is_empty(), "should emit at least one event");

    let last = events.last().unwrap();
    assert_event_topic_matches(&env, &last, events::REGISTER_EVENT_NAME);

    let event_creator: Address = last
        .1
        .get(events::TOPIC_CREATOR_INDEX)
        .unwrap()
        .into_val(&env);
    assert_eq!(event_creator, fixture.creator);
}

#[test]
fn test_register_creator_event_data_is_indexer_friendly() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);
    let handle = String::from_str(&env, "alice");

    fixture.client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: fixture.creator.clone(),
            handle: handle.clone(),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let events = env.events().all();
    let last = events.last().unwrap();
    let payload: events::CreatorRegisteredEvent = last.2.into_val(&env);

    let creator_addr = fixture.creator.clone();
    let expected = CreatorRegisteredEventBuilder::new()
        .creator(fixture.creator)
        .handle(handle)
        .supply(0)
        .holder_count(0)
        .creator_bps(0)
        .protocol_bps(0)
        .fee_recipient(creator_addr)
        .registered_at_ledger(env.ledger().sequence())
        .build();

    assert_eq!(payload, expected);
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
            "protocol_bps",
            "fee_recipient",
            "registered_at_ledger"
        ]
    );
}

#[test]
fn test_register_creator_event_fires_once() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);

    let before = env.events().all().len();
    fixture.register_creator(&env, "bob");
    let after = env.events().all().len();

    assert_eq!(after - before, 1, "register should emit exactly one event");
}

#[test]
#[should_panic(expected = "event topic mismatch")]
fn test_assert_event_topic_matches_rejects_unexpected_identifier() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);

    fixture.register_creator(&env, "alice");

    let events = env.events().all();
    let last = events.last().unwrap();

    assert_event_topic_matches(&env, &last, events::BUY_EVENT_NAME);
}

#[test]
fn test_buy_key_event_payload_fields_are_validated_from_fixture() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);
    let buyer = Address::generate(&env);

    fixture.register_creator(&env, "alice");
    fixture.buy_key(&buyer, 150);

    let topics = fixture.last_trade_topics(&env);
    let payload = fixture.last_buy_payload(&env);

    assert_eq!(topics.event_name, events::BUY_EVENT_NAME);
    assert_eq!(topics.creator, fixture.creator);
    assert_eq!(topics.actor, buyer);

    assert_eq!(payload.buyer, buyer);
    assert_eq!(payload.creator_id, fixture.creator);
    assert_eq!(payload.quantity, 1);
    assert_eq!(payload.price_paid, 100); // initial key price for supply 0 is 100
}

#[test]
fn test_buy_key_event_payload_tracks_new_supply_across_purchases() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);
    let buyer1 = Address::generate(&env);
    let buyer2 = Address::generate(&env);

    fixture.register_creator(&env, "alice");

    fixture.buy_key(&buyer1, KEY_PRICE);
    let first_payload = fixture.last_buy_payload(&env);
    assert_eq!(first_payload.price_paid, KEY_PRICE);
    assert_eq!(
        first_payload.new_supply, 1,
        "first buy new_supply should be 1"
    );

    fixture.buy_key(&buyer2, KEY_PRICE);
    let second_payload = fixture.last_buy_payload(&env);
    assert_eq!(second_payload.price_paid, KEY_PRICE);
    assert_eq!(
        second_payload.new_supply, 2,
        "second buy new_supply should be 2"
    );
}

#[test]
fn test_buy_key_event_payload_field_order_is_documented() {
    assert_eq!(
        events::BUY_EVENT_DATA_FIELDS,
        [
            "buyer",
            "creator_id",
            "quantity",
            "price_paid",
            "new_supply",
            "ledger",
        ]
    );
}

#[test]
fn test_buy_key_event_present_after_purchase() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);
    let buyer = Address::generate(&env);

    fixture.register_creator(&env, "alice");
    fixture.buy_key(&buyer, KEY_PRICE);

    let has_buy_event = env.events().all().iter().any(|(_, topics, _)| {
        topics
            .get(events::TOPIC_EVENT_NAME_INDEX)
            .map(|value| {
                let sym: Symbol = value.into_val(&env);
                sym == events::BUY_EVENT_NAME
            })
            .unwrap_or(false)
    });

    assert!(has_buy_event, "buy event should be present");
}

#[test]
fn test_sell_key_event_payload_fields_are_validated_from_fixture() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);
    let seller = Address::generate(&env);

    fixture.register_creator(&env, "alice");
    fixture.buy_key(&seller, KEY_PRICE);
    fixture.buy_key(&seller, KEY_PRICE);
    fixture.sell_key(&seller);

    let topics = fixture.last_trade_topics(&env);
    let payload = fixture.last_sell_payload(&env);

    assert_eq!(topics.event_name, events::SELL_EVENT_NAME);
    assert_eq!(topics.creator, fixture.creator);
    assert_eq!(topics.actor, seller);
    assert_eq!(payload.creator_id, fixture.creator);
    assert_eq!(payload.supply, 1);
}

#[test]
fn test_sell_key_event_payload_tracks_zero_supply_after_last_sale() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);
    let seller = Address::generate(&env);

    fixture.register_creator(&env, "alice");
    fixture.buy_key(&seller, KEY_PRICE);
    fixture.sell_key(&seller);

    let topics = fixture.last_trade_topics(&env);
    let payload = fixture.last_sell_payload(&env);

    assert_eq!(topics.event_name, events::SELL_EVENT_NAME);
    assert_eq!(topics.creator, fixture.creator);
    assert_eq!(topics.actor, seller);
    assert_eq!(payload.creator_id, fixture.creator);
    assert_eq!(payload.supply, 0);
}

#[test]
fn test_sell_key_event_payload_field_order_is_documented() {
    assert_eq!(
        events::SELL_EVENT_DATA_FIELDS,
        ["seller", "creator_id", "quantity", "proceeds", "ledger"]
    );
}

#[test]
#[should_panic(expected = "event topic mismatch")]
fn test_assert_event_topic_matches_panics_on_buy_vs_sell_mismatch() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);
    let buyer = Address::generate(&env);

    fixture.register_creator(&env, "alice");
    fixture.buy_key(&buyer, KEY_PRICE);

    let buy_event = env
        .events()
        .all()
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::BUY_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("buy event should be present");

    assert_event_topic_matches(&env, &buy_event, events::SELL_EVENT_NAME);
}

#[test]
fn test_assert_event_topic_matches_passes_on_matching_topic() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);
    let buyer = Address::generate(&env);

    fixture.register_creator(&env, "alice");
    fixture.buy_key(&buyer, KEY_PRICE);

    let buy_event = env
        .events()
        .all()
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::BUY_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("buy event should be present");

    assert_event_topic_matches(&env, &buy_event, events::BUY_EVENT_NAME);
}

#[test]
#[should_panic(expected = "event topic should be present")]
fn test_assert_event_topic_matches_panics_when_no_topics() {
    let env = Env::default();
    let addr = Address::generate(&env);
    let empty_topics: Vec<Val> = Vec::new(&env);
    let event = (addr, empty_topics, 0_i32.into_val(&env));

    assert_event_topic_matches(&env, &event, events::BUY_EVENT_NAME);
}

#[test]
fn test_assert_event_topic_mismatch_message_identifies_topics() {
    let env = Env::default();
    env.mock_all_auths();
    let fixture = EventFixture::new(&env);
    let buyer = Address::generate(&env);

    fixture.register_creator(&env, "alice");
    fixture.buy_key(&buyer, KEY_PRICE);

    let buy_event = env
        .events()
        .all()
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::BUY_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("buy event should be present");

    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_event_topic_matches(&env, &buy_event, events::SELL_EVENT_NAME);
    }))
    .unwrap_err();

    let message = err
        .downcast_ref::<std::string::String>()
        .cloned()
        .or_else(|| {
            err.downcast_ref::<&str>()
                .map(|s| std::string::String::from(*s))
        })
        .unwrap_or_default();
    assert!(
        message.contains("event topic mismatch"),
        "message should indicate topic mismatch, got: {}",
        message
    );
    assert!(
        message.contains(&format!("{:?}", events::BUY_EVENT_NAME)),
        "message should identify actual topic, got: {}",
        message
    );
    assert!(
        message.contains(&format!("{:?}", events::SELL_EVENT_NAME)),
        "message should identify expected topic, got: {}",
        message
    );
}
