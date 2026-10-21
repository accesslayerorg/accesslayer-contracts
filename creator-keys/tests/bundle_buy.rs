//! Integration tests for the atomic `bundle_buy` / `get_bundle_quote`
//! entrypoints (issue #1057).
//!
//! Acceptance criteria verified:
//! - Full bundle reverts when any single item fails
//! - All items execute successfully when all within `max_price`
//! - `BundleExecuted` emitted with correct per-item breakdown
//! - `get_bundle_quote` returns accurate estimates for all items
//! - Bundle rejected when item count exceeds 10

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::bundle::{BundleItem, MAX_BUNDLE_ITEMS};
use creator_keys::{events, ContractError, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol, Vec,
};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 500;
const PROTOCOL_BPS: u32 = 500;
/// Generous ceiling for items that are expected to succeed.
const LOOSE_MAX: i128 = 1_000_000_000;

fn setup(env: &Env) -> CreatorKeysContractClient<'_> {
    let (client, _) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let protocol_recipient = Address::generate(env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);
    client
}

fn creators(env: &Env, client: &CreatorKeysContractClient<'_>, n: u32) -> std::vec::Vec<Address> {
    (0..n)
        .map(|i| register_test_creator(env, client, &std::format!("creator{i}")))
        .collect()
}

fn item(key_id: &Address, amount: u32, max_price: i128) -> BundleItem {
    BundleItem {
        key_id: key_id.clone(),
        amount,
        max_price,
    }
}

#[test]
fn all_items_execute_when_within_max_price() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let keys = creators(&env, &client, 3);
    let buyer = Address::generate(&env);

    let items = Vec::from_array(
        &env,
        [
            item(&keys[0], 2, LOOSE_MAX),
            item(&keys[1], 1, LOOSE_MAX),
            item(&keys[2], 3, LOOSE_MAX),
        ],
    );
    let receipt = client.bundle_buy(&buyer, &items);

    assert_eq!(receipt.items.len(), 3);
    for (i, expected) in [2u32, 1, 3].iter().enumerate() {
        assert_eq!(client.get_key_balance(&keys[i], &buyer), *expected);
        assert_eq!(client.get_total_key_supply(&keys[i]), *expected);
    }
}

#[test]
fn bundle_executed_event_has_correct_per_item_breakdown() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let keys = creators(&env, &client, 3);
    let buyer = Address::generate(&env);

    let items = Vec::from_array(
        &env,
        [
            item(&keys[0], 2, LOOSE_MAX),
            item(&keys[1], 1, LOOSE_MAX),
            item(&keys[2], 3, LOOSE_MAX),
        ],
    );
    let quote = client.get_bundle_quote(&items);
    let receipt = client.bundle_buy(&buyer, &items);

    let log = env.events().all();
    let matching: std::vec::Vec<_> = log
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::BUNDLE_EXECUTED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(matching.len(), 1, "exactly one BundleExecuted event");

    let (_, topics, data) = &matching[0];
    let topic_buyer: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic_buyer, buyer);

    let event: events::BundleExecutedEvent = data.clone().into_val(&env);
    assert_eq!(event, receipt);
    assert_eq!(event.buyer, buyer);
    assert_eq!(event.items.len(), 3);

    let mut sum = 0i128;
    for i in 0..3u32 {
        let line = event.items.get(i).unwrap();
        let q = quote.items.get(i).unwrap();
        assert_eq!(line.key_id, keys[i as usize]);
        assert_eq!(line.amount, items.get(i).unwrap().amount);
        assert!(line.price_paid > 0);
        assert_eq!(line.price_paid, q.estimated_cost);
        sum += line.price_paid;
    }
    assert_eq!(event.total_cost, sum);
    assert_eq!(event.total_cost, quote.total_cost);
}

#[test]
fn full_bundle_reverts_when_an_item_exceeds_max_price() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let keys = creators(&env, &client, 3);
    let buyer = Address::generate(&env);

    let probe = Vec::from_array(&env, [item(&keys[2], 3, LOOSE_MAX)]);
    let cost_of_last = client.get_bundle_quote(&probe).total_cost;

    // Items 0 and 1 are fine; item 2's ceiling is one unit below its cost.
    let items = Vec::from_array(
        &env,
        [
            item(&keys[0], 2, LOOSE_MAX),
            item(&keys[1], 1, LOOSE_MAX),
            item(&keys[2], 3, cost_of_last - 1),
        ],
    );
    let result = client.try_bundle_buy(&buyer, &items);
    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));

    // Nothing from the earlier, individually valid items may persist.
    for key in &keys {
        assert_eq!(client.get_key_balance(key, &buyer), 0);
        assert_eq!(client.get_total_key_supply(key), 0);
    }
    let bundle_events = env
        .events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::BUNDLE_EXECUTED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .count();
    assert_eq!(bundle_events, 0);
}

