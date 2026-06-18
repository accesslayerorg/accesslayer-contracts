//! Regression test for protocol fee recipient balance after sell (#296).
//!
//! Sell event and quote tests validate fee fields, but they do not confirm the
//! protocol fee recipient actually receives the fee amount. This test records the
//! recipient balance before a sell and asserts it increases by the bps-derived fee.

mod contract_test_env;

use contract_test_env::{
    compute_expected_protocol_fee, register_creator_keys, register_test_creator,
    set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

#[test]
fn test_sell_increases_protocol_fee_recipient_balance_by_bps_fee() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "alice");
    let holder = soroban_sdk::Address::generate(&env);

    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    let balance_before = client.get_protocol_recipient_balance();
    let quote = client.get_sell_quote(&creator, &holder);
    let expected_protocol_fee = compute_expected_protocol_fee(KEY_PRICE, PROTOCOL_BPS);
    assert_eq!(
        quote.protocol_fee, expected_protocol_fee,
        "sell quote protocol fee should match bps calculation"
    );

    client.sell_key(&creator, &holder, &None);

    let balance_after = client.get_protocol_recipient_balance();
    assert_eq!(
        balance_after - balance_before,
        expected_protocol_fee,
        "protocol fee recipient balance should increase by the sell protocol fee"
    );
    assert_eq!(
        balance_after - balance_before,
        quote.protocol_fee,
        "credited amount should match the sell quote protocol fee"
    );
}

#[test]
fn test_sell_protocol_fee_recipient_balance_accumulates_across_two_sells() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "bob");
    let holder = soroban_sdk::Address::generate(&env);

    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    let expected_protocol_fee = compute_expected_protocol_fee(KEY_PRICE, PROTOCOL_BPS);
    let balance_before = client.get_protocol_recipient_balance();

    client.sell_key(&creator, &holder, &None);
    let balance_after_first_sell = client.get_protocol_recipient_balance();
    assert_eq!(
        balance_after_first_sell - balance_before,
        expected_protocol_fee,
        "first sell should credit one protocol fee"
    );

    client.sell_key(&creator, &holder, &None);
    let balance_after_second_sell = client.get_protocol_recipient_balance();
    assert_eq!(
        balance_after_second_sell - balance_after_first_sell,
        expected_protocol_fee,
        "second sell should credit another protocol fee"
    );
    assert_eq!(
        balance_after_second_sell - balance_before,
        expected_protocol_fee * 2,
        "two sells should accumulate two protocol fees"
    );
}
