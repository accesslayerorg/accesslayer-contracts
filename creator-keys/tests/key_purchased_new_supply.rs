//! Unit tests for issue #698 — the `KeyPurchased` (buy) event must emit
//! `new_supply` equal to the on-chain total supply **after** the purchase.
//!
//! Acceptance criteria:
//! - `new_supply` in event equals post-buy `get_total_key_supply`
//! - Sequential buys emit events with correctly incrementing `new_supply`
//! - No event emitted on a failed buy
//! - `new_supply` starts at 1 for the very first buy from supply 0

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1_000;

/// Extract the `KeysBoughtEvent` payload from the most recent buy event in the log.
fn last_buy_event(env: &soroban_sdk::Env) -> events::KeysBoughtEvent {
    let log = env.events().all();
    let (_, _, data) = log
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
        .expect("no buy event found in event log");
    data.into_val(env)
}

/// Return the number of buy events in the event log.
fn buy_event_count(env: &soroban_sdk::Env) -> usize {
    env.events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(env);
                    name == events::BUY_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .count()
}

// ── AC1: new_supply equals post-buy get_total_key_supply ─────────────────────

#[test]
fn test_single_buy_from_supply_zero_emits_new_supply_one() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &KEY_PRICE);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    let payload = last_buy_event(&env);
    let on_chain_supply = client.get_total_key_supply(&creator);

    assert_eq!(
        payload.new_supply, 1,
        "first buy from supply 0 must emit new_supply: 1"
    );
    assert_eq!(
        payload.new_supply, on_chain_supply,
        "event new_supply must equal get_total_key_supply after the buy"
    );
}

#[test]
fn test_buy_from_supply_ten_emits_new_supply_eleven() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &KEY_PRICE);

    let creator = register_test_creator(&env, &client, "bob");

    // Buy 10 keys to establish a base supply of 10
    for _ in 0..10 {
        let buyer = Address::generate(&env);
        client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    }

    assert_eq!(
        client.get_total_key_supply(&creator),
        10,
        "supply should be 10 before the test buy"
    );

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    let payload = last_buy_event(&env);
    let on_chain_supply = client.get_total_key_supply(&creator);

    assert_eq!(
        payload.new_supply, 11,
        "buy from supply 10 must emit new_supply: 11"
    );
    assert_eq!(
        payload.new_supply, on_chain_supply,
        "event new_supply must equal get_total_key_supply after the buy"
    );
}

// ── AC2: sequential buys emit incrementing new_supply ────────────────────────

#[test]
fn test_two_sequential_buys_emit_incrementing_new_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &KEY_PRICE);

    let creator = register_test_creator(&env, &client, "carol");
    let buyer1 = Address::generate(&env);
    let buyer2 = Address::generate(&env);

    client.buy_key(&creator, &buyer1, &KEY_PRICE, &None);
    let first_payload = last_buy_event(&env);

    client.buy_key(&creator, &buyer2, &KEY_PRICE, &None);
    let second_payload = last_buy_event(&env);

    assert_eq!(
        first_payload.new_supply, 1,
        "first buy new_supply must be 1"
    );
    assert_eq!(
        second_payload.new_supply, 2,
        "second buy new_supply must be 2"
    );
    assert_eq!(
        second_payload.new_supply,
        first_payload.new_supply + 1,
        "new_supply must increment by 1 between sequential buys"
    );
}

#[test]
fn test_five_sequential_buys_new_supply_increments_correctly() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &KEY_PRICE);

    let creator = register_test_creator(&env, &client, "dave");

    for expected_supply in 1u32..=5 {
        let buyer = Address::generate(&env);
        client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
        let payload = last_buy_event(&env);
        assert_eq!(
            payload.new_supply, expected_supply,
            "buy #{expected_supply}: new_supply must equal {expected_supply}"
        );
        assert_eq!(
            payload.new_supply,
            client.get_total_key_supply(&creator),
            "buy #{expected_supply}: new_supply must match on-chain get_total_key_supply"
        );
    }
}

// ── AC3: no event emitted on a failed buy ─────────────────────────────────────

#[test]
fn test_no_buy_event_emitted_when_buy_fails_due_to_insufficient_payment() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &KEY_PRICE);

    let creator = register_test_creator(&env, &client, "eve");
    let buyer = Address::generate(&env);

    let count_before = buy_event_count(&env);

    // Attempt to buy with payment below the key price — should fail
    let result = client.try_buy_key(&creator, &buyer, &(KEY_PRICE - 1), &None);
    assert!(result.is_err(), "buy with insufficient payment must fail");

    let count_after = buy_event_count(&env);
    assert_eq!(
        count_before, count_after,
        "no buy event must be emitted after a failed buy"
    );
}

// ── AC4: new_supply matches get_total_key_supply return value ─────────────────

#[test]
fn test_new_supply_in_event_matches_get_total_key_supply_after_each_buy() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &KEY_PRICE);

    let creator = register_test_creator(&env, &client, "frank");

    for _ in 0..3 {
        let buyer = Address::generate(&env);
        client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
        let payload = last_buy_event(&env);
        let on_chain = client.get_total_key_supply(&creator);
        assert_eq!(
            payload.new_supply, on_chain,
            "event new_supply must equal get_total_key_supply immediately after the buy"
        );
    }
}
