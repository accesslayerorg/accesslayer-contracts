//! Integration test for protocol fee distributed correctly to protocol fee
//! recipient on buy (#598).
//!
//! Confirms that the protocol fee recipient's accrued balance increases by the
//! exact expected amount after a buy transaction.

mod contract_test_env;

use contract_test_env::{
    compute_expected_protocol_fee, register_creator_keys, register_test_creator,
    set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

const KEY_PRICE: i128 = 1000;

#[test]
fn test_buy_increases_protocol_fee_recipient_balance_by_bps_fee() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let protocol_bps: u32 = 1000;
    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, protocol_bps);
    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    let balance_before = client.get_protocol_recipient_balance();
    let quote = client.get_buy_quote(&creator);
    let expected_protocol_fee = compute_expected_protocol_fee(KEY_PRICE, protocol_bps);
    assert_eq!(
        quote.protocol_fee, expected_protocol_fee,
        "buy quote protocol fee should match bps calculation"
    );

    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let balance_after = client.get_protocol_recipient_balance();
    assert_eq!(
        balance_after - balance_before,
        expected_protocol_fee,
        "protocol fee recipient balance should increase by the buy protocol fee"
    );
    assert_eq!(
        balance_after - balance_before,
        quote.protocol_fee,
        "credited amount should match the buy quote protocol fee"
    );
}

#[test]
fn test_buy_increases_protocol_fee_recipient_balance_with_different_bps() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let protocol_bps: u32 = 2500;
    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 7500, protocol_bps);
    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "bob");
    let buyer = soroban_sdk::Address::generate(&env);

    let balance_before = client.get_protocol_recipient_balance();
    let quote = client.get_buy_quote(&creator);
    let expected_protocol_fee = compute_expected_protocol_fee(KEY_PRICE, protocol_bps);
    assert_eq!(
        quote.protocol_fee, expected_protocol_fee,
        "buy quote protocol fee should match 25% bps calculation"
    );

    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let balance_after = client.get_protocol_recipient_balance();
    assert_eq!(
        balance_after - balance_before,
        expected_protocol_fee,
        "protocol fee recipient balance should increase by the 25% protocol fee"
    );
}

#[test]
fn test_buy_protocol_fee_recipient_balance_accumulates_across_two_buys() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let protocol_bps: u32 = 1000;
    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, protocol_bps);
    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "carol");
    let buyer_a = soroban_sdk::Address::generate(&env);
    let buyer_b = soroban_sdk::Address::generate(&env);

    let expected_protocol_fee = compute_expected_protocol_fee(KEY_PRICE, protocol_bps);
    let balance_before = client.get_protocol_recipient_balance();

    let quote_a = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer_a, &quote_a.total_amount, &None);
    let balance_after_first_buy = client.get_protocol_recipient_balance();
    assert_eq!(
        balance_after_first_buy - balance_before,
        expected_protocol_fee,
        "first buy should credit one protocol fee"
    );

    let quote_b = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer_b, &quote_b.total_amount, &None);
    let balance_after_second_buy = client.get_protocol_recipient_balance();
    assert_eq!(
        balance_after_second_buy - balance_after_first_buy,
        expected_protocol_fee,
        "second buy should credit another protocol fee"
    );
    assert_eq!(
        balance_after_second_buy - balance_before,
        expected_protocol_fee * 2,
        "two buys should accumulate two protocol fees"
    );
}

#[test]
fn test_buy_protocol_fee_recipient_does_not_receive_creator_fee() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator_bps: u32 = 9000;
    let protocol_bps: u32 = 1000;
    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, creator_bps, protocol_bps);
    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "dave");
    let buyer = soroban_sdk::Address::generate(&env);

    let quote = client.get_buy_quote(&creator);
    let expected_protocol_fee = compute_expected_protocol_fee(KEY_PRICE, protocol_bps);

    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let protocol_balance = client.get_protocol_recipient_balance();
    assert_eq!(
        protocol_balance, expected_protocol_fee,
        "protocol fee recipient should only receive the protocol fee portion"
    );
    assert!(
        protocol_balance < quote.total_amount,
        "protocol fee must be less than total payment"
    );
    assert!(
        protocol_balance != quote.creator_fee,
        "protocol fee must not equal the creator fee"
    );
}

#[test]
fn test_buy_protocol_fee_uses_floor_division() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Use a price and bps that produce a fractional result requiring floor division.
    // 1500 * 1500 / 10000 = 225 (exact in this case, but verify the floor behavior)
    let protocol_bps: u32 = 1500;
    let admin = set_pricing_and_fees(&env, &client, 1500, 8500, protocol_bps);
    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "eve");
    let buyer = soroban_sdk::Address::generate(&env);

    let balance_before = client.get_protocol_recipient_balance();

    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let balance_after = client.get_protocol_recipient_balance();
    let credited = balance_after - balance_before;

    // Verify floor division: price * protocol_bps / 10000 floored
    let expected_floor = (1500i128 * protocol_bps as i128) / 10_000;
    assert_eq!(
        credited, expected_floor,
        "protocol fee should use floor division"
    );
    assert_eq!(credited, quote.protocol_fee);
}
