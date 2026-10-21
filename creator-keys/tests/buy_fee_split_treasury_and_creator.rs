//! Integration tests for fee splitting between protocol treasury and creator fee recipient (#680).
//!
//! Verifies that:
//! 1. Protocol fee configured at 500 bps (5%) credits treasury with exactly 5% of gross cost.
//! 2. Creator fee configured at 200 bps (2%) credits creator fee recipient with exactly 2% of gross cost.
//! 3. Buyer is charged gross cost + both fees.
//! 4. No stroop rounding error — sum of all credits equals total buyer charge.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, test_env_with_auths, STROOPS_PER_DISPLAY_UNIT,
};
use soroban_sdk::testutils::Address as _;

const PROTOCOL_BPS: u32 = 500; // 5% protocol fee
const CREATOR_BPS: u32 = 200; // 2% creator fee

#[test]
fn test_buy_splits_fees_correctly_between_treasury_and_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    let gross_cost: i128 = 10 * STROOPS_PER_DISPLAY_UNIT; // 10 XLM = 100,000,000 stroops

    // Configure pricing and fee split: 500 bps (5%) protocol fee, 200 bps (2%) creator fee
    client.set_key_price(&admin, &gross_cost);
    client.set_protocol_admin(&admin, &admin);
    client.set_fee_config(&admin, &CREATOR_BPS, &PROTOCOL_BPS);

    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    // Record initial balance states
    let treasury_balance_before = client.get_treasury_balance();
    let creator_balance_before = client.get_creator_fee_balance(&creator);
    assert_eq!(treasury_balance_before, 0, "treasury balance starts at 0");
    assert_eq!(creator_balance_before, 0, "creator fee balance starts at 0");

    // Fetch quote and verify expected component breakdown
    let quote = client.get_buy_quote(&creator);
    let expected_protocol_fee = (gross_cost * PROTOCOL_BPS as i128) / 10_000; // 5% = 5,000,000 stroops
    let expected_creator_fee = (gross_cost * CREATOR_BPS as i128) / 10_000; // 2% = 2,000,000 stroops
    let expected_total_charge = gross_cost + expected_protocol_fee + expected_creator_fee; // 107,000,000 stroops

    assert_eq!(quote.price, gross_cost, "quote price equals gross cost");
    assert_eq!(
        quote.protocol_fee, expected_protocol_fee,
        "treasury protocol fee equals exactly 5% of gross cost"
    );
    assert_eq!(
        quote.creator_fee, expected_creator_fee,
        "creator recipient fee equals exactly 2% of gross cost"
    );
    assert_eq!(
        quote.total_amount, expected_total_charge,
        "buyer total charge equals gross cost + both fees"
    );

    // Execute buy transaction
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    // Record balances after buy transaction
    let treasury_balance_after = client.get_treasury_balance();
    let creator_balance_after = client.get_creator_fee_balance(&creator);

    let treasury_delta = treasury_balance_after - treasury_balance_before;
    let creator_delta = creator_balance_after - creator_balance_before;

    // Acceptance Criterion 1: Treasury receives exactly 5% of gross cost
    assert_eq!(
        treasury_delta, expected_protocol_fee,
        "treasury received exactly 5% of gross cost"
    );

    // Acceptance Criterion 2: Creator recipient receives exactly 2% of gross cost
    assert_eq!(
        creator_delta, expected_creator_fee,
        "creator recipient received exactly 2% of gross cost"
    );

    // Acceptance Criterion 3: Buyer charged gross cost plus both fees
    assert_eq!(
        quote.total_amount,
        gross_cost + expected_protocol_fee + expected_creator_fee,
        "buyer charged gross cost + both fees"
    );

    // Acceptance Criterion 4: No stroop rounding error — sum of all credits equals total buyer charge
    let total_credited = gross_cost + treasury_delta + creator_delta;
    assert_eq!(
        total_credited, quote.total_amount,
        "sum of credits (gross cost + treasury fee + creator fee) equals total buyer charge with no stroop rounding error"
    );
}

#[test]
fn test_buy_fee_split_accumulates_across_multiple_buys() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    let gross_cost: i128 = 5 * STROOPS_PER_DISPLAY_UNIT; // 5 XLM = 50,000,000 stroops

    client.set_key_price(&admin, &gross_cost);
    client.set_protocol_admin(&admin, &admin);
    client.set_fee_config(&admin, &CREATOR_BPS, &PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "bob");
    let buyer1 = soroban_sdk::Address::generate(&env);
    let buyer2 = soroban_sdk::Address::generate(&env);

    let expected_protocol_fee = (gross_cost * PROTOCOL_BPS as i128) / 10_000;
    let expected_creator_fee = (gross_cost * CREATOR_BPS as i128) / 10_000;

    let quote1 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer1, &quote1.total_amount, &None);

    let quote2 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer2, &quote2.total_amount, &None);

    let treasury_balance = client.get_treasury_balance();
    let creator_balance = client.get_creator_fee_balance(&creator);

    assert_eq!(
        treasury_balance,
        expected_protocol_fee * 2,
        "treasury accumulated exactly 2x 5% protocol fee after 2 buys"
    );
    assert_eq!(
        creator_balance,
        expected_creator_fee * 2,
        "creator recipient accumulated exactly 2x 2% creator fee after 2 buys"
    );
}

#[test]
fn test_buy_fee_split_no_stroop_rounding_error_at_odd_gross_cost() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    let gross_cost: i128 = 1_234_567; // Odd stroop amount to verify integer arithmetic precision

    client.set_key_price(&admin, &gross_cost);
    client.set_protocol_admin(&admin, &admin);
    client.set_fee_config(&admin, &CREATOR_BPS, &PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "carol");
    let buyer = soroban_sdk::Address::generate(&env);

    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let treasury_delta = client.get_treasury_balance();
    let creator_delta = client.get_creator_fee_balance(&creator);

    let expected_treasury = (gross_cost * PROTOCOL_BPS as i128) / 10_000;
    let expected_creator = (gross_cost * CREATOR_BPS as i128) / 10_000;

    assert_eq!(treasury_delta, expected_treasury);
    assert_eq!(creator_delta, expected_creator);
    assert_eq!(
        gross_cost + treasury_delta + creator_delta,
        quote.total_amount,
        "sum of credits strictly matches total payment even for odd stroop amounts"
    );
}
