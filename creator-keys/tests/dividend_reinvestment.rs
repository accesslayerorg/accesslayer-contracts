//! Integration tests for the dividend reinvestment function (#805).
//!
//! Holders who receive dividends can call `reinvest_dividend` to automatically
//! use their unclaimed dividend balance to purchase additional keys via the
//! bonding curve in a single transaction, compounding returns.
//! Any remainder below the price of one key is returned.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::events::{self, DIVIDEND_REINVESTED_EVENT_NAME};
use creator_keys::ContractError;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 100;
const CREATOR_BPS: u32 = 8_000;
const PROTOCOL_BPS: u32 = 2_000;

struct Setup<'a> {
    client: creator_keys::CreatorKeysContractClient<'a>,
    admin: Address,
    creator: Address,
}

fn setup(env: &Env) -> Setup<'_> {
    let (client, _) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");

    Setup {
        client,
        admin,
        creator,
    }
}

#[test]
fn test_reinvest_dividend_with_no_claimable_fails() {
    let env = test_env_with_auths();
    let s = setup(&env);

    let holder = Address::generate(&env);
    // Buy 1 key for holder
    s.client.buy_key(&s.creator, &holder, &KEY_PRICE, &None);

    // No dividends distributed yet
    let result = s.client.try_reinvest_dividend(&s.creator, &holder);
    assert_eq!(
        result,
        Err(Ok(ContractError::NoDividendClaimable)),
        "reinvesting with 0 claimable dividend must return NoDividendClaimable"
    );
}

#[test]
fn test_reinvest_dividend_buys_keys_and_returns_remainder() {
    let env = test_env_with_auths();
    let s = setup(&env);

    let holder = Address::generate(&env);
    // Buy 1 key for holder (supply becomes 1)
    s.client.buy_key(&s.creator, &holder, &KEY_PRICE, &None);

    // Distribute dividends to key holders (supply = 1, so holder gets full net dividend)
    s.client
        .distribute_dividend(&s.creator, &s.admin, &1000i128);

    let claimable = s.client.get_claimable_dividend(&s.creator, &holder);
    assert!(claimable > 0, "holder must have claimable dividends");

    // Clear event log before reinvestment
    env.events().all();

    let pre_balance = s.client.get_key_balance(&s.creator, &holder);
    let result = s.client.reinvest_dividend(&s.creator, &holder);

    // Capture events immediately
    let events = env.events().all();
    let mut found_event = None;
    for (_, topics, data) in events.iter() {
        if let Some(t) = topics.get(0) {
            let name: Symbol = t.into_val(&env);
            if name == DIVIDEND_REINVESTED_EVENT_NAME {
                let payload: events::DividendReinvestedEvent = data.into_val(&env);
                found_event = Some(payload);
            }
        }
    }

    let payload = found_event.expect("DividendReinvestedEvent must be emitted");
    assert_eq!(payload.wallet, holder);
    assert_eq!(payload.key_id, s.creator);
    assert_eq!(payload.keys_bought, result.keys_bought);
    assert_eq!(payload.remainder_returned, result.remainder_returned);

    // Verify keys bought and remainder
    assert!(
        result.keys_bought > 0,
        "at least one key must be bought if claimable >= key_price"
    );
    assert_eq!(
        s.client.get_key_balance(&s.creator, &holder),
        pre_balance + result.keys_bought,
        "holder key balance must increase by keys_bought"
    );

    // Verify unclaimed dividend is cleared to 0
    let post_claimable = s.client.get_claimable_dividend(&s.creator, &holder);
    assert_eq!(
        post_claimable, 0,
        "unclaimed dividend balance must be cleared to 0"
    );
}

#[test]
fn test_reinvest_dividend_fails_when_paused() {
    let env = test_env_with_auths();
    let s = setup(&env);

    let holder = Address::generate(&env);
    s.client.buy_key(&s.creator, &holder, &KEY_PRICE, &None);

    let other = Address::generate(&env);
    s.client.buy_key(&s.creator, &other, &KEY_PRICE, &None);

    // Pause contract
    s.client.pause(&s.admin);

    let result = s.client.try_reinvest_dividend(&s.creator, &holder);
    assert_eq!(result, Err(Ok(ContractError::ProtocolPaused)));
}

#[test]
fn test_reinvest_dividend_fails_when_blacklisted() {
    let env = test_env_with_auths();
    let s = setup(&env);

    let holder = Address::generate(&env);
    s.client.buy_key(&s.creator, &holder, &KEY_PRICE, &None);

    let other = Address::generate(&env);
    s.client.buy_key(&s.creator, &other, &KEY_PRICE, &None);

    // Blacklist holder
    s.client.blacklist_wallet(&s.admin, &holder);

    let result = s.client.try_reinvest_dividend(&s.creator, &holder);
    assert_eq!(result, Err(Ok(ContractError::WalletBlacklisted)));
}
