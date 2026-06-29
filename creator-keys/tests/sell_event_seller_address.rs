//! Regression test for sell event seller address field integrity.
//!
//! Verifies that the seller address in the emitted sell event matches
//! the address that initiated the sell call, preventing potential issues
//! where event reporting and actual execution could diverge.

use creator_keys::{events, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol, Val,
};

const KEY_PRICE: i128 = 100;

#[test]
fn test_sell_event_seller_address_matches_caller() {
    let env = Env::default();
    env.mock_all_auths();

    // Setup
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let seller = Address::generate(&env);

    // Configure contract
    client.set_key_price(&admin, &KEY_PRICE);
    client.register_creator(
        &creator,
        &String::from_str(&env, "alice"),
        &None,
        &None,
        &None,
        &None,
    );

    // Buyer purchases keys
    client.buy_key(&creator, &seller, &KEY_PRICE, &None);

    // Clear event log and then perform the sell
    env.events().all(); // Clear existing events
    client.sell_key(&creator, &seller, &None);

    // Extract and verify the sell event
    let event_log = env.events().all();
    let (_, topics, _) = event_log
        .last()
        .expect("sell event should be present in event log");

    // Verify event name is sell
    let event_name: Symbol = topics
        .get(events::TOPIC_EVENT_NAME_INDEX)
        .expect("event name topic should be present")
        .into_val(&env);
    assert_eq!(event_name, events::SELL_EVENT_NAME);

    // Verify seller address field matches caller
    let event_seller: Address = topics
        .get(events::TOPIC_BUYER_INDEX)
        .expect("seller address field should be present in event")
        .into_val(&env);
    assert_eq!(
        event_seller, seller,
        "seller address in event must match the caller"
    );

    // Verify creator address field is present and correct
    let event_creator: Address = topics
        .get(events::TOPIC_CREATOR_INDEX)
        .expect("creator address field should be present in event")
        .into_val(&env);
    assert_eq!(event_creator, creator);
}

#[test]
fn test_sell_event_seller_address_field_is_non_zero() {
    let env = Env::default();
    env.mock_all_auths();

    // Setup
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let seller = Address::generate(&env);

    // Configure and execute
    client.set_key_price(&admin, &KEY_PRICE);
    client.register_creator(
        &creator,
        &String::from_str(&env, "alice"),
        &None,
        &None,
        &None,
        &None,
    );
    client.buy_key(&creator, &seller, &KEY_PRICE, &None);
    client.sell_key(&creator, &seller, &None);

    // Verify the seller address field is present and non-zero
    let event_log = env.events().all();
    let (_, topics, _) = event_log.last().expect("sell event should be present");

    let seller_field: Option<Val> = topics.get(events::TOPIC_BUYER_INDEX);
    assert!(
        seller_field.is_some(),
        "seller address field must be present in sell event"
    );

    let event_seller: Address = seller_field.unwrap().into_val(&env);
    // An Address cannot be zero in Soroban (it's always a valid address),
    // but we verify it's the expected seller to confirm field integrity
    assert_eq!(event_seller, seller);
}
