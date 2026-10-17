//! Integration tests for on-chain price alerts (issue #1030).
//!
//! Wallets register a target price on a key with an `Above` or `Below`
//! direction. Every buy and sell compares the key's spot price before and
//! after the trade; an alert whose target the price crossed emits
//! `PriceAlertTriggered` and is deactivated so it never fires again.
//!
//! The fixture uses a linear curve with `KEY_PRICE = 100` and `SLOPE = 10`, so
//! the spot price at supply `s` is `100 + 10 * s`.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_ledger_sequence, set_protocol_fee_bps,
    test_env_with_auths, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use creator_keys::{
    events,
    price_alerts::{
        AlertDirection, PriceAlert, PriceAlertDataKey, MAX_ACTIVE_ALERTS_PER_KEY,
        MAX_ACTIVE_ALERTS_PER_WALLET,
    },
    ContractError, CreatorKeysContractClient,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    vec, Address, Env, IntoVal, Symbol, Val, Vec,
};

const KEY_PRICE: i128 = 100;
const SLOPE: i128 = 10;
/// Comfortably covers any single key in these tests.
const PAYMENT: i128 = 1_000_000;

struct Setup<'a> {
    env: &'a Env,
    client: CreatorKeysContractClient<'a>,
    contract_id: Address,
    key: Address,
}

fn setup(env: &Env) -> Setup<'_> {
    let (client, contract_id) = register_creator_keys(env);
    let admin = Address::generate(env);
    client.set_key_price(&admin, &KEY_PRICE);
    client.set_curve_slope(&admin, &SLOPE);
    let key = register_test_creator(env, &client, "alice");
    Setup {
        env,
        client,
        contract_id,
        key,
    }
}

fn spot_price(supply: u32) -> i128 {
    KEY_PRICE + SLOPE * i128::from(supply)
}

/// Moves to the next ledger so a holder may sell what it bought (flash-loan guard).
fn next_ledger(env: &Env) {
    set_ledger_sequence(env, env.ledger().sequence() + 1);
}

/// `PriceAlertTriggered` events from the most recent contract call, with their topics.
fn triggered_events(env: &Env) -> std::vec::Vec<(Vec<Val>, events::PriceAlertTriggeredEvent)> {
    env.events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(env);
                    name == events::PRICE_ALERT_TRIGGERED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .map(|(_, topics, data)| (topics, data.into_val(env)))
        .collect()
}

fn key_alerts_entry(s: &Setup) -> Option<Vec<PriceAlert>> {
    s.env.as_contract(&s.contract_id, || {
        s.env
            .storage()
            .persistent()
            .get(&PriceAlertDataKey::KeyAlerts(s.key.clone()))
    })
}

#[test]
fn register_alert_stores_direction_and_target_price() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    set_ledger_sequence(s.env, 42);

    let above_id = s
        .client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &150);
    let below_id = s
        .client
        .register_alert(&s.key, &wallet, &AlertDirection::Below, &80);
    assert_eq!((above_id, below_id), (1, 2), "ids are issued sequentially");

    let alerts = s.client.get_alerts(&wallet);
    assert_eq!(
        alerts,
        vec![
            s.env,
            PriceAlert {
                alert_id: 1,
                wallet: wallet.clone(),
                key_id: s.key.clone(),
                direction: AlertDirection::Above,
                target_price: 150,
                created_ledger: 42,
            },
            PriceAlert {
                alert_id: 2,
                wallet: wallet.clone(),
                key_id: s.key.clone(),
                direction: AlertDirection::Below,
                target_price: 80,
                created_ledger: 42,
            },
        ]
    );
    assert_eq!(key_alerts_entry(&s), Some(alerts));
}

#[test]
fn register_alert_emits_registered_event() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);

    let alert_id = s
        .client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &150);

    let (_, topics, data) = s.env.events().all().last().unwrap();
    let expected_topics: Vec<Val> = (
        events::PRICE_ALERT_REGISTERED_EVENT_NAME,
        s.key.clone(),
        wallet.clone(),
    )
        .into_val(s.env);
    assert_eq!(topics, expected_topics);
    let event: events::PriceAlertRegisteredEvent = data.into_val(s.env);
    assert_eq!(
        event,
        events::PriceAlertRegisteredEvent {
            wallet,
            key_id: s.key.clone(),
            alert_id,
            direction: AlertDirection::Above,
            target_price: 150,
            ledger: s.env.ledger().sequence(),
        }
    );
}

