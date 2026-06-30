//! Tests for dividend-related event emission.

mod contract_test_env;

use contract_test_env::{
    distribute_test_dividend, register_creator_keys, register_test_creator, set_pricing_and_fees,
    test_env_with_auths, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use creator_keys::events::{
    dividend_claimed_topics, dividend_distributed_topics, DividendClaimedEvent,
    DividendDistributedEvent,
};
use soroban_sdk::{testutils::Address as _, testutils::Events, Address, IntoVal};

#[test]
fn test_distribute_dividend_event_topics_and_payload() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100, &None);

    let distributor = Address::generate(&env);
    let amount = 10_000i128;
    distribute_test_dividend(&client, &creator, &distributor, amount);

    let events = env.events().all();
    let (topics, data) = events
        .iter()
        .rev()
        .find_map(|(_, topics, data)| {
            if topics == dividend_distributed_topics(&creator).into_val(&env) {
                Some((topics, data))
            } else {
                None
            }
        })
        .expect("DividendDistributed event not found");

    let _ = topics; // topics already validated by find_map predicate
    let event: DividendDistributedEvent = data.into_val(&env);
    assert_eq!(event.creator, creator);
    assert_eq!(event.total_amount, amount);
    assert_eq!(event.snapshot_supply, 1);
}

#[test]
fn test_claim_dividend_event_topics_and_payload() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    let claimed = client.claim_dividend(&creator, &buyer);

    let events = env.events().all();
    let (_, data) = events
        .iter()
        .rev()
        .find_map(|(_, topics, data)| {
            if topics == dividend_claimed_topics(&creator, &buyer).into_val(&env) {
                Some((topics, data))
            } else {
                None
            }
        })
        .expect("DividendClaimed event not found");

    let event: DividendClaimedEvent = data.into_val(&env);
    assert_eq!(event.creator, creator);
    assert_eq!(event.claimant, buyer);
    assert_eq!(event.amount, claimed);
}

#[test]
fn test_distribute_dividend_event_fields_individual_assertions() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    // Set pricing and fees with 10% protocol fee
    set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    
    // Distribute to a creator with a known supply (e.g. 2 keys bought)
    client.buy_key(&creator, &buyer, &100, &None);
    client.buy_key(&creator, &buyer, &100, &None);
    
    // Set env ledger to a positive non-zero sequence
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 12345;
    env.ledger().set(ledger_info);

    let distributor = Address::generate(&env);
    let gross_amount = 20_000i128;
    distribute_test_dividend(&client, &creator, &distributor, gross_amount);

    let events = env.events().all();
    let (_, data) = events
        .iter()
        .rev()
        .find_map(|(_, topics, data)| {
            if topics == dividend_distributed_topics(&creator).into_val(&env) {
                Some((topics, data))
            } else {
                None
            }
        })
        .expect("DividendDistributed event not found");

    let event: DividendDistributedEvent = data.into_val(&env);
    
    // Assert creator matches the creator used
    assert_eq!(event.creator, creator);
    
    // Assert total_amount reflects the gross amount before protocol fee deduction (not the net amount)
    assert_eq!(event.total_amount, gross_amount);
    
    // Assert snapshot_supply matches total supply at distribution time
    assert_eq!(event.snapshot_supply, 2);
    
    // Assert ledger is a positive non-zero value
    assert!(event.ledger > 0);
    assert_eq!(event.ledger, 12345);
}

