//! Integration tests for the buy payment guard (issue #708).
//!
//! The buy path must verify the attached XLM payment covers the full cost
//! including fees before executing. These tests confirm:
//!
//! - paying exactly the required amount succeeds
//! - paying one stroop less reverts with the contract's insufficient-funds
//!   error ([`ContractError::InsufficientPayment`])
//! - overpaying succeeds and the excess is neither refunded nor credited to
//!   any fee balance (excess handling is the caller's responsibility)
//! - a zero payment is rejected before any state mutation
//! - failed buys leave supply unchanged and emit no buy event
//!
//! # Required amount and error-code mapping notes
//!
//! Fees are carved out of the bonding-curve price (`docs/fee-assumptions.md`),
//! so the enforced minimum payment is the quoted `price`; the quote response's
//! `total_amount` additionally displays the itemized fee components for indexers,
//! and paying it simply counts as overpayment. The issue text uses generic
//! `insufficient_funds` naming: this contract maps underpayment to its dedicated
//! ABI-stable [`ContractError::InsufficientPayment`] code, and zero/non-positive
//! payments to [`ContractError::NotPositiveAmount`], which fires first as a more
//! precise guard for the degenerate zero case.

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, compute_expected_creator_fee, compute_expected_protocol_fee,
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{events, ContractError};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

/// Counts emitted buy events in the environment's event log.
fn count_buy_events(env: &Env) -> usize {
    env.events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            let name: Symbol = topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .expect("event topic tuple must contain an event name")
                .into_val(env);
            name == events::BUY_EVENT_NAME
        })
        .count()
}

fn setup(
    env: &Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    Address,
    Address,
) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator, Address::generate(env))
}

// ---------------------------------------------------------------------------
// Exact payment succeeds
// ---------------------------------------------------------------------------

#[test]
fn test_buy_with_exact_required_amount_succeeds() {
    let env = test_env_with_auths();
    let (client, creator, buyer) = setup(&env);

    // Fees are carved out of the bonding-curve price, so the quoted price is
    // the exact required amount the buy path enforces.
    let quote = client.get_buy_quote(&creator);
    assert_eq!(quote.price, KEY_PRICE);

    let supply = client.buy_key(&creator, &buyer, &quote.price, &None);
    assert_eq!(supply, 1);
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);

    // Fee accounting reflects exactly the price split, nothing more.
    assert_eq!(
        client.get_creator_fee_balance(&creator),
        compute_expected_creator_fee(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS)
    );
}

// ---------------------------------------------------------------------------
// One stroop short panics with insufficient funds and mutates nothing
// ---------------------------------------------------------------------------

#[test]
fn test_buy_one_stroop_short_panics_with_insufficient_funds() {
    let env = test_env_with_auths();
    let (client, creator, buyer) = setup(&env);

    let quote = client.get_buy_quote(&creator);
    let snapshot_before = capture_snapshot(&client, &creator, &buyer);
    let buy_events_before = count_buy_events(&env);

    let result = client.try_buy_key(&creator, &buyer, &(quote.price - 1), &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::InsufficientPayment)),
        "underpayment by one stroop must revert with InsufficientPayment"
    );

    let snapshot_after = capture_snapshot(&client, &creator, &buyer);
    snapshot_before.assert_unchanged(&snapshot_after);

    // No supply change and no buy event on panic.
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(count_buy_events(&env), buy_events_before);
}

#[test]
fn test_buy_one_stroop_short_leaves_fee_balances_untouched() {
    let env = test_env_with_auths();
    let (client, creator, buyer) = setup(&env);

    let quote = client.get_buy_quote(&creator);
    let creator_fees_before = client.get_creator_fee_balance(&creator);
    let protocol_balance_before = client.get_protocol_recipient_balance();

    let result = client.try_buy_key(&creator, &buyer, &(quote.price - 1), &None);
    assert_eq!(result, Err(Ok(ContractError::InsufficientPayment)));

    assert_eq!(
        client.get_creator_fee_balance(&creator),
        creator_fees_before
    );
    assert_eq!(
        client.get_protocol_recipient_balance(),
        protocol_balance_before
    );
}

// ---------------------------------------------------------------------------
// Overpayment succeeds; excess is not refunded or credited anywhere
// ---------------------------------------------------------------------------

#[test]
fn test_buy_overpayment_succeeds_and_excess_is_not_credited() {
    let env = test_env_with_auths();
    let (client, creator, buyer) = setup(&env);

    let quote = client.get_buy_quote(&creator);
    // Paying more than the required price also covers paying the quote's
    // displayed `total_amount` (price plus itemized fees) — both overpay.
    let excess: i128 = 9_999;
    let payment = quote.price + excess;

    let creator_fees_before = client.get_creator_fee_balance(&creator);
    let protocol_balance_before = client.get_protocol_recipient_balance();

    let supply = client.buy_key(&creator, &buyer, &payment, &None);
    assert_eq!(supply, 1, "overpayment must still succeed");

    // Fee accounting is derived from the key price only — the excess stroops
    // are invisible to the contract (refund/credit is handled by the caller).
    let expected_creator_fee = compute_expected_creator_fee(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let expected_protocol_fee = compute_expected_protocol_fee(KEY_PRICE, PROTOCOL_BPS);
    assert_eq!(
        client.get_creator_fee_balance(&creator),
        creator_fees_before + expected_creator_fee
    );
    assert_eq!(
        client.get_protocol_recipient_balance(),
        protocol_balance_before + expected_protocol_fee
    );
    assert_eq!(
        expected_creator_fee + expected_protocol_fee,
        KEY_PRICE,
        "fee split must account for exactly the price"
    );
}

// ---------------------------------------------------------------------------
// Zero payment is rejected before any state change
// ---------------------------------------------------------------------------

#[test]
fn test_buy_zero_payment_panics_without_state_change() {
    let env = test_env_with_auths();
    let (client, creator, buyer) = setup(&env);

    let snapshot_before = capture_snapshot(&client, &creator, &buyer);
    let buy_events_before = count_buy_events(&env);

    // The zero-amount guard (`NotPositiveAmount`) fires before the payment
    // comparison; it is the contract's precise variant of insufficient funds.
    let result = client.try_buy_key(&creator, &buyer, &0i128, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::NotPositiveAmount)),
        "zero XLM payment must be rejected"
    );

    let snapshot_after = capture_snapshot(&client, &creator, &buyer);
    snapshot_before.assert_unchanged(&snapshot_after);

    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(count_buy_events(&env), buy_events_before);
}
