//! Regression test: protocol treasury balance unchanged after a failed trade (#371).
//!
//! Protocol fees are credited to the treasury only on the success path of
//! `buy_key` / `sell_key`. When a trade fails, no fee — not even a partial one —
//! may reach the treasury. These tests record the read-only treasury balance
//! before a trade that is expected to fail, trigger the failing trade, and
//! assert the balance is identical afterward, covering both a buy failure and a
//! sell failure.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

/// Records the treasury balance, attempts a buy that fails with
/// InsufficientPayment, then asserts the balance is unchanged.
#[test]
fn test_treasury_balance_unchanged_after_failed_buy_insufficient_payment() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let protocol_recipient = Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Read-only treasury balance before the failing buy.
    let treasury_before = client.get_protocol_recipient_balance();

    // Pay one stroop below the key price — should fail with InsufficientPayment.
    let result = client.try_buy_key(&creator, &buyer, &(KEY_PRICE - 1), &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::InsufficientPayment)),
        "buy should fail with InsufficientPayment"
    );

    // No fee — partial or otherwise — may have reached the treasury.
    let treasury_after = client.get_protocol_recipient_balance();
    assert_eq!(
        treasury_after, treasury_before,
        "treasury balance must be unchanged after a failed buy"
    );
}

/// Records the treasury balance, attempts a buy against an unregistered
/// creator, then asserts the balance is unchanged.
#[test]
fn test_treasury_balance_unchanged_after_failed_buy_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let protocol_recipient = Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    // A successful buy first establishes a non-zero treasury balance, so the
    // assertion proves the balance is held steady rather than merely staying 0.
    let registered = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    client.buy_key(&registered, &buyer, &KEY_PRICE, &None);

    let treasury_before = client.get_protocol_recipient_balance();
    assert!(
        treasury_before > 0,
        "setup: treasury should hold a fee after one successful buy"
    );

    // Buying keys for a creator that was never registered must fail.
    let unregistered = Address::generate(&env);
    let result = client.try_buy_key(&unregistered, &buyer, &KEY_PRICE, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "buy should fail with NotRegistered"
    );

    let treasury_after = client.get_protocol_recipient_balance();
    assert_eq!(
        treasury_after, treasury_before,
        "treasury balance must be unchanged after a failed buy against an unregistered creator"
    );
}

/// Records the treasury balance, attempts a sell that fails with
/// InsufficientBalance, then asserts the balance is unchanged.
#[test]
fn test_treasury_balance_unchanged_after_failed_sell_insufficient_balance() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let protocol_recipient = Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    // Seed a non-zero treasury balance with one successful buy.
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    let treasury_before = client.get_protocol_recipient_balance();
    assert!(
        treasury_before > 0,
        "setup: treasury should hold a fee after one successful buy"
    );

    // A different address holding zero keys cannot sell.
    let non_holder = Address::generate(&env);
    let result = client.try_sell_key(&creator, &non_holder, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::InsufficientBalance)),
        "sell should fail with InsufficientBalance"
    );

    let treasury_after = client.get_protocol_recipient_balance();
    assert_eq!(
        treasury_after, treasury_before,
        "treasury balance must be unchanged after a failed sell"
    );
}