#[test]
fn buy_pushing_price_above_target_emits_price_alert_triggered() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let buyer = Address::generate(s.env);
    let alert_id = s
        .client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(2));

    // 100 -> 110: below the target, nothing fires.
    s.client.buy_key(&s.key, &buyer, &PAYMENT, &None);
    assert!(triggered_events(s.env).is_empty());

    // 110 -> 120: reaches the target.
    s.client.buy_key(&s.key, &buyer, &PAYMENT, &None);
    let fired = triggered_events(s.env);
    assert_eq!(fired.len(), 1);
    let (topics, event) = &fired[0];
    let expected_topics: Vec<Val> = (
        events::PRICE_ALERT_TRIGGERED_EVENT_NAME,
        s.key.clone(),
        wallet.clone(),
    )
        .into_val(s.env);
    assert_eq!(topics, &expected_topics);
    assert_eq!(
        event,
        &events::PriceAlertTriggeredEvent {
            wallet: wallet.clone(),
            key_id: s.key.clone(),
            alert_id,
            direction: AlertDirection::Above,
            target_price: spot_price(2),
            new_price: spot_price(2),
            ledger: s.env.ledger().sequence(),
        }
    );
}

#[test]
fn sell_pushing_price_below_target_emits_price_alert_triggered() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let holder = Address::generate(s.env);
    s.client.buy_keys(&s.key, &holder, &5, &PAYMENT, &None);
    next_ledger(s.env);
    assert_eq!(s.client.get_total_key_supply(&s.key), 5);

    let alert_id = s
        .client
        .register_alert(&s.key, &wallet, &AlertDirection::Below, &spot_price(3));

    // 150 -> 140: still above the target.
    s.client.sell_key(&s.key, &holder, &None);
    assert!(triggered_events(s.env).is_empty());

    // 140 -> 130: reaches the target.
    s.client.sell_key(&s.key, &holder, &None);
    let fired = triggered_events(s.env);
    assert_eq!(fired.len(), 1);
    assert_eq!(
        fired[0].1,
        events::PriceAlertTriggeredEvent {
            wallet,
            key_id: s.key.clone(),
            alert_id,
            direction: AlertDirection::Below,
            target_price: spot_price(3),
            new_price: spot_price(3),
            ledger: s.env.ledger().sequence(),
        }
    );
}

#[test]
fn alert_is_deactivated_after_first_trigger() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let buyer = Address::generate(s.env);
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(1));

    s.client.buy_key(&s.key, &buyer, &PAYMENT, &None);
    assert_eq!(triggered_events(s.env).len(), 1);

    assert!(s.client.get_alerts(&wallet).is_empty());
    assert_eq!(
        key_alerts_entry(&s),
        None,
        "the key's alert entry is removed once its last alert fires"
    );

    // Further price rises do not fire it again.
    s.client.buy_key(&s.key, &buyer, &PAYMENT, &None);
    assert!(triggered_events(s.env).is_empty());
}

#[test]
fn get_alerts_returns_only_active_alerts() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let buyer = Address::generate(s.env);
    let fires = s
        .client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(1));
    let stays_above =
        s.client
            .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(9));
    let stays_below = s
        .client
        .register_alert(&s.key, &wallet, &AlertDirection::Below, &50);
    assert_eq!(s.client.get_alerts(&wallet).len(), 3);

    s.client.buy_key(&s.key, &buyer, &PAYMENT, &None);
    let fired = triggered_events(s.env);
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].1.alert_id, fires);

    let active: std::vec::Vec<u64> = s
        .client
        .get_alerts(&wallet)
        .iter()
        .map(|a| a.alert_id)
        .collect();
    assert_eq!(active, [stays_above, stays_below]);
}

#[test]
fn alert_does_not_retrigger_when_price_crosses_back_and_again() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let trader = Address::generate(s.env);
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(2));

    s.client.buy_keys(&s.key, &trader, &2, &PAYMENT, &None);
    assert_eq!(triggered_events(s.env).len(), 1);
    next_ledger(s.env);

    // Back below the target, then across it again.
    s.client.sell_key(&s.key, &trader, &None);
    assert!(triggered_events(s.env).is_empty());
    s.client.buy_key(&s.key, &trader, &PAYMENT, &None);
    assert!(triggered_events(s.env).is_empty());
    next_ledger(s.env);
    s.client.sell_key(&s.key, &trader, &None);
    s.client.buy_key(&s.key, &trader, &PAYMENT, &None);
    assert!(triggered_events(s.env).is_empty());
}

