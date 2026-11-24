//! Subscription tier management and on-chain access gating (#1179).

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, set_test_timestamp,
    test_env_with_auths,
};
use creator_keys::{CreatorKeysContractClient, SubscriptionTierError};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

const KEY_PRICE: i128 = 100;
const FEE: i128 = 500;
const PERIOD: u64 = 1_000;
const TIER_ID: u32 = 1;
const START: u64 = 1_700_000_000;

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, KEY_PRICE, 9_000, 1_000);
    let creator = register_test_creator(env, &client, "tiered");
    set_test_timestamp(env, START);
    client.create_tier(
        &creator,
        &TIER_ID,
        &FEE,
        &PERIOD,
        &String::from_str(env, "Inner circle"),
    );
    (client, creator)
}

#[test]
fn tier_creation_stores_fee_period_and_label() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    let tier = client.get_tier(&creator, &TIER_ID).expect("tier stored");
    assert_eq!(tier.key_id, creator);
    assert_eq!(tier.tier_id, TIER_ID);
    assert_eq!(tier.fee_per_period, FEE);
    assert_eq!(tier.period_seconds, PERIOD);
    assert_eq!(tier.label, String::from_str(&env, "Inner circle"));
}

#[test]
fn creating_the_same_tier_twice_is_rejected() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    let result = client.try_create_tier(
        &creator,
        &TIER_ID,
        &FEE,
        &PERIOD,
        &String::from_str(&env, "duplicate"),
    );
    assert_eq!(
        result.err().unwrap().unwrap(),
        SubscriptionTierError::TierAlreadyExists
    );
}

#[test]
fn subscribe_records_the_expiry_and_credits_creator_earnings() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let wallet = Address::generate(&env);

    let expiry = client.subscribe(&creator, &wallet, &TIER_ID);
    assert_eq!(expiry, START + PERIOD);
    assert!(client.is_subscribed(&creator, &wallet, &TIER_ID));

    let record = client
        .get_tier_subscription(&creator, &wallet, &TIER_ID)
        .expect("subscription stored");
    assert_eq!(record.expires_at, expiry);
    assert_eq!(record.period_seconds, PERIOD);

    assert_eq!(client.get_creator_fee_balance(&creator), FEE);
}

#[test]
fn access_ends_when_the_period_expires() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let wallet = Address::generate(&env);

    client.subscribe(&creator, &wallet, &TIER_ID);

    set_test_timestamp(&env, START + PERIOD - 1);
    assert!(client.is_subscribed(&creator, &wallet, &TIER_ID));

    set_test_timestamp(&env, START + PERIOD);
    assert!(!client.is_subscribed(&creator, &wallet, &TIER_ID));
}

#[test]
fn renewal_extends_from_the_current_expiry_not_from_now() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let wallet = Address::generate(&env);

    let first = client.subscribe(&creator, &wallet, &TIER_ID);
    assert_eq!(first, START + PERIOD);

    // Renew half-way through the paid period: the remaining time is preserved.
    set_test_timestamp(&env, START + PERIOD / 2);
    let renewed = client.subscribe(&creator, &wallet, &TIER_ID);

    assert_eq!(renewed, first + PERIOD);
    assert_eq!(client.get_creator_fee_balance(&creator), FEE * 2);
}

#[test]
fn re_subscribing_after_lapse_starts_from_now() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let wallet = Address::generate(&env);

    client.subscribe(&creator, &wallet, &TIER_ID);

    let after_lapse = START + PERIOD * 2;
    set_test_timestamp(&env, after_lapse);
    assert!(!client.is_subscribed(&creator, &wallet, &TIER_ID));

    let expiry = client.subscribe(&creator, &wallet, &TIER_ID);
    assert_eq!(expiry, after_lapse + PERIOD);
}

#[test]
fn subscriptions_are_scoped_per_wallet_and_per_tier() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let wallet = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.create_tier(
        &creator,
        &2u32,
        &(FEE * 2),
        &PERIOD,
        &String::from_str(&env, "VIP"),
    );
    client.subscribe(&creator, &wallet, &TIER_ID);

    assert!(client.is_subscribed(&creator, &wallet, &TIER_ID));
    assert!(!client.is_subscribed(&creator, &stranger, &TIER_ID));
    assert!(!client.is_subscribed(&creator, &wallet, &2u32));
}

#[test]
fn subscribing_to_an_unknown_tier_is_rejected() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let wallet = Address::generate(&env);

    let result = client.try_subscribe(&creator, &wallet, &99u32);
    assert_eq!(
        result.err().unwrap().unwrap(),
        SubscriptionTierError::TierNotFound
    );
}

#[test]
fn tiers_cannot_be_created_for_an_unregistered_key() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let unregistered = Address::generate(&env);

    let result = client.try_create_tier(
        &unregistered,
        &TIER_ID,
        &FEE,
        &PERIOD,
        &String::from_str(&env, "nobody"),
    );
    assert_eq!(
        result.err().unwrap().unwrap(),
        SubscriptionTierError::NotRegistered
    );
}

#[test]
fn zero_period_and_negative_fee_are_rejected() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    let zero_period = client.try_create_tier(
        &creator,
        &2u32,
        &FEE,
        &0u64,
        &String::from_str(&env, "bad period"),
    );
    assert_eq!(
        zero_period.err().unwrap().unwrap(),
        SubscriptionTierError::InvalidPeriod
    );

    let negative_fee = client.try_create_tier(
        &creator,
        &3u32,
        &-1i128,
        &PERIOD,
        &String::from_str(&env, "bad fee"),
    );
    assert_eq!(
        negative_fee.err().unwrap().unwrap(),
        SubscriptionTierError::InvalidFee
    );
}
