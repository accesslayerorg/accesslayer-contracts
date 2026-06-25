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