#[test]
fn below_alert_does_not_retrigger_when_price_crosses_back_and_again() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let trader = Address::generate(s.env);
    s.client.buy_keys(&s.key, &trader, &3, &PAYMENT, &None);
    next_ledger(s.env);
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Below, &spot_price(2));

    s.client.sell_key(&s.key, &trader, &None);
    assert_eq!(triggered_events(s.env).len(), 1);

    s.client.buy_key(&s.key, &trader, &PAYMENT, &None);
    next_ledger(s.env);
    s.client.sell_key(&s.key, &trader, &None);
    assert!(triggered_events(s.env).is_empty());
}

#[test]
fn price_moving_without_crossing_target_does_not_trigger() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let trader = Address::generate(s.env);
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(10));
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Below, &50);

    s.client.buy_keys(&s.key, &trader, &3, &PAYMENT, &None);
    assert!(triggered_events(s.env).is_empty());
    next_ledger(s.env);
    s.client.sell_key(&s.key, &trader, &None);
    assert!(triggered_events(s.env).is_empty());

    assert_eq!(s.client.get_alerts(&wallet).len(), 2);
}

#[test]
fn alert_on_far_side_of_target_waits_for_a_real_crossing() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let trader = Address::generate(s.env);
    s.client.buy_keys(&s.key, &trader, &2, &PAYMENT, &None);
    next_ledger(s.env);

    // Spot price is 120: already above 110, and exactly at 120.
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(1));
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(2));

    // Rising further from on/above the target is not a crossing.
    s.client.buy_key(&s.key, &trader, &PAYMENT, &None);
    assert!(triggered_events(s.env).is_empty());
    assert_eq!(s.client.get_alerts(&wallet).len(), 2);

    // Drop below both targets (130 -> 100), then rise back across them.
    next_ledger(s.env);
    s.client.sell_key(&s.key, &trader, &None);
    s.client.sell_key(&s.key, &trader, &None);
    s.client.sell_key(&s.key, &trader, &None);
    assert!(triggered_events(s.env).is_empty());
    s.client.buy_keys(&s.key, &trader, &2, &PAYMENT, &None);
    assert_eq!(triggered_events(s.env).len(), 2);
}

#[test]
fn multi_key_buy_crossing_several_targets_fires_each_once() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let buyer = Address::generate(s.env);
    for supply in 1..=3 {
        s.client
            .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(supply));
    }

    // 100 -> 130 in one call.
    s.client.buy_keys(&s.key, &buyer, &3, &PAYMENT, &None);
    let fired = triggered_events(s.env);
    let ids: std::vec::Vec<u64> = fired.iter().map(|(_, e)| e.alert_id).collect();
    assert_eq!(ids, [1, 2, 3]);
    assert!(fired.iter().all(|(_, e)| e.new_price == spot_price(3)));
    assert!(s.client.get_alerts(&wallet).is_empty());
}

#[test]
fn wallets_and_keys_are_isolated() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let other_key = register_test_creator(s.env, &s.client, "bob");
    let wallet_a = Address::generate(s.env);
    let wallet_b = Address::generate(s.env);
    let buyer = Address::generate(s.env);

    let a_fires =
        s.client
            .register_alert(&s.key, &wallet_a, &AlertDirection::Above, &spot_price(1));
    let b_fires =
        s.client
            .register_alert(&s.key, &wallet_b, &AlertDirection::Above, &spot_price(1));
    let b_waits =
        s.client
            .register_alert(&s.key, &wallet_b, &AlertDirection::Above, &spot_price(5));
    let a_other_key = s.client.register_alert(
        &other_key,
        &wallet_a,
        &AlertDirection::Above,
        &spot_price(1),
    );

    s.client.buy_key(&s.key, &buyer, &PAYMENT, &None);
    let fired = triggered_events(s.env);
    let fired: std::vec::Vec<(Address, u64)> = fired
        .iter()
        .map(|(_, e)| (e.wallet.clone(), e.alert_id))
        .collect();
    assert_eq!(
        fired,
        [(wallet_a.clone(), a_fires), (wallet_b.clone(), b_fires)]
    );

    let a_active: std::vec::Vec<u64> = s
        .client
        .get_alerts(&wallet_a)
        .iter()
        .map(|a| a.alert_id)
        .collect();
    assert_eq!(
        a_active,
        [a_other_key],
        "the other key's alert is untouched"
    );
    let b_active: std::vec::Vec<u64> = s
        .client
        .get_alerts(&wallet_b)
        .iter()
        .map(|a| a.alert_id)
        .collect();
    assert_eq!(b_active, [b_waits]);

    // A trade on the other key fires only that key's alert.
    s.client.buy_key(&other_key, &buyer, &PAYMENT, &None);
    let fired = triggered_events(s.env);
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].1.alert_id, a_other_key);
    assert_eq!(fired[0].1.key_id, other_key);
    assert!(s.client.get_alerts(&wallet_a).is_empty());
    assert_eq!(s.client.get_alerts(&wallet_b).len(), 1);
}

