//! Integration tests for the `KeysSold` event carrying the seller and the
//! post-sell supply (#736).
//!
//! Downstream indexers rebuild supply from the trade event stream, so the sell
//! event has to name who sold and what the supply became — and that figure has
//! to agree with what the contract reports afterwards, or the indexed supply
//! drifts from chain state.
//!
//! `sell_key` sells one key per call, so the issue's "sell of 3 keys" is three
//! calls; the event asserted below is the one emitted by the third.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{events, ContractError, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

const STARTING_SUPPLY: u32 = 10;
const KEYS_SOLD: u32 = 3;
const EXPECTED_SUPPLY_AFTER: u32 = STARTING_SUPPLY - KEYS_SOLD;

/// Deploy the contract with pricing, fees, a protocol admin and one creator.
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

/// Buy `count` keys for `buyer`, one call per key.
fn buy_keys(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
    count: u32,
) {
    for _ in 0..count {
        let quote = client.get_buy_quote(creator);
        client.buy_key(creator, buyer, &quote.total_amount, &None);
    }
}

/// Decode the single `KeysSold` event in the log, failing if there is not exactly one.
fn expect_one_sell_event(env: &Env) -> events::KeysSoldEvent {
    let log = env.events().all();
    let sell_events: std::vec::Vec<_> = log
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(env);
                    name == events::SELL_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .collect();

    assert_eq!(
        sell_events.len(),
        1,
        "exactly one sell event must be emitted per sell"
    );
    let (_, _, data) = sell_events[0].clone();
    data.into_val(env)
}

/// Count the `KeysSold` events currently in the log.
fn sell_event_count(env: &Env) -> u32 {
    env.events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(env);
                    name == events::SELL_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .count() as u32
}

// ---------------------------------------------------------------------------
// The event names the seller and the supply the sell produced
// ---------------------------------------------------------------------------

#[test]
fn test_sell_event_reports_seller_and_new_supply_after_selling_three_of_ten() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let seller = Address::generate(&env);

    buy_keys(&client, &creator, &seller, STARTING_SUPPLY);
    assert_eq!(client.get_total_key_supply(&creator), STARTING_SUPPLY);
    assert_eq!(client.get_key_balance(&creator, &seller), STARTING_SUPPLY);

    // Sell the first two keys, then clear the log so only the third sell's
    // event is under assertion.
    client.sell_key(&creator, &seller, &None);
    client.sell_key(&creator, &seller, &None);
    env.events().all();

    let returned_supply = client.sell_key(&creator, &seller, &None);
    let event = expect_one_sell_event(&env);

    assert_eq!(
        event.seller, seller,
        "seller field must be the wallet that sold"
    );
    assert_eq!(event.creator_id, creator, "creator_id field must match");
    assert_eq!(event.quantity, 1, "each sell_key call sells one key");
    assert_eq!(
        event.new_supply, EXPECTED_SUPPLY_AFTER,
        "new_supply must be 7 after selling 3 of 10"
    );

    // The event, the call's return value and the on-chain view must not disagree.
    assert_eq!(
        event.new_supply,
        client.get_total_key_supply(&creator),
        "new_supply must match the supply the contract reports after the sell"
    );
    assert_eq!(
        event.new_supply, returned_supply,
        "new_supply must match the value sell_key returned"
    );
}

#[test]
fn test_sell_event_new_supply_tracks_every_sell_down_to_zero() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let seller = Address::generate(&env);

    buy_keys(&client, &creator, &seller, STARTING_SUPPLY);

    // Each successive sell must report the supply that sell produced.
    for expected_supply in (0..STARTING_SUPPLY).rev() {
        env.events().all();
        client.sell_key(&creator, &seller, &None);

        let event = expect_one_sell_event(&env);
        assert_eq!(
            event.new_supply, expected_supply,
            "new_supply must be {expected_supply} after this sell"
        );
        assert_eq!(event.new_supply, client.get_total_key_supply(&creator));
    }
}

#[test]
fn test_sell_event_names_the_selling_wallet_not_another_holder() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let seller = Address::generate(&env);
    let other_holder = Address::generate(&env);

    buy_keys(&client, &creator, &seller, 5);
    buy_keys(&client, &creator, &other_holder, 5);

    env.events().all();
    client.sell_key(&creator, &seller, &None);

    let event = expect_one_sell_event(&env);
    assert_eq!(event.seller, seller);
    assert_ne!(
        event.seller, other_holder,
        "the event must not attribute the sale to an uninvolved holder"
    );
    assert_eq!(
        event.new_supply, 9,
        "new_supply counts all holders' keys, not just the seller's"
    );
}

// ---------------------------------------------------------------------------
// A rejected sell emits nothing
// ---------------------------------------------------------------------------

#[test]
fn test_no_sell_event_emitted_when_the_sell_is_rejected() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);
    let stranger = Address::generate(&env);

    buy_keys(&client, &creator, &holder, STARTING_SUPPLY);

    env.events().all();
    let supply_before = client.get_total_key_supply(&creator);

    // A wallet holding nothing cannot sell.
    assert_eq!(
        client.try_sell_key(&creator, &stranger, &None),
        Err(Ok(ContractError::InsufficientBalance))
    );

    assert_eq!(
        sell_event_count(&env),
        0,
        "a rejected sell must not emit a sell event"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "a rejected sell must not move supply"
    );
}

#[test]
fn test_no_sell_event_emitted_when_the_sell_misses_its_slippage_floor() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    buy_keys(&client, &creator, &holder, STARTING_SUPPLY);

    env.events().all();

    // Demand more than the sale can possibly return.
    assert_eq!(
        client.try_sell_key(&creator, &holder, &Some(i128::MAX)),
        Err(Ok(ContractError::SlippageExceeded))
    );

    assert_eq!(
        sell_event_count(&env),
        0,
        "a sell rejected on slippage must not emit a sell event"
    );
    assert_eq!(client.get_total_key_supply(&creator), STARTING_SUPPLY);
    assert_eq!(client.get_key_balance(&creator, &holder), STARTING_SUPPLY);
}
