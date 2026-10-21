//! Tests for the `airdrop_keys` recipient limit (issue #537).
//!
//! The airdrop entrypoint accepts at most [`MAX_AIRDROP_RECIPIENTS`] (50)
//! recipient entries per call. These tests pin the boundary behavior:
//! 51 entries revert with a clear error and mint nothing, while exactly 50
//! entries succeed and credit every recipient.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
    DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use creator_keys::{AirdropEntry, ContractError, MAX_AIRDROP_RECIPIENTS};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

const KEY_PRICE: i128 = 100;
const KEYS_PER_RECIPIENT: u32 = 1;

/// Builds `count` airdrop entries, each minting one key to a fresh wallet.
fn airdrop_entries(env: &Env, count: u32) -> Vec<AirdropEntry> {
    let mut entries = Vec::new(env);
    for _ in 0..count {
        entries.push_back(AirdropEntry {
            address: Address::generate(env),
            amount: KEYS_PER_RECIPIENT,
        });
    }
    entries
}

/// Expected creator payment for `key_count` keys under the default flat curve:
/// curve cost plus the protocol fee on that cost.
fn expected_airdrop_payment(key_count: u32) -> i128 {
    let curve_cost = KEY_PRICE * i128::from(key_count);
    let protocol_fee = (curve_cost * i128::from(DEFAULT_PROTOCOL_BPS)) / 10_000;
    curve_cost + protocol_fee
}

#[test]
fn test_airdrop_over_limit_reverts_with_clear_error() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        KEY_PRICE,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");

    let over_limit = MAX_AIRDROP_RECIPIENTS + 1;
    let entries = airdrop_entries(&env, over_limit);
    let payment = expected_airdrop_payment(over_limit);

    let result = client.try_airdrop_keys(&creator, &creator, &entries, &payment);

    assert_eq!(
        result,
        Err(Ok(ContractError::AirdropRecipientLimitExceeded)),
        "51 recipients must revert with AirdropRecipientLimitExceeded"
    );
}

#[test]
fn test_airdrop_at_limit_succeeds_and_credits_every_recipient() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        KEY_PRICE,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");

    let entries = airdrop_entries(&env, MAX_AIRDROP_RECIPIENTS);
    let payment = expected_airdrop_payment(MAX_AIRDROP_RECIPIENTS);

    let summary = client.airdrop_keys(&creator, &creator, &entries, &payment);

    assert_eq!(summary.total_keys, MAX_AIRDROP_RECIPIENTS);
    assert_eq!(summary.recipient_count, MAX_AIRDROP_RECIPIENTS);
    assert_eq!(summary.total_cost, payment);
    assert_eq!(
        client.get_total_key_supply(&creator),
        MAX_AIRDROP_RECIPIENTS,
        "supply must grow by the total airdropped amount"
    );
    assert_eq!(
        client.get_creator_holder_count(&creator),
        MAX_AIRDROP_RECIPIENTS,
        "each recipient is a new holder"
    );
    for entry in entries.iter() {
        assert_eq!(
            client.get_key_balance(&creator, &entry.address),
            KEYS_PER_RECIPIENT,
            "every recipient at the limit must be credited"
        );
    }
}

#[test]
fn test_failed_over_limit_airdrop_mints_nothing() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        KEY_PRICE,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");

    // Seed non-trivial state so "unchanged" is distinguishable from "empty".
    let existing_holder = Address::generate(&env);
    client.buy_key(&creator, &existing_holder, &KEY_PRICE, &None);
    let supply_before = client.get_total_key_supply(&creator);
    let holder_count_before = client.get_creator_holder_count(&creator);
    let protocol_balance_before = client.get_protocol_recipient_balance();

    let over_limit = MAX_AIRDROP_RECIPIENTS + 1;
    let entries = airdrop_entries(&env, over_limit);
    let payment = expected_airdrop_payment(over_limit);

    let result = client.try_airdrop_keys(&creator, &creator, &entries, &payment);
    assert!(result.is_err(), "over-limit airdrop must revert");

    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "supply must not change after a failed airdrop"
    );
    assert_eq!(
        client.get_creator_holder_count(&creator),
        holder_count_before,
        "holder count must not change after a failed airdrop"
    );
    assert_eq!(
        client.get_protocol_recipient_balance(),
        protocol_balance_before,
        "no protocol fee may accrue from a failed airdrop"
    );
    for entry in entries.iter() {
        assert_eq!(
            client.get_key_balance(&creator, &entry.address),
            0,
            "no keys may be minted to any recipient after a failed airdrop"
        );
    }
}
