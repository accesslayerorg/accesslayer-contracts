//! Tests verifying that the buy-key event includes the payment amount.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{symbol_short, testutils::Address as _, testutils::Events, Env, IntoVal, String};

#[test]
fn test_buy_key_event_includes_payment_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let buyer = soroban_sdk::Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"));
    let supply = client.buy_key(&creator, &buyer, &150i128);
    assert_eq!(supply, 1);

    let events = env.events().all();
    // Last event is the buy event
    let buy_event = events.last().unwrap();
    // Data is (supply, payment)
    let (event_supply, event_payment): (u32, i128) = buy_event.2.into_val(&env);
    assert_eq!(event_supply, 1u32);
    assert_eq!(event_payment, 150i128);
}

#[test]
fn test_buy_key_event_topics_include_creator_and_buyer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let buyer = soroban_sdk::Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"));
    client.buy_key(&creator, &buyer, &200i128);

    let events = env.events().all();
    let buy_event = events.last().unwrap();

    // Topics: (symbol "buy", creator address, buyer address)
    let topic_symbol: soroban_sdk::Symbol = buy_event.1.get(0).unwrap().into_val(&env);
    let topic_creator: soroban_sdk::Address = buy_event.1.get(1).unwrap().into_val(&env);
    let topic_buyer: soroban_sdk::Address = buy_event.1.get(2).unwrap().into_val(&env);

    assert_eq!(topic_symbol, symbol_short!("buy"));
    assert_eq!(topic_creator, creator);
    assert_eq!(topic_buyer, buyer);
}
