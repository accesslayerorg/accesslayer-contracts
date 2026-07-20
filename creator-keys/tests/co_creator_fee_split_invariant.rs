//! Tests that co-creator fee splits preserve the full creator fee with no XLM lost.
//!
//! Validates the invariant: co_creator_amount + creator_recipient_amount == total_creator_fee
//! across different split percentages (30%, 50%, 10%).

mod contract_test_env;

use contract_test_env::{
    compute_expected_creator_fee, register_creator_keys, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{CoCreatorConfig, RegisterCreatorParams};
use soroban_sdk::{Address, Env, String};

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

/// Helper to register a creator with a co-creator configuration.
fn register_creator_with_co_creator(
    env: &Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    handle: &str,
    share_bps: u32,
) -> (Address, Address) {
    let creator = Address::generate(env);
    let co_creator = Address::generate(env);
    let config = CoCreatorConfig {
        address: co_creator.clone(),
        share_bps,
    };

    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &None,
        &None,
        &Some(config),
        &None,
    );

    (creator, co_creator)
}

/// Helper to verify the co-creator fee split invariant for a given split percentage.
fn verify_fee_split_invariant(share_bps: u32, test_name: &str) {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let (creator, co_creator) =
        register_creator_with_co_creator(&env, &client, test_name, share_bps);
    let buyer = Address::generate(&env);

    // Capture initial balances
    let creator_balance_before = client.get_creator_fee_balance(&creator);
    let co_creator_balance_before = client.get_co_creator_fee_balance(&creator, &co_creator);

    // Execute buy
    let quote = client.get_buy_quote(&creator);
    let expected_total_creator_fee =
        compute_expected_creator_fee(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    // Capture balance increases
    let creator_balance_after = client.get_creator_fee_balance(&creator);
    let co_creator_balance_after = client.get_co_creator_fee_balance(&creator, &co_creator);

    let creator_increase = creator_balance_after - creator_balance_before;
    let co_creator_increase = co_creator_balance_after - co_creator_balance_before;

    // Verify the invariant: sum equals total creator fee
    assert_eq!(
        creator_increase + co_creator_increase,
        expected_total_creator_fee,
        "Fee split invariant violated for {share_bps} bps: creator={creator_increase}, co_creator={co_creator_increase}, expected_total={expected_total_creator_fee}"
    );

    // Verify quote matches the total creator fee
    assert_eq!(
        quote.creator_fee, expected_total_creator_fee,
        "Quote creator fee mismatch for {share_bps} bps"
    );

    // Verify no party receives more than their share
    let expected_co_creator = (expected_total_creator_fee * share_bps as i128) / 10_000;
    let expected_creator_recipient = expected_total_creator_fee - expected_co_creator;

    assert_eq!(
        creator_increase, expected_creator_recipient,
        "Creator recipient received incorrect amount for {share_bps} bps"
    );
    assert_eq!(
        co_creator_increase, expected_co_creator,
        "Co-creator received incorrect amount for {share_bps} bps"
    );
}

#[test]
fn test_co_creator_fee_split_30_percent_no_xlm_lost() {
    verify_fee_split_invariant(3000, "creator_30");
}

#[test]
fn test_co_creator_fee_split_50_percent_no_xlm_lost() {
    verify_fee_split_invariant(5000, "creator_50");
}

#[test]
fn test_co_creator_fee_split_10_percent_no_xlm_lost() {
    verify_fee_split_invariant(1000, "creator_10");
}

#[test]
fn test_co_creator_fee_split_invariant_across_multiple_trades() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    // Test with 30% share across multiple trades
    let share_bps = 3000_u32;
    let (creator, co_creator) =
        register_creator_with_co_creator(&env, &client, "multi_trade", share_bps);

    let expected_total_creator_fee =
        compute_expected_creator_fee(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    // Execute 5 buy operations
    let trade_count = 5;
    for i in 0..trade_count {
        let buyer = Address::generate(&env);
        let creator_balance_before = client.get_creator_fee_balance(&creator);
        let co_creator_balance_before = client.get_co_creator_fee_balance(&creator, &co_creator);

        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &buyer, &quote.total_amount, &None);

        let creator_increase = client.get_creator_fee_balance(&creator) - creator_balance_before;
        let co_creator_increase =
            client.get_co_creator_fee_balance(&creator, &co_creator) - co_creator_balance_before;

        // Verify invariant holds for each trade
        assert_eq!(
            creator_increase + co_creator_increase,
            expected_total_creator_fee,
            "Fee split invariant violated on trade {i}: creator={creator_increase}, co_creator={co_creator_increase}"
        );
    }

    // Verify cumulative balances also match expected totals
    let total_creator_balance = client.get_creator_fee_balance(&creator);
    let total_co_creator_balance = client.get_co_creator_fee_balance(&creator, &co_creator);

    assert_eq!(
        total_creator_balance + total_co_creator_balance,
        expected_total_creator_fee * trade_count as i128,
        "Cumulative fee split invariant violated"
    );
}

