//! Tests for optional co-creator revenue splits (#516).

mod contract_test_env;

use contract_test_env::{
    compute_expected_creator_fee, register_creator_keys, register_test_creator,
    set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{events, CoCreatorConfig, ContractError};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol,
};

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;
const CO_CREATOR_SHARE_BPS: u32 = 3000;

fn split_creator_fee(creator_fee: i128, share_bps: u32) -> (i128, i128) {
    let co_creator_fee = (creator_fee * share_bps as i128) / 10_000;
    (creator_fee - co_creator_fee, co_creator_fee)
}

fn co_creator_fee_events(env: &Env) -> std::vec::Vec<events::CoCreatorFeeEarned> {
    let mut payloads = std::vec::Vec::new();

    for (_, topics, data) in env.events().all().iter() {
        let event_name: Symbol = topics
            .get(events::TOPIC_EVENT_NAME_INDEX)
            .expect("event name topic should be present")
            .into_val(env);
        if event_name == events::CO_CREATOR_FEE_EARNED_EVENT_NAME {
            payloads.push(data.clone().into_val(env));
        }
    }

    payloads
}

fn register_creator_with_co_creator(
    env: &Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    handle: &str,
    share_bps: u32,
) -> (Address, Address, CoCreatorConfig) {
    let creator = Address::generate(env);
    let co_creator = Address::generate(env);
    let config = CoCreatorConfig {
        address: co_creator.clone(),
        share_bps,
    };

    client.register_creator(
        &creator,
        &String::from_str(env, handle),
        &None,
        &None,
        &None,
        &Some(config.clone()),
    );

    (creator, co_creator, config)
}

#[test]
fn test_register_creator_stores_optional_co_creator_config() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let plain_creator = register_test_creator(&env, &client, "plain");
    assert_eq!(client.get_co_creator(&plain_creator), None);

    let (creator, _, config) = register_creator_with_co_creator(
        &env,
        &client,
        "alice",
        CO_CREATOR_SHARE_BPS,
    );

    assert_eq!(client.get_co_creator(&creator), Some(config));
}

#[test]
fn test_register_creator_rejects_invalid_co_creator_share_bps() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    for (handle, share_bps) in [("zero", 0_u32), ("full", 10_000_u32)] {
        let creator = Address::generate(&env);
        let co_creator = Address::generate(&env);
        let config = CoCreatorConfig {
            address: co_creator,
            share_bps,
        };

        let result = client.try_register_creator(
            &creator,
            &String::from_str(&env, handle),
            &None,
            &None,
            &None,
            &Some(config),
        );

        assert_eq!(result, Err(Ok(ContractError::InvalidCoCreatorShare)));
    }
}

#[test]
fn test_buy_splits_creator_fee_between_creator_recipient_and_co_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let (creator, co_creator, _) = register_creator_with_co_creator(
        &env,
        &client,
        "alice",
        CO_CREATOR_SHARE_BPS,
    );
    let buyer = Address::generate(&env);
    let quote = client.get_buy_quote(&creator);
    let expected_creator_fee =
        compute_expected_creator_fee(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let (expected_recipient_fee, expected_co_creator_fee) =
        split_creator_fee(expected_creator_fee, CO_CREATOR_SHARE_BPS);

    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    assert_eq!(quote.creator_fee, expected_creator_fee);
    assert_eq!(
        client.get_creator_fee_balance(&creator),
        expected_recipient_fee
    );
    assert_eq!(
        client.get_co_creator_fee_balance(&creator, &co_creator),
        expected_co_creator_fee
    );
    assert_eq!(
        expected_recipient_fee + expected_co_creator_fee,
        quote.creator_fee
    );

    let payloads = co_creator_fee_events(&env);
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0].creator_id, creator);
    assert_eq!(payloads[0].co_creator, co_creator);
    assert_eq!(payloads[0].amount, expected_co_creator_fee);
}

#[test]
fn test_sell_splits_creator_fee_and_keeps_config_immutable() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let (creator, co_creator, config) = register_creator_with_co_creator(
        &env,
        &client,
        "alice",
        CO_CREATOR_SHARE_BPS,
    );
    let holder = Address::generate(&env);
    let buy_quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &holder, &buy_quote.total_amount, &None);

    let recipient_before = client.get_creator_fee_balance(&creator);
    let co_creator_before = client.get_co_creator_fee_balance(&creator, &co_creator);
    let sell_quote = client.get_sell_quote(&creator, &holder);
    let (expected_recipient_fee, expected_co_creator_fee) =
        split_creator_fee(sell_quote.creator_fee, CO_CREATOR_SHARE_BPS);

    client.sell_key(&creator, &holder, &None);

    let recipient_delta = client.get_creator_fee_balance(&creator) - recipient_before;
    let co_creator_delta =
        client.get_co_creator_fee_balance(&creator, &co_creator) - co_creator_before;

    assert_eq!(recipient_delta, expected_recipient_fee);
    assert_eq!(co_creator_delta, expected_co_creator_fee);
    assert_eq!(recipient_delta + co_creator_delta, sell_quote.creator_fee);
    assert_eq!(client.get_co_creator(&creator), Some(config));

    let payloads = co_creator_fee_events(&env);
    let last_payload = payloads.last().expect("co-creator fee event should emit");
    assert_eq!(last_payload.creator_id, creator);
    assert_eq!(last_payload.co_creator, co_creator);
    assert_eq!(last_payload.amount, expected_co_creator_fee);
}

#[test]
fn test_creator_without_co_creator_keeps_existing_fee_behavior() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "solo");
    let buyer = Address::generate(&env);
    let quote = client.get_buy_quote(&creator);

    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    assert_eq!(client.get_co_creator(&creator), None);
    assert_eq!(client.get_creator_fee_balance(&creator), quote.creator_fee);
    assert!(co_creator_fee_events(&env).is_empty());
}
