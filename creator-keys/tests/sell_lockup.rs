//! Integration tests for the anti-flash-trade sell lockup (issue #753).
//!
//! Once the admin configures a lockup duration via `set_lockup_duration`,
//! `sell_key` rejects any sale made before the configured time has elapsed
//! since the seller's most recent buy, with `LockupPeriodActive` and a
//! `lockup_blocked` event. Sells after the window succeed.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_test_timestamp,
    test_env_with_auths,
};
use creator_keys::events::{self, LOCKUP_BLOCKED_EVENT_NAME};
use creator_keys::ContractError;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 100;
const LOCKUP_SECS: u64 = 86_400;
const BASE_TIMESTAMP: u64 = 1_700_000_000;

struct Setup<'a> {
    client: creator_keys::CreatorKeysContractClient<'a>,
    admin: Address,
    creator: Address,
}

fn setup_with_lockup(env: &Env) -> Setup<'_> {
    let (client, _) = register_creator_keys(env);
    let admin = set_key_price_for_tests(env, &client, KEY_PRICE);
    // set_lockup_duration requires the protocol admin.
    client.set_protocol_admin(&admin, &admin);
    client.set_lockup_duration(&admin, &LOCKUP_SECS);
    let creator = register_test_creator(env, &client, "alice");
    set_test_timestamp(env, BASE_TIMESTAMP);

    Setup {
        client,
        admin,
        creator,
    }
}

fn lockup_blocked_events(env: &Env) -> soroban_sdk::Vec<events::LockupBlockedEvent> {
    let mut found = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == LOCKUP_BLOCKED_EVENT_NAME {
            found.push_back(data.into_val(env));
        }
    }
    found
}

#[test]
fn test_sell_within_lockup_is_rejected_and_emits_event() {
    let env = test_env_with_auths();
    let s = setup_with_lockup(&env);

    let trader = Address::generate(&env);
    s.client.buy_key(&s.creator, &trader, &KEY_PRICE, &None);

    // Selling in the same ledger/timestamp as the buy is blocked.
    let result = s.client.try_sell_key(&s.creator, &trader, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::AllocationLocked)),
        "a sell inside the 24h lockup must be rejected"
    );

    // Capture events immediately after the rejection — any subsequent contract
    // invocation (including view calls) will flush the test-env event buffer.
    let events_found = lockup_blocked_events(&env);

    // State is untouched by the rejected sell.
    assert_eq!(client_supply(&s), 1);
    assert_eq!(s.client.get_key_balance(&s.creator, &trader), 1);

    assert_eq!(
        events_found.len(),
        1,
        "exactly one lockup_blocked event is emitted per rejection"
    );
    let payload = events_found.get(0).unwrap();
    assert_eq!(payload.creator_id, s.creator);
    assert_eq!(payload.seller, trader);
    assert_eq!(payload.last_buy_timestamp, BASE_TIMESTAMP);
    assert_eq!(payload.unlock_at, BASE_TIMESTAMP + LOCKUP_SECS);
    assert_eq!(payload.current_timestamp, BASE_TIMESTAMP);
}

fn client_supply(s: &Setup<'_>) -> u32 {
    s.client.get_total_key_supply(&s.creator)
}

#[test]
fn test_sell_after_lockup_succeeds() {
    let env = test_env_with_auths();
    let s = setup_with_lockup(&env);

    let trader = Address::generate(&env);
    s.client.buy_key(&s.creator, &trader, &KEY_PRICE, &None);

    // Advance past the lockup window; expiry is inclusive.
    set_test_timestamp(&env, BASE_TIMESTAMP + LOCKUP_SECS);
    let supply = s.client.sell_key(&s.creator, &trader, &None);
    assert_eq!(supply, 0);
    assert_eq!(s.client.get_key_balance(&s.creator, &trader), 0);
}

#[test]
fn test_last_buy_timestamp_is_updated_on_every_buy() {
    let env = test_env_with_auths();
    let s = setup_with_lockup(&env);

    let trader = Address::generate(&env);

    // First buy at BASE_TIMESTAMP.
    s.client.buy_key(&s.creator, &trader, &KEY_PRICE, &None);

    // Second buy 80 minutes later restarts the lockup window.
    let second_buy_ts = BASE_TIMESTAMP + 80_000;
    set_test_timestamp(&env, second_buy_ts);
    s.client.buy_key(&s.creator, &trader, &KEY_PRICE, &None);

    // Past the first buy's window but still inside the second buy's window:
    // the sell must stay blocked because last_buy_timestamp was refreshed.
    set_test_timestamp(&env, second_buy_ts + LOCKUP_SECS - 1);
    let result = s.client.try_sell_key(&s.creator, &trader, &None);
    assert_eq!(result, Err(Ok(ContractError::AllocationLocked)));

    // Once the refreshed window has elapsed the sell goes through.
    set_test_timestamp(&env, second_buy_ts + LOCKUP_SECS);
    s.client.sell_key(&s.creator, &trader, &None);
    // Two keys were bought and one has been sold.
    assert_eq!(client_supply(&s), 1);
    assert_eq!(s.client.get_key_balance(&s.creator, &trader), 1);
}

#[test]
fn test_admin_can_update_the_lockup_duration() {
    let env = test_env_with_auths();
    let s = setup_with_lockup(&env);

    // Shorten the window to one hour.
    s.client.set_lockup_duration(&s.admin, &3_600);
    assert_eq!(
        s.client.get_lockup_duration(),
        3_600,
        "the configured duration must be readable"
    );

    let trader = Address::generate(&env);
    s.client.buy_key(&s.creator, &trader, &KEY_PRICE, &None);

    set_test_timestamp(&env, BASE_TIMESTAMP + 3_600);
    s.client.sell_key(&s.creator, &trader, &None);
    assert_eq!(client_supply(&s), 0);
}

#[test]
fn test_non_admin_cannot_configure_the_lockup() {
    let env = test_env_with_auths();
    let s = setup_with_lockup(&env);

    let impostor = Address::generate(&env);
    let result = s.client.try_set_lockup_duration(&impostor, &LOCKUP_SECS);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_zero_duration_is_rejected() {
    let env = test_env_with_auths();
    let s = setup_with_lockup(&env);

    let result = s.client.try_set_lockup_duration(&s.admin, &0);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn test_lockup_is_inactive_until_configured() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");
    set_test_timestamp(&env, BASE_TIMESTAMP);

    // The default duration is reported for visibility...
    assert_eq!(client.get_lockup_duration(), 86_400);

    // ...but no sell is ever time-gated until the admin opts in.
    let trader = Address::generate(&env);
    client.buy_key(&creator, &trader, &KEY_PRICE, &None);
    client.sell_key(&creator, &trader, &None);
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert!(lockup_blocked_events(&env).is_empty());
}
