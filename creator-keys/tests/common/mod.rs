use crate::events;
use soroban_sdk::{
    testutils::Events,
    Address, Env, IntoVal, Symbol,
};

pub struct EventsFixture<'a> {
    env: &'a Env,
}

impl<'a> EventsFixture<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }

    pub fn assert_registration_event(
        &self,
        creator: &Address,
        expected_handle: &soroban_sdk::String,
        expected_supply: u32,
        expected_holder_count: u32,
    ) {
        let last_event = self.get_last_event_by_name(events::REGISTER_EVENT_NAME);
        let (_, topics, data) = last_event;

        let event_creator: Address = topics.get(1).unwrap().into_val(self.env);
        assert_eq!(event_creator, *creator);

        let payload: events::CreatorRegisteredEvent = data.into_val(self.env);
        assert_eq!(payload.creator, *creator);
        assert_eq!(payload.handle, *expected_handle);
        assert_eq!(payload.supply, expected_supply);
        assert_eq!(payload.holder_count, expected_holder_count);
    }

    pub fn assert_buy_event(
        &self,
        creator: &Address,
        buyer: &Address,
        expected_supply: u32,
        expected_payment: i128,
    ) {
        let last_event = self.get_last_event_by_name(events::BUY_EVENT_NAME);
        let (_, topics, data) = last_event;

        let event_creator: Address = topics.get(1).unwrap().into_val(self.env);
        assert_eq!(event_creator, *creator);

        let event_buyer: Address = topics.get(2).unwrap().into_val(self.env);
        assert_eq!(event_buyer, *buyer);

        let (supply, payment): (u32, i128) = data.into_val(self.env);
        assert_eq!(supply, expected_supply);
        assert_eq!(payment, expected_payment);
    }

    pub fn assert_sell_event(&self, creator: &Address, seller: &Address, expected_supply: u32) {
        let last_event = self.get_last_event_by_name(events::SELL_EVENT_NAME);
        let (_, topics, data) = last_event;

        let event_creator: Address = topics.get(1).unwrap().into_val(self.env);
        assert_eq!(event_creator, *creator);

        let event_seller: Address = topics.get(2).unwrap().into_val(self.env);
        assert_eq!(event_seller, *seller);

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
