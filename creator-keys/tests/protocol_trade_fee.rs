//! Integration tests for the protocol trade fee collector (issue #751).
//!
//! Once the admin configures the fee via `set_protocol_fee`, every buy and
//! sell deducts the configured basis points from the trade amount and credits
//! them to the protocol treasury before the creator payout is computed. With
//! a fee config that routes the full remainder to the creator, a 100 stroop
//! trade must send exactly 1 stroop to the treasury and 99 to the creator or
//! seller. A rate of 0 bps produces no treasury credit and no event.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::events::{self, FEE_COLLECTED_EVENT_NAME};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol, Vec,
};

const KEY_PRICE: i128 = 100;
/// Full remainder routed to the creator so the split math is easy to verify.
const CREATOR_BPS: u32 = 10_000;
const PROTOCOL_BPS: u32 = 0;

struct Setup<'a> {
    client: creator_keys::CreatorKeysContractClient<'a>,
    admin: Address,
    creator: Address,
    treasury: Address,
}

fn setup(env: &Env) -> Setup<'_> {
    let (client, _contract_id) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");
    let treasury = Address::generate(env);

    Setup {
        client,
        admin,
        creator,
        treasury,
    }
}

/// Collects `(treasury, amount)` pairs from fee collection events in the log.
fn collected_fees(env: &Env) -> Vec<(Address, i128)> {
    let mut found = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == FEE_COLLECTED_EVENT_NAME {
            let payload: events::FeeCollectedEvent = data.into_val(env);
            found.push_back((payload.treasury, payload.amount));
        }
    }
    found
}

#[test]
fn test_buy_routes_one_percent_to_treasury_and_remainder_to_creator() {
    let env = test_env_with_auths();
    let s = setup(&env);

    // `None` selects the default 100 bps (1%) rate.
    s.client.set_protocol_fee(&s.admin, &None, &s.treasury);
    let (rate, recipient) = s.client.get_protocol_trade_fee();
    assert_eq!(rate, 100, "default rate must be 100 bps (1%)");
    assert_eq!(recipient, Some(s.treasury.clone()));

    let buyer = Address::generate(&env);
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    // Capture events immediately after the trade — view calls below will
    // flush the Soroban test-env event buffer.
    let fees = collected_fees(&env);

    assert_eq!(
        s.client.get_treasury_balance(),
        1,
        "1% of a 100 stroop buy must reach the treasury"
    );
    assert_eq!(
        s.client.get_creator_fee_balance(&s.creator),
        99,
        "the creator must receive the 99 stroop remainder"
    );

    assert_eq!(fees.len(), 1, "exactly one fee_collected event per trade");
    assert_eq!(fees.get(0).unwrap(), (s.treasury.clone(), 1));
}

#[test]
fn test_sell_routes_one_percent_to_treasury_and_remainder_to_seller() {
    let env = test_env_with_auths();
    let s = setup(&env);

    s.client.set_protocol_fee(&s.admin, &None, &s.treasury);

    let trader = Address::generate(&env);
    s.client.buy_key(&s.creator, &trader, &KEY_PRICE, &None);
    assert_eq!(s.client.get_treasury_balance(), 1);

    s.client.sell_key(&s.creator, &trader, &None);

    // Capture events immediately after the sell — any subsequent contract
    // invocation (including view calls) will flush the test-env event buffer.
    //
    // Math: price=100, protocol trade fee=1 (1%), net=99.
    // CREATOR_BPS=10_000 means 100% of net (99 stroops) flows to the creator;
    // seller proceeds = net - creator_fee = 99 - 99 = 0.
    let all_events = env.events().all();
    let sell_events: std::vec::Vec<_> = all_events
        .iter()
        .filter(|(_, topics, _)| {
            topics.get(0).map(|v| {
                let name: Symbol = v.into_val(&env);
                name == events::SELL_EVENT_NAME
            }) == Some(true)
        })
        .collect();
    let fees = collected_fees(&env);

    assert_eq!(
        s.client.get_treasury_balance(),
        2,
        "the sell must add another 1% of the 100 stroop price"
    );

    assert_eq!(sell_events.len(), 1, "exactly one sell event expected");
    let (_, _, data) = &sell_events[0];
    let payload: events::KeysSoldEvent = data.into_val(&env);
    // With CREATOR_BPS=10_000 the full net goes to the creator, so the
    // seller receives 0 stroops (proceeds are the seller's share only).
    assert_eq!(
        payload.proceeds, 0,
        "seller proceeds are 0 when 100% of net is routed to the creator"
    );

    assert!(
        !fees.is_empty(),
        "a fee_collected event must be emitted on the sell"
    );
    assert!(
        fees.iter().any(|fee| fee.0 == s.treasury && fee.1 == 1),
        "the sell's fee_collected event must carry the treasury and 1 stroop"
    );
}

#[test]
fn test_admin_can_update_fee_rate_and_treasury_address() {
    let env = test_env_with_auths();
    let s = setup(&env);

    let first_treasury = Address::generate(&env);
    s.client
        .set_protocol_fee(&s.admin, &Some(500), &first_treasury);

    let buyer = Address::generate(&env);
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(
        s.client.get_treasury_balance(),
        5,
        "500 bps of a 100 stroop buy is 5 stroops"
    );
    assert_eq!(
        s.client.get_treasury_address(),
        Some(first_treasury.clone())
    );

    let second_treasury = Address::generate(&env);
    s.client
        .set_protocol_fee(&s.admin, &Some(200), &second_treasury);

    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(
        s.client.get_treasury_balance(),
        7,
        "only the new 200 bps rate applies to later trades"
    );
    assert_eq!(
        s.client.get_treasury_address(),
        Some(second_treasury.clone()),
        "the treasury address update must be visible"
    );
}

#[test]
fn test_zero_bps_transfers_full_amount_with_no_treasury_call() {
    let env = test_env_with_auths();
    let s = setup(&env);

    s.client.set_protocol_fee(&s.admin, &Some(0), &s.treasury);
    let (rate, _) = s.client.get_protocol_trade_fee();
    assert_eq!(rate, 0);

    let buyer = Address::generate(&env);
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    assert_eq!(
        s.client.get_treasury_balance(),
        0,
        "a zero fee must never credit the treasury"
    );
    assert_eq!(
        s.client.get_creator_fee_balance(&s.creator),
        KEY_PRICE,
        "the creator receives the full amount at 0 bps"
    );
    assert!(
        collected_fees(&env).is_empty(),
        "no fee_collected event may be emitted at 0 bps"
    );
}

#[test]
fn test_dormant_when_not_configured() {
    let env = test_env_with_auths();
    let s = setup(&env);

    let (rate, recipient) = s.client.get_protocol_trade_fee();
    assert_eq!(rate, 0);
    assert_eq!(recipient, None, "no treasury is configured yet");

    let buyer = Address::generate(&env);
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    assert_eq!(s.client.get_treasury_balance(), 0);
    assert_eq!(
        s.client.get_creator_fee_balance(&s.creator),
        KEY_PRICE,
        "without the trade fee the full amount flows to the creator"
    );
    assert!(collected_fees(&env).is_empty());
}