#[test]
fn register_alert_rejects_invalid_inputs() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let unregistered = Address::generate(s.env);

    assert_eq!(
        s.client
            .try_register_alert(&unregistered, &wallet, &AlertDirection::Above, &150),
        Err(Ok(ContractError::NotRegistered))
    );
    assert_eq!(
        s.client
            .try_register_alert(&s.key, &wallet, &AlertDirection::Above, &0),
        Err(Ok(ContractError::NotPositiveAmount))
    );
    assert_eq!(
        s.client
            .try_register_alert(&s.key, &wallet, &AlertDirection::Below, &-5),
        Err(Ok(ContractError::NotPositiveAmount))
    );
    assert!(s.client.get_alerts(&wallet).is_empty());
}

#[test]
fn register_alert_requires_wallet_auth() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);

    s.env.mock_auths(&[]);
    let result = s
        .client
        .try_register_alert(&s.key, &wallet, &AlertDirection::Above, &150);
    assert!(matches!(result, Err(Err(_))), "unauthorized call must fail");

    s.env.mock_all_auths();
    assert!(s.client.get_alerts(&wallet).is_empty());
}

#[test]
fn register_alert_records_wallet_as_required_signer() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);

    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &150);

    let auths = s.env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, wallet);
}

#[test]
fn wallet_alert_cap_is_enforced_and_freed_by_triggers() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let buyer = Address::generate(s.env);
    let other_key = register_test_creator(s.env, &s.client, "bob");

    // One alert that will fire, the rest spread over another key.
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(1));
    for _ in 1..MAX_ACTIVE_ALERTS_PER_WALLET {
        s.client
            .register_alert(&other_key, &wallet, &AlertDirection::Above, &10_000);
    }
    assert_eq!(
        s.client
            .try_register_alert(&s.key, &wallet, &AlertDirection::Above, &10_000),
        Err(Ok(ContractError::WalletAlertLimitReached))
    );

    s.client.buy_key(&s.key, &buyer, &PAYMENT, &None);
    assert_eq!(triggered_events(s.env).len(), 1);

    // The fired alert no longer counts against the cap.
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &10_000);
    assert_eq!(
        s.client.get_alerts(&wallet).len(),
        MAX_ACTIVE_ALERTS_PER_WALLET
    );
}

#[test]
fn key_alert_cap_is_enforced() {
    let env = test_env_with_auths();
    let s = setup(&env);
    for _ in 0..MAX_ACTIVE_ALERTS_PER_KEY {
        let wallet = Address::generate(s.env);
        s.client
            .register_alert(&s.key, &wallet, &AlertDirection::Above, &10_000);
    }
    let late = Address::generate(s.env);
    assert_eq!(
        s.client
            .try_register_alert(&s.key, &late, &AlertDirection::Above, &10_000),
        Err(Ok(ContractError::KeyAlertLimitReached))
    );
}

#[test]
fn full_key_of_alerts_fires_in_a_single_buy() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let buyer = Address::generate(s.env);
    let mut wallets = std::vec::Vec::new();
    for _ in 0..MAX_ACTIVE_ALERTS_PER_KEY {
        let wallet = Address::generate(s.env);
        s.client
            .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(1));
        wallets.push(wallet);
    }

    // Worst case for one trade: every alert on the key fires at once.
    s.client.buy_key(&s.key, &buyer, &PAYMENT, &None);
    assert_eq!(
        triggered_events(s.env).len(),
        MAX_ACTIVE_ALERTS_PER_KEY as usize
    );
    assert_eq!(key_alerts_entry(&s), None);
    for wallet in &wallets {
        assert!(s.client.get_alerts(wallet).is_empty());
    }
}

