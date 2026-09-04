//! Integration test verifying buy and sell event topics are distinct
//! from each other and from all other contract event topics.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_key_price_for_tests};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 100;

/// All event name constants defined in the contract, excluding buy and sell.
const ALL_OTHER_EVENT_NAMES: &[Symbol] = &[
    events::PAUSE_EVENT_NAME,
    events::UNPAUSE_EVENT_NAME,
    events::REGISTER_EVENT_NAME,
    events::TRANSFER_EVENT_NAME,
    events::BUYBACK_EVENT_NAME,
    events::REFERRAL_FEE_EARNED_EVENT_NAME,
    events::POLL_CREATED_EVENT_NAME,
    events::POLL_VOTE_EVENT_NAME,
    events::DIVIDEND_DISTRIBUTED_EVENT_NAME,
    events::DIVIDEND_CLAIMED_EVENT_NAME,
    events::ALLOCATION_LOCKED_EVENT_NAME,
    events::ALLOCATION_CLAIMED_EVENT_NAME,
    events::PROTOCOL_FEE_RECIPIENT_UPDATED_EVENT_NAME,
    events::CREATOR_FEE_RECIPIENT_UPDATED_EVENT_NAME,
    events::CO_CREATOR_FEE_EARNED_EVENT_NAME,
    events::FEE_CONFIG_UPDATED_EVENT_NAME,
    events::KEYS_TRANSFERRED_EVENT_NAME,
    events::KEYS_AIRDROPPED_EVENT_NAME,
    events::TREASURY_WITHDRAWAL_EVENT_NAME,
    events::TTL_EXTENDED_EVENT_NAME,
];

fn extract_first_event_name(env: &Env, event_name_filter: Symbol) -> Symbol {
    let event_log = env.events().all();
    let (_, topics, _) = event_log
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(env);
                    name == event_name_filter
                })
                .unwrap_or(false)
        })
        .expect("event should be present in event log");
    topics
        .get(events::TOPIC_EVENT_NAME_INDEX)
        .unwrap()
        .into_val(env)
}

#[test]
fn test_buy_and_sell_event_topics_are_distinct() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");
    let user = Address::generate(&env);

    client.buy_key(&creator, &user, &KEY_PRICE, &None);
    let buy_event_name = extract_first_event_name(&env, events::BUY_EVENT_NAME);

    env.ledger().with_mut(|l| l.sequence_number += 1);
    client.sell_key(&creator, &user, &None);
    let sell_event_name = extract_first_event_name(&env, events::SELL_EVENT_NAME);

    assert_eq!(buy_event_name, events::BUY_EVENT_NAME);
    assert_eq!(sell_event_name, events::SELL_EVENT_NAME);

    assert_ne!(
        buy_event_name, sell_event_name,
        "buy and sell event topics must be distinct"
    );

    for other in ALL_OTHER_EVENT_NAMES {
        assert_ne!(
            buy_event_name, *other,
            "buy event topic must not match {other:?}"
        );
        assert_ne!(
            sell_event_name, *other,
            "sell event topic must not match {other:?}"
        );
    }
}
