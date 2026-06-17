//! Regression test: protocol fees accumulate correctly in the treasury across
//! multiple distinct buyer addresses.
//!
//! Single-buyer treasury tests leave a blind spot: the treasury balance could
//! theoretically be keyed to the buyer address rather than accumulating globally.
//! This test uses three distinct buyers to confirm cross-buyer accumulation.

mod contract_test_env;

use contract_test_env::{
    compute_expected_protocol_fee, register_creator_keys, register_test_creator,
    set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

#[test]
fn test_treasury_accumulates_protocol_fees_across_distinct_buyers() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "alice");

    let buyers = [
        soroban_sdk::Address::generate(&env),
        soroban_sdk::Address::generate(&env),
        soroban_sdk::Address::generate(&env),
    ];

    let expected_fee = compute_expected_protocol_fee(KEY_PRICE, PROTOCOL_BPS);
    let mut cumulative_fees: i128 = 0;

    for buyer in &buyers {
        let quote = client.get_buy_quote(&creator);
        assert_eq!(
            quote.protocol_fee, expected_fee,
            "buy quote protocol fee should match bps calculation"
        );

        let treasury_before = client.get_protocol_recipient_balance();

        client.buy_key(&creator, buyer, &quote.total_amount);

        let treasury_after = client.get_protocol_recipient_balance();
        assert_eq!(
            treasury_after - treasury_before,
            expected_fee,
            "treasury should increase by exactly one protocol fee per buy"
        );

        cumulative_fees = cumulative_fees.checked_add(expected_fee).unwrap();
    }

    assert_eq!(
        client.get_protocol_recipient_balance(),
        cumulative_fees,
        "final treasury balance must equal the sum of all individual protocol fees"
    );
}