#[test]
fn full_bundle_reverts_when_a_later_item_fails_for_another_reason() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let keys = creators(&env, &client, 2);
    let buyer = Address::generate(&env);
    let unregistered = Address::generate(&env);

    let items = Vec::from_array(
        &env,
        [
            item(&keys[0], 2, LOOSE_MAX),
            item(&keys[1], 1, LOOSE_MAX),
            item(&unregistered, 1, LOOSE_MAX),
        ],
    );
    let result = client.try_bundle_buy(&buyer, &items);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));

    for key in &keys {
        assert_eq!(client.get_key_balance(key, &buyer), 0);
        assert_eq!(client.get_total_key_supply(key), 0);
    }
}

#[test]
fn full_bundle_reverts_when_an_item_hits_its_supply_cap() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let keys = creators(&env, &client, 2);
    let buyer = Address::generate(&env);

    // Cap key 1 at a single key, then ask for two: the item cannot be filled.
    client.set_supply_cap(&keys[1], &1u32);

    let items = Vec::from_array(
        &env,
        [item(&keys[0], 1, LOOSE_MAX), item(&keys[1], 2, LOOSE_MAX)],
    );
    let result = client.try_bundle_buy(&buyer, &items);
    assert_eq!(result, Err(Ok(ContractError::BundleItemUnfilled)));
    assert_eq!(client.get_total_key_supply(&keys[0]), 0);
    assert_eq!(client.get_total_key_supply(&keys[1]), 0);
}

#[test]
fn quote_matches_execution_on_curve_that_moves_between_items() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let keys = creators(&env, &client, 3);
    let buyer = Address::generate(&env);
    let other = Address::generate(&env);

    // Pre-seed different supplies so each item starts at a different price.
    client.bundle_buy(
        &other,
        &Vec::from_array(
            &env,
            [item(&keys[0], 4, LOOSE_MAX), item(&keys[2], 7, LOOSE_MAX)],
        ),
    );

    let items = Vec::from_array(
        &env,
        [
            item(&keys[0], 3, LOOSE_MAX),
            item(&keys[1], 5, LOOSE_MAX),
            item(&keys[2], 2, LOOSE_MAX),
        ],
    );
    let quote = client.get_bundle_quote(&items);
    assert_eq!(quote.items.len(), 3);

    let receipt = client.bundle_buy(&buyer, &items);
    assert_eq!(receipt.items.len(), 3);
    let mut total = 0i128;
    for i in 0..3u32 {
        let q = quote.items.get(i).unwrap();
        let r = receipt.items.get(i).unwrap();
        assert_eq!(q.key_id, r.key_id);
        assert_eq!(q.amount, r.amount);
        assert_eq!(q.estimated_cost, r.price_paid, "item {i} estimate");
        total += q.estimated_cost;
    }
    assert_eq!(quote.total_cost, total);
    assert_eq!(quote.total_cost, receipt.total_cost);
}

#[test]
fn quote_reports_price_impact_per_item() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let keys = creators(&env, &client, 2);

    let items = Vec::from_array(
        &env,
        [item(&keys[0], 1, LOOSE_MAX), item(&keys[1], 6, LOOSE_MAX)],
    );
    let quote = client.get_bundle_quote(&items);

    for i in 0..2u32 {
        let q = quote.items.get(i).unwrap();
        let before = client.query_price(&q.key_id, &0u64);
        let after = client.query_price(&q.key_id, &(q.amount as u64));
        let expected = if before > 0 && after > before {
            (((after - before) as u128) * 10_000 / (before as u128)) as u32
        } else {
            0
        };
        assert_eq!(q.price_impact_bps, expected);
    }
    // Buying more keys moves the price at least as much.
    assert!(
        quote.items.get(1).unwrap().price_impact_bps
            >= quote.items.get(0).unwrap().price_impact_bps
    );
}

#[test]
fn bundle_of_exactly_ten_items_is_accepted() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let keys = creators(&env, &client, MAX_BUNDLE_ITEMS);
    let buyer = Address::generate(&env);

    let mut items = Vec::new(&env);
    for key in &keys {
        items.push_back(item(key, 1, LOOSE_MAX));
    }
    assert_eq!(client.get_bundle_quote(&items).items.len(), 10);
    let receipt = client.bundle_buy(&buyer, &items);
    assert_eq!(receipt.items.len(), 10);
    for key in &keys {
        assert_eq!(client.get_key_balance(key, &buyer), 1);
    }
}

#[test]
fn bundle_with_more_than_ten_items_is_rejected() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let keys = creators(&env, &client, MAX_BUNDLE_ITEMS + 1);
    let buyer = Address::generate(&env);

    let mut items = Vec::new(&env);
    for key in &keys {
        items.push_back(item(key, 1, LOOSE_MAX));
    }
    assert_eq!(items.len(), 11);

    assert_eq!(
        client.try_bundle_buy(&buyer, &items),
        Err(Ok(ContractError::BundleSizeExceeded))
    );
    assert_eq!(
        client.try_get_bundle_quote(&items),
        Err(Ok(ContractError::BundleSizeExceeded))
    );
    for key in &keys {
        assert_eq!(client.get_total_key_supply(key), 0);
    }
}
