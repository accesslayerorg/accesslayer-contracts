//! Tests for InsurancePurchased and InsurancePayoutTriggered event emission.

mod contract_test_env;

use contract_test_env::{
    buy_default_policy, setup_with_price, test_env_with_auths, DEFAULT_COVERAGE,
    DEFAULT_KEY_PRICE, DEFAULT_THRESHOLD_BPS,
};
use key_insurance::events::{
    InsurancePayoutTriggeredEvent, InsurancePurchasedEvent, INSURANCE_PURCHASED_NAME,
    INSURANCE_PAYOUT_NAME,
};
use soroban_sdk::{testutils::{Address as _, Events as _}, IntoVal};

#[test]
fn test_insurance_purchased_event_emitted() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = soroban_sdk::Address::generate(&env);

    client.purchase_insurance(&wallet, &key_id, &DEFAULT_COVERAGE, &DEFAULT_THRESHOLD_BPS);

    let events = env.events().all();
    let matching: soroban_sdk::Vec<_> = events
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(0)
                .map(|t| t == INSURANCE_PURCHASED_NAME.into_val(&env))
                .unwrap_or(false)
        })
        .collect();

    assert!(!matching.is_empty(), "InsurancePurchased event must be emitted");
}

#[test]
fn test_insurance_purchased_event_data_is_correct() {
    let env = test_env_with_auths();
    let (client, _, _admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = soroban_sdk::Address::generate(&env);

    client.purchase_insurance(&wallet, &key_id, &DEFAULT_COVERAGE, &DEFAULT_THRESHOLD_BPS);

    let events = env.events().all();
    let (_, _, data) = events.last().expect("at least one event");
    let payload: InsurancePurchasedEvent = data.into_val(&env);

    assert_eq!(payload.wallet, wallet);
    assert_eq!(payload.key_id, key_id);
    assert_eq!(payload.coverage_amount, DEFAULT_COVERAGE);
    assert_eq!(payload.threshold_bps, DEFAULT_THRESHOLD_BPS);
    assert!(payload.premium > 0);
}

#[test]
fn test_insurance_payout_event_emitted() {
    let env = test_env_with_auths();
    let (client, _, admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    client.set_key_price(&admin, &key_id, &100i128);
    client.trigger_payout(&key_id, &wallet);

    let events = env.events().all();
    let matching: soroban_sdk::Vec<_> = events
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(0)
                .map(|t| t == INSURANCE_PAYOUT_NAME.into_val(&env))
                .unwrap_or(false)
        })
        .collect();

    assert!(!matching.is_empty(), "InsurancePayoutTriggered event must be emitted");
}

#[test]
fn test_insurance_payout_event_data_is_correct() {
    let env = test_env_with_auths();
    let (client, _, admin, key_id) = setup_with_price(&env, DEFAULT_KEY_PRICE);
    let wallet = buy_default_policy(&env, &client, &key_id);

    client.set_key_price(&admin, &key_id, &100i128);
    client.trigger_payout(&key_id, &wallet);

    let events = env.events().all();
    let (_, _, data) = events.last().expect("at least one event");
    let payload: InsurancePayoutTriggeredEvent = data.into_val(&env);

    assert_eq!(payload.wallet, wallet);
    assert_eq!(payload.key_id, key_id);
    assert_eq!(payload.payout_amount, DEFAULT_COVERAGE);
}
