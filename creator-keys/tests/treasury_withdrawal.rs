//! Integration tests for issue #517 — protocol treasury withdrawal function.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_pricing_and_fees, test_env_with_auths};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal,
};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Sets a protocol admin in the contract and returns the admin address.
fn set_admin(env: &Env, client: &CreatorKeysContractClient<'_>) -> Address {
    let first_admin = Address::generate(env);
    let new_admin = Address::generate(env);
    client.set_protocol_admin(&first_admin, &new_admin);
    new_admin
}

/// Full setup: pricing + fees + admin. Returns (client, admin).
fn setup_with_admin(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _id) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, 100i128, 9000, 1000);
    let admin = set_admin(env, &client);
    (client, admin)
}

/// Buy one key and return the protocol_fee that should have been credited.
fn buy_one_key(env: &Env, client: &CreatorKeysContractClient<'_>) -> i128 {
    let creator = Address::generate(env);
    let buyer = Address::generate(env);
    client.register_creator(
        &creator,
        &soroban_sdk::String::from_str(env, "alice"),
        &None,
        &None,
        &None,
    );
    client.buy_key(&creator, &buyer, &100i128, &None);
    // 10% of 100 = 10 stroops
    10i128
}

// ── get_treasury_balance ──────────────────────────────────────────────────────

#[test]
fn get_treasury_balance_returns_zero_before_any_trades() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);
    assert_eq!(client.get_treasury_balance(), 0i128);
}

#[test]
fn get_treasury_balance_increases_after_buy_key() {
    let env = test_env_with_auths();
    let (client, _admin) = setup_with_admin(&env);

    let fee = buy_one_key(&env, &client);
    assert_eq!(client.get_treasury_balance(), fee);
}

#[test]
fn get_treasury_balance_accumulates_across_multiple_buys() {
    let env = test_env_with_auths();
    let (client, _admin) = setup_with_admin(&env);

    let creator = Address::generate(&env);
    client.register_creator(
        &creator,
        &soroban_sdk::String::from_str(&env, "bob"),
        &None,
        &None,
        &None,
    );

    let buyer1 = Address::generate(&env);
    let buyer2 = Address::generate(&env);
    client.buy_key(&creator, &buyer1, &100i128, &None);
    client.buy_key(&creator, &buyer2, &100i128, &None);

    // Each buy contributes 10 (10% of 100), two buys = 20
    assert_eq!(client.get_treasury_balance(), 20i128);
}

// ── withdraw_treasury ─────────────────────────────────────────────────────────

#[test]
fn withdraw_treasury_succeeds_for_admin_partial() {
    let env = test_env_with_auths();
    let (client, admin) = setup_with_admin(&env);

    buy_one_key(&env, &client); // treasury = 10

    let recipient = Address::generate(&env);
    let remaining = client.withdraw_treasury(&admin, &5i128, &recipient);
    assert_eq!(remaining, 5i128);
    assert_eq!(client.get_treasury_balance(), 5i128);
}

#[test]
fn withdraw_treasury_succeeds_full_withdrawal_leaves_zero() {
    let env = test_env_with_auths();
    let (client, admin) = setup_with_admin(&env);

    buy_one_key(&env, &client); // treasury = 10

    let recipient = Address::generate(&env);
    let remaining = client.withdraw_treasury(&admin, &10i128, &recipient);
    assert_eq!(remaining, 0i128);
    assert_eq!(client.get_treasury_balance(), 0i128);
}

#[test]
fn withdraw_treasury_emits_correct_event() {
    let env = test_env_with_auths();
    let (client, admin) = setup_with_admin(&env);

    buy_one_key(&env, &client); // treasury = 10

    let recipient = Address::generate(&env);
    client.withdraw_treasury(&admin, &7i128, &recipient);

    let events = env.events().all();
    // Find the treasury withdrawal event (last event emitted)
    let last = events.last().unwrap();
    let data: creator_keys::events::TreasuryWithdrawalEvent = last.2.into_val(&env);
    assert_eq!(data.amount, 7i128);
    assert_eq!(data.recipient, recipient);
    assert_eq!(data.remaining_balance, 3i128);
}

#[test]
fn withdraw_treasury_rejects_non_admin() {
    let env = test_env_with_auths();
    let (client, _admin) = setup_with_admin(&env);

    buy_one_key(&env, &client);

    let impostor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let result = client.try_withdraw_treasury(&impostor, &5i128, &recipient);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn withdraw_treasury_rejects_zero_amount() {
    let env = test_env_with_auths();
    let (client, admin) = setup_with_admin(&env);

    buy_one_key(&env, &client);

    let recipient = Address::generate(&env);
    let result = client.try_withdraw_treasury(&admin, &0i128, &recipient);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn withdraw_treasury_rejects_negative_amount() {
    let env = test_env_with_auths();
    let (client, admin) = setup_with_admin(&env);

    buy_one_key(&env, &client);

    let recipient = Address::generate(&env);
    let result = client.try_withdraw_treasury(&admin, &-1i128, &recipient);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn withdraw_treasury_rejects_over_withdrawal() {
    let env = test_env_with_auths();
    let (client, admin) = setup_with_admin(&env);

    buy_one_key(&env, &client); // treasury = 10

    let recipient = Address::generate(&env);
    let result = client.try_withdraw_treasury(&admin, &11i128, &recipient);
    assert_eq!(result, Err(Ok(ContractError::InsufficientTreasuryBalance)));
}

#[test]
fn withdraw_treasury_rejects_over_withdrawal_on_empty_balance() {
    let env = test_env_with_auths();
    let (client, admin) = setup_with_admin(&env);

    // No buys — treasury = 0
    let recipient = Address::generate(&env);
    let result = client.try_withdraw_treasury(&admin, &1i128, &recipient);
    assert_eq!(result, Err(Ok(ContractError::InsufficientTreasuryBalance)));
}

#[test]
fn withdraw_treasury_multiple_partial_withdrawals_track_correctly() {
    let env = test_env_with_auths();
    let (client, admin) = setup_with_admin(&env);

    let creator = Address::generate(&env);
    client.register_creator(
        &creator,
        &soroban_sdk::String::from_str(&env, "charlie"),
        &None,
        &None,
        &None,
    );

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100i128, &None);
    client.buy_key(&creator, &buyer, &100i128, &None);
    client.buy_key(&creator, &buyer, &100i128, &None);
    // treasury = 30

    let recipient = Address::generate(&env);
    client.withdraw_treasury(&admin, &10i128, &recipient); // remaining = 20
    client.withdraw_treasury(&admin, &10i128, &recipient); // remaining = 10
    client.withdraw_treasury(&admin, &10i128, &recipient); // remaining = 0

    assert_eq!(client.get_treasury_balance(), 0i128);
}

#[test]
fn treasury_balance_is_independent_of_protocol_fee_recipient_balance() {
    // Both accumulators are credited independently; clearing one must not affect the other.
    let env = test_env_with_auths();
    let (client, admin) = setup_with_admin(&env);

    let fee_recipient = Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &fee_recipient);

    buy_one_key(&env, &client); // treasury = 10, fee_recipient_balance = 10

    // Withdraw all treasury
    let recipient = Address::generate(&env);
    client.withdraw_treasury(&admin, &10i128, &recipient);
    assert_eq!(client.get_treasury_balance(), 0i128);

    // Next buy credits treasury again, independently
    buy_one_key(&env, &client);
    assert_eq!(client.get_treasury_balance(), 10i128);
}
