//! Tests for dividend distribution skipping wallets with zero key balance (#445).
//!
//! A wallet that holds zero keys for a creator should receive no dividend share
//! even if it previously held keys and is still tracked in storage.

mod contract_test_env;

use contract_test_env::{
    compute_expected_holder_dividend, distribute_test_dividend, register_creator_keys,
    register_test_creator, set_pricing_and_fees, test_env_with_auths, DEFAULT_PROTOCOL_BPS,
};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

#[test]
fn test_zero_balance_wallet_receives_no_dividend() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100, 9000, DEFAULT_PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    // Wallet A buys 2 keys
    let wallet_a = Address::generate(&env);
    client.buy_key(&creator, &wallet_a, &100, &None);
    client.buy_key(&creator, &wallet_a, &100, &None);

    // Wallet B buys 1 key then sells it all (zero balance)
    let wallet_b = Address::generate(&env);
    client.buy_key(&creator, &wallet_b, &100, &None);
    client.sell_key(&creator, &wallet_b, &None);

    // Wallet B now has zero balance, wallet A has 2 keys
    assert_eq!(client.get_key_balance(&creator, &wallet_b), 0);
    assert_eq!(client.get_key_balance(&creator, &wallet_a), 2);

    // Distribute dividend
    let distributor = Address::generate(&env);
    let amount = 10_000i128;
    distribute_test_dividend(&client, &creator, &distributor, amount);

    // Zero-balance wallet has zero claimable dividend
    assert_eq!(
        client.get_claimable_dividend(&creator, &wallet_b),
        0,
        "zero-balance wallet should receive no dividend"
    );

    // Positive-balance wallet receives full distribution (all keys)
    let expected = compute_expected_holder_dividend(amount, 2, 2, DEFAULT_PROTOCOL_BPS);
    assert_eq!(
        client.get_claimable_dividend(&creator, &wallet_a),
        expected,
        "positive-balance wallet should receive the full distribution"
    );

    // Total claimable equals distributed amount minus protocol fee
    let total_claimable = client.get_claimable_dividend(&creator, &wallet_a)
        + client.get_claimable_dividend(&creator, &wallet_b);
    let protocol_fee = (amount * DEFAULT_PROTOCOL_BPS as i128) / 10_000;
    assert_eq!(
        total_claimable,
        amount - protocol_fee,
        "total claimable should equal distributed amount minus protocol fee"
    );
}

#[test]
fn test_zero_balance_wallet_after_partial_sell() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100, 9000, DEFAULT_PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    // Wallet A buys 3 keys
    let wallet_a = Address::generate(&env);
    client.buy_key(&creator, &wallet_a, &100, &None);
    client.buy_key(&creator, &wallet_a, &100, &None);
    client.buy_key(&creator, &wallet_a, &100, &None);

    // Wallet B buys 2 keys then sells all
    let wallet_b = Address::generate(&env);
    client.buy_key(&creator, &wallet_b, &100, &None);
    client.buy_key(&creator, &wallet_b, &100, &None);
    client.sell_key(&creator, &wallet_b, &None);
    client.sell_key(&creator, &wallet_b, &None);

    assert_eq!(client.get_key_balance(&creator, &wallet_b), 0);

    // Distribute dividend
    let distributor = Address::generate(&env);
    let amount = 30_000i128;
    distribute_test_dividend(&client, &creator, &distributor, amount);

    // Zero-balance wallet gets nothing
    assert_eq!(
        client.get_claimable_dividend(&creator, &wallet_b),
        0,
        "zero-balance wallet should have zero claimable dividend"
    );

    // Wallet A (all 3 keys) gets full share
    let expected = compute_expected_holder_dividend(amount, 3, 3, DEFAULT_PROTOCOL_BPS);
    assert_eq!(
        client.get_claimable_dividend(&creator, &wallet_a),
        expected,
        "wallet with all supply should receive full distribution"
    );
}

#[test]
fn test_two_zero_balance_wallets_with_one_holder() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100, 9000, DEFAULT_PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");

    // Three wallets buy keys, two sell all
    let wallet_a = Address::generate(&env);
    let wallet_b = Address::generate(&env);
    let wallet_c = Address::generate(&env);

    client.buy_key(&creator, &wallet_a, &100, &None);
    client.buy_key(&creator, &wallet_b, &100, &None);
    client.buy_key(&creator, &wallet_c, &100, &None);

    // Wallets B and C sell all keys
    client.sell_key(&creator, &wallet_b, &None);
    client.sell_key(&creator, &wallet_c, &None);

    assert_eq!(client.get_key_balance(&creator, &wallet_b), 0);
    assert_eq!(client.get_key_balance(&creator, &wallet_c), 0);
    assert_eq!(client.get_key_balance(&creator, &wallet_a), 1);

    // Distribute dividend
    let distributor = Address::generate(&env);
    let amount = 10_000i128;
    distribute_test_dividend(&client, &creator, &distributor, amount);

    // Zero-balance wallets get nothing
    assert_eq!(client.get_claimable_dividend(&creator, &wallet_b), 0);
    assert_eq!(client.get_claimable_dividend(&creator, &wallet_c), 0);

    // Wallet A gets full share
    let expected = compute_expected_holder_dividend(amount, 1, 1, DEFAULT_PROTOCOL_BPS);
    assert_eq!(client.get_claimable_dividend(&creator, &wallet_a), expected);
}
