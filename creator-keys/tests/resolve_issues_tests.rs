use creator_keys::{events, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::Address as _, testutils::Events, testutils::Ledger, Address, Env, IntoVal, String,
    Symbol,
};

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    client.set_key_price(&admin, &100_i128);

    let creator = Address::generate(env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    (client, admin, creator)
}

#[test]
fn test_creator_supply_increments_sequential_and_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    // Start from a creator with supply 0.
    // Assert supply is 0 before any buy.
    assert_eq!(client.query_supply(&creator), 0);

    // Perform three sequential buy transactions, each for 1 key.
    // Assert supply is 1, 2, and 3 after each respective buy.
    client.buy_key(&creator, &buyer, &100_i128, &None);
    assert_eq!(client.query_supply(&creator), 1);

    client.buy_key(&creator, &buyer, &100_i128, &None);
    assert_eq!(client.query_supply(&creator), 2);

    client.buy_key(&creator, &buyer, &100_i128, &None);
    assert_eq!(client.query_supply(&creator), 3);

    // Assert a failed buy (insufficient funds / payment less than price) does not increment the supply.
    // Here, key price is 100, we try to pay 50.
    let result = client.try_buy_key(&creator, &buyer, &50_i128, &None);
    assert!(result.is_err());
    assert_eq!(client.query_supply(&creator), 3);
}

#[test]
fn test_buy_event_fields_on_success() {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy contract
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    // Seed key price
    client.set_key_price(&admin, &123_i128);

    // Register a seeded creator
    let creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "seeded_creator"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let buyer = Address::generate(&env);

    // Set a specific ledger sequence for testing sequence assertion
    let test_ledger = 456u32;
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = test_ledger;
    env.ledger().set(ledger_info);

    // Perform buy of 1 key
    client.buy_key(&creator, &buyer, &150_i128, &None);

    // Extract the contract events
    let events = env.events().all();
    let buy_event = events
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
        .unwrap();

    // Assert topics
    let topic_symbol: soroban_sdk::Symbol = buy_event
        .1
        .get(events::TOPIC_EVENT_NAME_INDEX)
        .unwrap()
        .into_val(&env);
    let topic_creator: soroban_sdk::Address = buy_event
        .1
        .get(events::TOPIC_CREATOR_INDEX)
        .unwrap()
        .into_val(&env);
    let topic_buyer: soroban_sdk::Address = buy_event
        .1
        .get(events::TOPIC_BUYER_INDEX)
        .unwrap()
        .into_val(&env);

    assert_eq!(topic_symbol, events::BUY_EVENT_NAME);
    assert_eq!(topic_creator, creator);
    assert_eq!(topic_buyer, buyer);

    // Extract data
    let event_data: events::KeysBoughtEvent = buy_event.2.into_val(&env);

    // Assert field values match the transaction inputs and current ledger number
    assert_eq!(event_data.buyer, buyer);
    assert_eq!(event_data.creator_id, creator);
    assert_eq!(event_data.quantity, 1u32);
    // price_paid matches bonding curve price at the supply step before the buy (which is 123)
    assert_eq!(event_data.price_paid, 123_i128);
    // ledger matches the transaction ledger number
    assert_eq!(event_data.ledger, test_ledger);
}
