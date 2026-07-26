//! Integration test verifying the creator fee recipient balance increases by the
//! correct creator fee amount after a buy, across two different fee BPS
//! configurations (Issue #595).

mod contract_test_env;

use contract_test_env::{
    compute_expected_creator_fee, register_creator_keys, register_test_creator,
    set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

const KEY_PRICE: i128 = 1000;

#[test]
fn test_buy_credits_creator_fee_recipient_balance_at_70_30_split() {
    let creator_bps = 7000u32;
    let protocol_bps = 3000u32;

    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, KEY_PRICE, creator_bps, protocol_bps);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    let quote = client.get_buy_quote(&creator);
    let expected_creator_fee = compute_expected_creator_fee(KEY_PRICE, creator_bps, protocol_bps);

    assert_eq!(
        quote.creator_fee, expected_creator_fee,
        "quote creator fee should match bps calculation for 70/30 split"
    );

    let balance_before = client.get_creator_fee_balance(&creator);
    assert_eq!(balance_before, 0, "recipient balance should start at zero");

    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let balance_after = client.get_creator_fee_balance(&creator);
    assert_eq!(
        balance_after - balance_before,
        expected_creator_fee,
        "creator fee balance should increase by the bps-derived creator fee (70/30 split)"
    );
    assert_eq!(
        balance_after, quote.creator_fee,
        "accrued balance should match the buy quote creator fee"
    );
}

#[test]
fn test_buy_credits_creator_fee_recipient_balance_at_50_50_split() {
    let creator_bps = 5000u32;
    let protocol_bps = 5000u32;

    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, KEY_PRICE, creator_bps, protocol_bps);
    let creator = register_test_creator(&env, &client, "bob");
    let buyer = soroban_sdk::Address::generate(&env);

    let quote = client.get_buy_quote(&creator);
    let expected_creator_fee = compute_expected_creator_fee(KEY_PRICE, creator_bps, protocol_bps);

    assert_eq!(
        quote.creator_fee, expected_creator_fee,
        "quote creator fee should match bps calculation for 50/50 split"
    );

    let balance_before = client.get_creator_fee_balance(&creator);
    assert_eq!(balance_before, 0, "recipient balance should start at zero");

    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let balance_after = client.get_creator_fee_balance(&creator);
    assert_eq!(
        balance_after - balance_before,
        expected_creator_fee,
        "creator fee balance should increase by the bps-derived creator fee (50/50 split)"
    );
    assert_eq!(
        balance_after, quote.creator_fee,
        "accrued balance should match the buy quote creator fee"
    );
}

#[test]
fn test_two_buys_accumulate_creator_fee_balance() {
    let creator_bps = 8000u32;
    let protocol_bps = 2000u32;

    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, KEY_PRICE, creator_bps, protocol_bps);
    let creator = register_test_creator(&env, &client, "carol");

    let expected_creator_fee = compute_expected_creator_fee(KEY_PRICE, creator_bps, protocol_bps);

    let buyer1 = soroban_sdk::Address::generate(&env);
    let buyer2 = soroban_sdk::Address::generate(&env);

    let q1 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer1, &q1.total_amount, &None);

    let q2 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer2, &q2.total_amount, &None);

    let balance_after = client.get_creator_fee_balance(&creator);
    assert_eq!(
        balance_after,
        expected_creator_fee * 2,
        "two buys should accrue twice the per-buy creator fee"
    );
}
