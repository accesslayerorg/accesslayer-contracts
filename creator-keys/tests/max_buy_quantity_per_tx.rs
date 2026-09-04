//! Tests for the max buy quantity per transaction feature (issue #828).

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_pricing_and_fees};
use creator_keys::{events, ContractError, MAX_BUY_QUANTITY_LIMIT};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 100;

fn env_with_auths() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

#[test]
fn test_set_max_buy_quantity_by_creator() {
    let env = env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");

    let result = client.try_set_max_buy_quantity(&creator, &5);
    assert_eq!(result, Ok(Ok(())));

    let max_qty = client.get_max_buy_quantity(&creator);
    assert_eq!(max_qty, Some(5));
}

#[test]
fn test_set_max_buy_quantity_too_high() {
    let env = env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");

    let result = client.try_set_max_buy_quantity(&creator, &(MAX_BUY_QUANTITY_LIMIT + 1));
    assert_eq!(result, Err(Ok(ContractError::LimitTooHigh)));
}

#[test]
fn test_set_max_buy_quantity_at_limit() {
    let env = env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");

    let result = client.try_set_max_buy_quantity(&creator, &MAX_BUY_QUANTITY_LIMIT);
    assert_eq!(result, Ok(Ok(())));
}

#[test]
fn test_buy_keys_within_limit() {
    let env = env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    client.set_max_buy_quantity(&creator, &3);

    let buy_quote = client.get_buy_quote(&creator);
    let total_payment = buy_quote.total_amount * 2;
    let result = client.try_buy_keys(&creator, &buyer, &total_payment, &None, &2, &None);
    assert_eq!(result, Ok(Ok(2))); // supply goes from 0 to 2

    assert_eq!(client.get_key_balance(&creator, &buyer), 2);
}

#[test]
fn test_buy_keys_exceeds_limit() {
    let env = env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    client.set_max_buy_quantity(&creator, &2);

    let buy_quote = client.get_buy_quote(&creator);
    let total_payment = buy_quote.total_amount * 3;
    let result = client.try_buy_keys(&creator, &buyer, &total_payment, &None, &3, &None);
    assert_eq!(result, Err(Ok(ContractError::QuantityExceedsLimit)));
}

#[test]
fn test_buy_keys_at_exact_limit() {
    let env = env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    client.set_max_buy_quantity(&creator, &2);

    let buy_quote = client.get_buy_quote(&creator);
    let total_payment = buy_quote.total_amount * 2;
    let result = client.try_buy_keys(&creator, &buyer, &total_payment, &None, &2, &None);
    assert_eq!(result, Ok(Ok(2)));

    assert_eq!(client.get_key_balance(&creator, &buyer), 2);
}

#[test]
fn test_buy_keys_no_limit_set() {
    let env = env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let buy_quote = client.get_buy_quote(&creator);
    let total_payment = buy_quote.total_amount * 5;
    let result = client.try_buy_keys(&creator, &buyer, &total_payment, &None, &5, &None);
    assert_eq!(result, Ok(Ok(5)));

    assert_eq!(client.get_key_balance(&creator, &buyer), 5);
}

#[test]
fn test_buy_keys_zero_quantity_fails() {
    let env = env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let result = client.try_buy_keys(&creator, &buyer, &100, &None, &0, &None);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn test_buy_keys_emits_event_on_set() {
    let env = env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");

    env.events().all();

    client.set_max_buy_quantity(&creator, &10);

    let event_log = env.events().all();
    let has_event = event_log.iter().any(|(_, topics, _)| {
        topics
            .get(0)
            .map(|v| {
                let name: Symbol = v.into_val(&env);
                name == events::MAX_BUY_QUANTITY_UPDATED_EVENT_NAME
            })
            .unwrap_or(false)
    });
    assert!(has_event, "max_buy_quantity_updated event should be emitted");
}

#[test]
fn test_get_max_buy_quantity_returns_none_when_unset() {
    let env = env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");

    let max_qty = client.get_max_buy_quantity(&creator);
    assert_eq!(max_qty, None);
}
