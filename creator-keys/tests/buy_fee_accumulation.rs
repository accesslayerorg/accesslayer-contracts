//! Integration test for fee accumulation across multiple buy transactions.
//!
//! After five buys from two distinct wallets for the same creator, the protocol
//! fee recipient and creator fee recipient balances should each equal the sum
//! of the per-buy fees computed independently.

mod contract_test_env;

use contract_test_env::{
    compute_expected_creator_fee, compute_expected_protocol_fee, register_creator_keys,
    register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

#[test]
fn test_buy_fee_accumulation_matches_independent_sum() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "alice");
    let wallet_a = soroban_sdk::Address::generate(&env);
    let wallet_b = soroban_sdk::Address::generate(&env);

    // Compute expected fees independently (not from contract reads).
    let expected_protocol_fee = compute_expected_protocol_fee(KEY_PRICE, PROTOCOL_BPS);
    let expected_creator_fee = compute_expected_creator_fee(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    // Record pre-buy balances.
    let protocol_balance_before = client.get_protocol_recipient_balance();
    let creator_balance_before = client.get_creator_fee_balance(&creator);
    assert_eq!(
        protocol_balance_before, 0,
        "protocol balance should start at 0"
    );
    assert_eq!(
        creator_balance_before, 0,
        "creator balance should start at 0"
    );

    // Wallet A buys 3 keys.
    for _ in 0..3 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &wallet_a, &quote.total_amount, &None);
    }

    // Wallet B buys 2 keys.
    for _ in 0..2 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &wallet_b, &quote.total_amount, &None);
    }

    // Total buys = 5, so expected totals are 5x the per-buy fee.
    let expected_total_protocol = expected_protocol_fee * 5;
    let expected_total_creator = expected_creator_fee * 5;

    let protocol_balance_after = client.get_protocol_recipient_balance();
    let creator_balance_after = client.get_creator_fee_balance(&creator);

    assert_eq!(
        protocol_balance_after - protocol_balance_before,
        expected_total_protocol,
        "protocol fee recipient balance should increase by sum of 5 protocol fees"
    );
    assert_eq!(
        creator_balance_after - creator_balance_before,
        expected_total_creator,
        "creator fee recipient balance should increase by sum of 5 creator fees"
    );

    // Sanity check: the two balances are different (non-zero, distinct).
    assert!(
        protocol_balance_after > 0,
        "protocol balance must be positive"
    );
    assert!(
        creator_balance_after > 0,
        "creator balance must be positive"
    );
    assert_ne!(
        protocol_balance_after, creator_balance_after,
        "protocol and creator balances should differ given 90/10 split"
    );
}