#[test]
fn test_co_creator_fee_split_invariant_on_sell() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    // Test with 30% share on sell operations
    let share_bps = 3000_u32;
    let (creator, co_creator) =
        register_creator_with_co_creator(&env, &client, "sell_test", share_bps);
    let trader = Address::generate(&env);

    // Buy a key first
    let buy_quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &trader, &buy_quote.total_amount, &None);

    // Capture balances before sell
    let creator_balance_before = client.get_creator_fee_balance(&creator);
    let co_creator_balance_before = client.get_co_creator_fee_balance(&creator, &co_creator);

    // Execute sell
    let sell_quote = client.get_sell_quote(&creator, &trader);
    client.sell_key(&creator, &trader, &None);

    // Capture balance increases from sell
    let creator_increase = client.get_creator_fee_balance(&creator) - creator_balance_before;
    let co_creator_increase =
        client.get_co_creator_fee_balance(&creator, &co_creator) - co_creator_balance_before;

    // Verify the invariant: sum equals total creator fee from sell
    assert_eq!(
        creator_increase + co_creator_increase,
        sell_quote.creator_fee,
        "Sell fee split invariant violated: creator={creator_increase}, co_creator={co_creator_increase}, expected_total={}", 
        sell_quote.creator_fee
    );

    // Verify individual shares are correct
    let expected_co_creator = (sell_quote.creator_fee * share_bps as i128) / 10_000;
    let expected_creator_recipient = sell_quote.creator_fee - expected_co_creator;

    assert_eq!(creator_increase, expected_creator_recipient);
    assert_eq!(co_creator_increase, expected_co_creator);
}

#[test]
fn test_co_creator_fee_split_boundary_cases() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    // Test minimum valid share (1 bps = 0.01%)
    verify_fee_split_invariant(1, "creator_min");

    // Test maximum valid share (9999 bps = 99.99%)
    verify_fee_split_invariant(9999, "creator_max");
}

#[test]
fn test_no_party_receives_more_than_their_share() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let test_cases = vec![
        (1000, "10_percent"),
        (3000, "30_percent"),
        (5000, "50_percent"),
        (7000, "70_percent"),
        (9000, "90_percent"),
    ];

    for (share_bps, test_name) in test_cases {
        let (creator, co_creator) =
            register_creator_with_co_creator(&env, &client, test_name, share_bps);
        let buyer = Address::generate(&env);

        let creator_balance_before = client.get_creator_fee_balance(&creator);
        let co_creator_balance_before = client.get_co_creator_fee_balance(&creator, &co_creator);

        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &buyer, &quote.total_amount, &None);

        let creator_increase = client.get_creator_fee_balance(&creator) - creator_balance_before;
        let co_creator_increase =
            client.get_co_creator_fee_balance(&creator, &co_creator) - co_creator_balance_before;

        // Calculate expected shares
        let expected_co_creator = (quote.creator_fee * share_bps as i128) / 10_000;
        let expected_creator_recipient = quote.creator_fee - expected_co_creator;

        // Verify neither party receives more than their share
        assert!(
            creator_increase <= expected_creator_recipient,
            "Creator recipient received more than their share for {share_bps} bps: got {creator_increase}, expected {expected_creator_recipient}"
        );
        assert!(
            co_creator_increase <= expected_co_creator,
            "Co-creator received more than their share for {share_bps} bps: got {co_creator_increase}, expected {expected_co_creator}"
        );

        // Verify exact amounts (should be equal, not just less-than-or-equal)
        assert_eq!(creator_increase, expected_creator_recipient);
        assert_eq!(co_creator_increase, expected_co_creator);
    }
}
