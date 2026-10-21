//! Subscription access gating (Issue #953).
//!
//! The gate's value is that the minimum-hold check reads the ledger rather than
//! trusting the caller, so these tests drive it through the contract entry
//! points — buying and selling real keys to move a balance — rather than calling
//! the internal helper with a hand-supplied balance.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use creator_keys::CreatorKeysContractClient;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

const MIN_HOLD: u32 = 3;
const DURATION: u32 = 1_000;
const KEY_PRICE: i128 = 100;

/// Contract with pricing configured, an admin, and one registered creator.
fn setup() -> (Env, CreatorKeysContractClient<'static>, Address, Address) {
    let env = test_env_with_auths();
    // Leaked so the client can outlive this frame; tests are short-lived and the
    // alternative is threading a lifetime through every helper.
    let env: &'static Env = Box::leak(Box::new(env));
    let (client, _) = register_creator_keys(env);

    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &KEY_PRICE);

    let creator = register_test_creator(env, &client, "alice");
    (env.clone(), client, admin, creator)
}

fn buy(client: &CreatorKeysContractClient<'_>, creator: &Address, wallet: &Address, count: u32) {
    for _ in 0..count {
        client.buy_key(creator, wallet, &KEY_PRICE, &None);
    }
}

/// Sells `count` keys, advancing the ledger first so any anti-flash-trade
/// lockup window has elapsed.
fn sell(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    wallet: &Address,
    count: u32,
) {
    env.ledger().with_mut(|l| {
        l.timestamp += 86_400;
        l.sequence_number += 100;
    });
    for _ in 0..count {
        client.sell_key(creator, wallet, &None);
    }
}

#[test]
fn subscribe_rejects_a_wallet_below_the_minimum() {
    let (env, client, admin, creator) = setup();
    let wallet = Address::generate(&env);

    client.set_min_hold_for_access(&admin, &creator, &MIN_HOLD);

    // Holds nothing at all.
    assert!(client.try_subscribe(&creator, &wallet, &DURATION).is_err());
    assert!(!client.is_subscribed(&creator, &wallet));
}

#[test]
fn subscribe_rejects_a_wallet_one_key_short() {
    let (env, client, admin, creator) = setup();
    let wallet = Address::generate(&env);

    client.set_min_hold_for_access(&admin, &creator, &MIN_HOLD);
    buy(&client, &creator, &wallet, MIN_HOLD - 1);

    assert!(client.try_subscribe(&creator, &wallet, &DURATION).is_err());
}

#[test]
fn subscribe_accepts_a_qualifying_hold_and_returns_the_expiry() {
    let (env, client, admin, creator) = setup();
    let wallet = Address::generate(&env);

    client.set_min_hold_for_access(&admin, &creator, &MIN_HOLD);
    buy(&client, &creator, &wallet, MIN_HOLD);

    let before = env.ledger().sequence();
    let expiry = client.subscribe(&creator, &wallet, &DURATION);

    assert_eq!(expiry, before + DURATION);
    assert!(client.is_subscribed(&creator, &wallet));
    assert_eq!(
        client
            .get_subscription(&creator, &wallet)
            .unwrap()
            .expires_at_ledger,
        expiry
    );
}

#[test]
fn access_is_denied_once_the_expiry_ledger_passes() {
    let (env, client, admin, creator) = setup();
    let wallet = Address::generate(&env);

    client.set_min_hold_for_access(&admin, &creator, &MIN_HOLD);
    buy(&client, &creator, &wallet, MIN_HOLD);
    client.subscribe(&creator, &wallet, &DURATION);

    // The comparison is `<=`, so access survives *at* the expiry ledger.
    env.ledger().with_mut(|l| l.sequence_number += DURATION);
    assert!(client.is_subscribed(&creator, &wallet));

    env.ledger().with_mut(|l| l.sequence_number += 1);
    assert!(!client.is_subscribed(&creator, &wallet));
}

#[test]
fn access_lapses_when_the_holding_drops_below_the_minimum() {
    // "Revoked automatically" — the gate is evaluated on read, so there is no
    // window in which a sold-down wallet still passes.
    let (env, client, admin, creator) = setup();
    let wallet = Address::generate(&env);

    client.set_min_hold_for_access(&admin, &creator, &MIN_HOLD);
    buy(&client, &creator, &wallet, MIN_HOLD);
    client.subscribe(&creator, &wallet, &DURATION);
    assert!(client.is_subscribed(&creator, &wallet));

    sell(&env, &client, &creator, &wallet, 1);

    assert!(
        !client.is_subscribed(&creator, &wallet),
        "selling below the minimum must revoke access without a transaction"
    );
}

