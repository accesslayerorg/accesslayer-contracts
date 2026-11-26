//! Integration tests for the admin-configurable per-key sell price floor (#1097).

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, register_creator_keys, register_test_creator, set_curve_slope,
    set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{
    events, events::PRICE_FLOOR_TRIGGERED_EVENT_NAME, ContractError, CreatorKeysContractClient,
    SwapDirection,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1_000;
const SLOPE: i128 = 100;

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, 9_000, 1_000);
    set_curve_slope(env, &client, SLOPE);
    let creator = register_test_creator(env, &client, "floor_creator");
    let holder = Address::generate(env);
    (client, admin, creator, holder)
}

fn stock(env: &Env, client: &CreatorKeysContractClient<'_>, creator: &Address, holder: &Address) {
    let cost: i128 = (0..10u32).map(|s| KEY_PRICE + SLOPE * s as i128).sum();
    client.buy_keys(creator, holder, &10u32, &cost, &None);
    let mut ledger = env.ledger().get();
    ledger.sequence_number += 1;
    env.ledger().set(ledger);
}

#[test]
fn floor_defaults_to_zero_and_is_admin_only() {
    let env = test_env_with_auths();
    let (client, admin, creator, _holder) = setup(&env);
    assert_eq!(client.get_price_floor(&creator), 0);

    let not_admin = Address::generate(&env);
    assert_eq!(
        client.try_set_price_floor(&not_admin, &creator, &500),
        Err(Ok(ContractError::Unauthorized))
    );
    client.set_price_floor(&admin, &creator, &1_500);
    assert_eq!(client.get_price_floor(&creator), 1_500);
    assert_eq!(client.get_price_floor(&Address::generate(&env)), 0);
}

#[test]
fn sell_at_or_below_floor_is_blocked_with_event() {
    for floor_adjust in [0i128, 1] {
        let env = test_env_with_auths();
        let (client, admin, creator, holder) = setup(&env);
        stock(&env, &client, &creator, &holder);
        let supply = client.get_total_key_supply(&creator) as u64;
        let current_price = client.get_price_at_supply(&creator, &supply);
        client.set_price_floor(&admin, &creator, &(current_price + floor_adjust));

        let result = client.try_swap(&creator, &holder, &1u32, &SwapDirection::Sell, &0u32);
        let log = env.events().all();
        let before = capture_snapshot(&client, &creator, &holder);
        let after = capture_snapshot(&client, &creator, &holder);
        assert_eq!(result, Err(Ok(ContractError::PriceFloorBreached)));
        before.assert_unchanged(&after);

        let floor_events: std::vec::Vec<_> = log
            .iter()
            .filter(|(_, topics, _)| {
                topics
                    .get(events::TOPIC_EVENT_NAME_INDEX)
                    .map(|v| {
                        let name: Symbol = v.into_val(&env);
                        name == PRICE_FLOOR_TRIGGERED_EVENT_NAME
                    })
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(floor_events.len(), 1);
        let (_, topics, data) = &floor_events[0];
        let event_key: Address = topics.get(1).unwrap().into_val(&env);
        assert_eq!(event_key, creator);
        let event: events::PriceFloorTriggeredEvent = data.clone().into_val(&env);
        assert_eq!(event.key_id, creator);
        assert_eq!(event.attempted_price, current_price);
        assert_eq!(event.floor_price, current_price + floor_adjust);
    }
}

#[test]
fn sell_above_floor_succeeds() {
    let env = test_env_with_auths();
    let (client, admin, creator, holder) = setup(&env);
    stock(&env, &client, &creator, &holder);
    let supply = client.get_total_key_supply(&creator) as u64;
    let current_price = client.get_price_at_supply(&creator, &supply);
    client.set_price_floor(&admin, &creator, &(current_price - 1));

    let _ = client.swap(&creator, &holder, &1u32, &SwapDirection::Sell, &0u32);
    assert_eq!(client.get_total_key_supply(&creator), 9);
    assert_eq!(client.get_key_balance(&creator, &holder), 9);
}

#[test]
fn buy_is_unaffected_by_floor() {
    let env = test_env_with_auths();
    let (client, admin, creator, buyer) = setup(&env);
    client.set_price_floor(&admin, &creator, &1_000_000);

    let _ = client.swap(&creator, &buyer, &1u32, &SwapDirection::Buy, &0u32);
    assert_eq!(client.get_total_key_supply(&creator), 1);
}

#[test]
fn floor_blocks_direct_sell_and_batch_sell_paths() {
    let env = test_env_with_auths();
    let (client, admin, creator, holder) = setup(&env);
    stock(&env, &client, &creator, &holder);
    client.set_price_floor(&admin, &creator, &1_000_000);

    assert_eq!(
        client.try_sell_key(&creator, &holder, &None),
        Err(Ok(ContractError::PriceFloorBreached))
    );
    assert_eq!(client.get_total_key_supply(&creator), 10);
}
