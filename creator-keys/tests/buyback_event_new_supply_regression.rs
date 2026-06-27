//! Regression test: `KeysBoughtBack` event contains correct `new_supply` value.
//!
//! The `new_supply` field in the `KeysBoughtBackEvent` must equal the pre-buyback
//! supply minus the buyback amount. A full buyback of all supply must result in
//! `new_supply` of zero.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_pricing_and_fees};
use creator_keys::events;
use soroban_sdk::{testutils::Events, Address, Env, IntoVal};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

fn setup(env: &Env) -> (creator_keys::CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

fn self_buy_keys(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    count: u32,
) {
    for _ in 0..count {
        let quote = client.get_buy_quote(creator);
        client.buy_key(creator, creator, &quote.total_amount, &None);
    }
}

fn last_buyback_payload(env: &Env) -> events::KeysBoughtBackEvent {
    let event_log = env.events().all();
    let last = event_log.last().unwrap();
    last.2.into_val(env)
}

#[test]
fn test_buyback_event_new_supply_equals_supply_before_minus_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 5);

    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(supply_before, 5, "setup: expected supply of 5");

    let buyback_amount: u32 = 3;
    let total_cost = client.get_buyback_quote(&creator, &buyback_amount);
    client.buyback(&creator, &creator, &buyback_amount, &total_cost, &None);

    let payload = last_buyback_payload(&env);
    assert_eq!(
        payload.new_supply,
        supply_before - buyback_amount,
        "new_supply must equal pre-buyback supply minus buyback amount"
    );
}

#[test]
fn test_buyback_full_supply_results_in_new_supply_zero() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 3);

    let total_cost = client.get_buyback_quote(&creator, &3);
    client.buyback(&creator, &creator, &3, &total_cost, &None);

    let payload = last_buyback_payload(&env);
    assert_eq!(
        payload.new_supply, 0,
        "full buyback of all supply must result in new_supply of zero"
    );
}

#[test]
fn test_buyback_event_new_supply_nonzero_when_supply_remains() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 4);

    let total_cost = client.get_buyback_quote(&creator, &1);
    client.buyback(&creator, &creator, &1, &total_cost, &None);

    let payload = last_buyback_payload(&env);
    assert!(
        payload.new_supply > 0,
        "new_supply must be nonzero when supply remains after buyback"
    );
    assert_eq!(
        payload.new_supply, 3,
        "new_supply must equal 3 after buying back 1 from supply of 4"
    );
}
