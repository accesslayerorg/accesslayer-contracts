//! Regression tests for buy event buyer address field integrity.
//!
//! Verifies that the buyer address in the emitted buy event matches
//! the address that initiated the buy call and that the field is present.

use creator_keys::{events, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol, Val,
};

const KEY_PRICE: i128 = 100;

#[test]
fn test_buy_event_buyer_address_matches_caller() {
    let env = Env::default();
    env.mock_all_auths();

    // Setup
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Configure contract
    client.set_key_price(&admin, &KEY_PRICE);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Clear any prior events then perform the buy
    env.events().all(); // clear
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    // Extract and verify the buy event
    let event_log = env.events().all();
    let (_, topics, _) = event_log
        .last()
        .expect("buy event should be present in event log");

    // Verify event name is buy
    let event_name: Symbol = topics
        .get(events::TOPIC_EVENT_NAME_INDEX)
        .expect("event name topic should be present")
        .into_val(&env);
    assert_eq!(event_name, events::BUY_EVENT_NAME);

    // Verify buyer address field matches caller
    let event_buyer: Address = topics
        .get(events::TOPIC_BUYER_INDEX)
        .expect("buyer address field should be present in event")
        .into_val(&env);
    assert_eq!(
        event_buyer, buyer,
        "buyer address in event must match the caller"
    );

    // Verify creator address field is present and correct
    let event_creator: Address = topics
        .get(events::TOPIC_CREATOR_INDEX)
        .expect("creator address field should be present in event")
        .into_val(&env);
    assert_eq!(event_creator, creator);
}

#[test]
fn test_buy_event_buyer_address_field_is_non_zero() {
    let env = Env::default();
    env.mock_all_auths();

    // Setup
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Configure and execute
    client.set_key_price(&admin, &KEY_PRICE);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    // Verify the buyer address field is present and matches expected
    let event_log = env.events().all();
    let (_, topics, _) = event_log.last().expect("buy event should be present");

    let buyer_field: Option<Val> = topics.get(events::TOPIC_BUYER_INDEX);
    assert!(
        buyer_field.is_some(),
        "buyer address field must be present in buy event"
    );

    let event_buyer: Address = buyer_field.unwrap().into_val(&env);
    // An Address cannot be zero in Soroban (it's always a valid address),
    // so we verify it's the expected buyer to confirm field integrity
    assert_eq!(event_buyer, buyer);
}