#[test]
fn access_returns_when_the_holding_is_topped_back_up() {
    // The record is not destroyed by dropping below the minimum, so re-acquiring
    // keys restores access for the remainder of the term.
    let (env, client, admin, creator) = setup();
    let wallet = Address::generate(&env);

    client.set_min_hold_for_access(&admin, &creator, &MIN_HOLD);
    buy(&client, &creator, &wallet, MIN_HOLD);
    client.subscribe(&creator, &wallet, &DURATION);

    sell(&env, &client, &creator, &wallet, 1);
    assert!(!client.is_subscribed(&creator, &wallet));

    buy(&client, &creator, &wallet, 1);
    assert!(client.is_subscribed(&creator, &wallet));
}

#[test]
fn subscribe_rejects_when_gating_is_not_configured() {
    // Reported distinctly from an insufficient balance: an operator needs to tell
    // "gating is off" from "you need more keys".
    let (env, client, _admin, creator) = setup();
    let wallet = Address::generate(&env);

    buy(&client, &creator, &wallet, 10);

    assert!(client.try_subscribe(&creator, &wallet, &DURATION).is_err());
    assert!(client.get_min_hold_for_access(&creator).is_none());
}

#[test]
fn subscribe_rejects_a_zero_duration() {
    // A subscription expiring on the ledger it was created in is never usable.
    let (env, client, admin, creator) = setup();
    let wallet = Address::generate(&env);

    client.set_min_hold_for_access(&admin, &creator, &MIN_HOLD);
    buy(&client, &creator, &wallet, MIN_HOLD);

    assert!(client.try_subscribe(&creator, &wallet, &0).is_err());
}

#[test]
fn a_zero_minimum_is_rejected() {
    // Zero would gate nothing while looking configured; removing the key is the
    // honest way to disable gating.
    let (_env, client, admin, creator) = setup();

    assert!(client
        .try_set_min_hold_for_access(&admin, &creator, &0)
        .is_err());
}

#[test]
fn the_configured_minimum_is_readable() {
    let (_env, client, admin, creator) = setup();

    client.set_min_hold_for_access(&admin, &creator, &MIN_HOLD);
    assert_eq!(client.get_min_hold_for_access(&creator), Some(MIN_HOLD));
}

#[test]
fn raising_the_minimum_does_not_retroactively_revoke() {
    // The subscription records the minimum in force when granted, so a later
    // increase applies to new subscriptions rather than voiding paid-for access
    // mid-term.
    let (env, client, admin, creator) = setup();
    let wallet = Address::generate(&env);

    client.set_min_hold_for_access(&admin, &creator, &MIN_HOLD);
    buy(&client, &creator, &wallet, MIN_HOLD);
    client.subscribe(&creator, &wallet, &DURATION);

    client.set_min_hold_for_access(&admin, &creator, &(MIN_HOLD + 10));

    assert!(client.is_subscribed(&creator, &wallet));
}

#[test]
fn subscriptions_are_scoped_per_creator() {
    let (env, client, admin, creator_a) = setup();
    let creator_b = register_test_creator(&env, &client, "bob");
    let wallet = Address::generate(&env);

    client.set_min_hold_for_access(&admin, &creator_a, &MIN_HOLD);
    client.set_min_hold_for_access(&admin, &creator_b, &MIN_HOLD);
    buy(&client, &creator_a, &wallet, MIN_HOLD);
    client.subscribe(&creator_a, &wallet, &DURATION);

    assert!(client.is_subscribed(&creator_a, &wallet));
    assert!(
        !client.is_subscribed(&creator_b, &wallet),
        "a subscription to one creator must not grant access to another"
    );
}

#[test]
fn is_subscribed_is_false_for_an_unknown_subscription() {
    let (env, client, _admin, creator) = setup();
    let stranger = Address::generate(&env);

    assert!(!client.is_subscribed(&creator, &stranger));
    assert!(client.get_subscription(&creator, &stranger).is_none());
}