#[test]
fn cancel_alert_deactivates_it_and_frees_the_slot() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let buyer = Address::generate(s.env);
    let cancelled =
        s.client
            .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(1));
    let kept = s
        .client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(9));

    s.client.cancel_alert(&wallet, &cancelled);
    let (_, _, data) = s.env.events().all().last().unwrap();
    let event: events::PriceAlertCancelledEvent = data.into_val(s.env);
    assert_eq!(event.alert_id, cancelled);
    assert_eq!(event.key_id, s.key);

    let active: std::vec::Vec<u64> = s
        .client
        .get_alerts(&wallet)
        .iter()
        .map(|a| a.alert_id)
        .collect();
    assert_eq!(active, [kept]);

    // The cancelled alert's target is crossed, but it no longer exists.
    s.client.buy_key(&s.key, &buyer, &PAYMENT, &None);
    assert!(triggered_events(s.env).is_empty());
}

#[test]
fn cancel_alert_rejects_unknown_foreign_and_fired_alerts() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let stranger = Address::generate(s.env);
    let buyer = Address::generate(s.env);
    let fired = s
        .client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(1));
    let active = s
        .client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(9));
    s.client.buy_key(&s.key, &buyer, &PAYMENT, &None);

    assert_eq!(
        s.client.try_cancel_alert(&wallet, &999),
        Err(Ok(ContractError::AlertNotFound))
    );
    assert_eq!(
        s.client.try_cancel_alert(&stranger, &active),
        Err(Ok(ContractError::AlertNotFound)),
        "a wallet cannot cancel another wallet's alert"
    );
    assert_eq!(
        s.client.try_cancel_alert(&wallet, &fired),
        Err(Ok(ContractError::AlertNotFound))
    );
    assert_eq!(s.client.get_alerts(&wallet).len(), 1);
}

#[test]
fn cancel_alert_requires_wallet_auth() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let wallet = Address::generate(s.env);
    let alert_id = s
        .client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &150);

    s.env.mock_auths(&[]);
    assert!(matches!(
        s.client.try_cancel_alert(&wallet, &alert_id),
        Err(Err(_))
    ));

    s.env.mock_all_auths();
    assert_eq!(s.client.get_alerts(&wallet).len(), 1);
}

#[test]
fn batch_buy_and_batch_sell_check_alerts_per_order() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let other_key = register_test_creator(s.env, &s.client, "bob");
    let wallet = Address::generate(s.env);
    let trader = Address::generate(s.env);
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(2));
    s.client
        .register_alert(&other_key, &wallet, &AlertDirection::Above, &spot_price(1));

    s.client.batch_buy(
        &trader,
        &vec![s.env, (s.key.clone(), 2u32), (other_key.clone(), 1u32)],
    );
    let fired: std::vec::Vec<Address> = triggered_events(s.env)
        .iter()
        .map(|(_, e)| e.key_id.clone())
        .collect();
    assert_eq!(fired, [s.key.clone(), other_key.clone()]);

    next_ledger(s.env);
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Below, &spot_price(1));
    s.client
        .batch_sell(&trader, &vec![s.env, (s.key.clone(), 1u32)]);
    let fired = triggered_events(s.env);
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].1.direction, AlertDirection::Below);
    assert_eq!(fired[0].1.new_price, spot_price(1));
}

#[test]
fn batch_v2_paths_check_alerts() {
    let env = test_env_with_auths();
    let s = setup(&env);
    // `batch_sell_v2` requires a protocol fee config.
    set_protocol_fee_bps(s.env, &s.client, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS);
    let wallet = Address::generate(s.env);
    let trader = Address::generate(s.env);
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Above, &spot_price(2));

    s.client
        .batch_buy_v2(&trader, &vec![s.env, (s.key.clone(), 2u32, None)]);
    assert_eq!(triggered_events(s.env).len(), 1);

    next_ledger(s.env);
    s.client
        .register_alert(&s.key, &wallet, &AlertDirection::Below, &spot_price(1));
    s.client
        .batch_sell_v2(&trader, &vec![s.env, (s.key.clone(), 1u32, None)]);
    assert_eq!(triggered_events(s.env).len(), 1);
}
