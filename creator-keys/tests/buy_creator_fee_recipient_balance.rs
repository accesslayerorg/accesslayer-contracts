//! Unit test verifying the creator fee recipient balance increases by the correct
//! creator fee amount after a buy, not just via emitted fee fields.

mod contract_test_env;

use contract_test_env::{
    compute_expected_creator_fee, register_creator_keys, register_test_creator,
    set_pricing_and_fees, test_env_with_auths, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use soroban_sdk::testutils::Address as _;

const KEY_PRICE: i128 = 1000;

#[test]
fn test_buy_credits_creator_fee_recipient_balance_by_bps_amount() {
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
    let buyer = soroban_sdk::Address::generate(&env);

    let quote = client.get_buy_quote(&creator);
    let expected_creator_fee =
        compute_expected_creator_fee(KEY_PRICE, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS);

    assert_eq!(
        quote.creator_fee, expected_creator_fee,
        "fixture quote creator fee should match bps calculation"
    );
    assert_eq!(
        client.get_creator_fee_bps(&creator),
        DEFAULT_CREATOR_BPS,
        "fixture should use the configured creator bps"
    );

    let balance_before = client.get_creator_fee_balance(&creator);
    assert_eq!(balance_before, 0, "recipient balance should start at zero");

    client.buy_key(&creator, &buyer, &quote.total_amount);

    let balance_after = client.get_creator_fee_balance(&creator);
    assert_eq!(
        balance_after - balance_before,
        expected_creator_fee,
        "creator fee recipient balance should increase by the bps-derived creator fee"
    );
    assert_eq!(
        balance_after, quote.creator_fee,
        "accrued balance should match the buy quote creator fee"
    );
}
