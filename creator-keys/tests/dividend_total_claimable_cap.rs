//! Regression test: after a dividend distribution the sum of all claimable
//! amounts across every holder must never exceed the distributed amount minus
//! the protocol fee. Covers unequal holder balances to catch proportional
//! split errors and rounding issues.

mod contract_test_env;

use contract_test_env::{
    assert_claimable, compute_expected_holder_dividend, distribute_test_dividend,
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
    DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use soroban_sdk::{testutils::Address as _, Address};

const KEY_PRICE: i128 = 100;

#[test]
fn test_sum_claimable_never_exceeds_distributed_minus_protocol_fee() {
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

    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);
    let holder_c = Address::generate(&env);

    // holder_a: 3 keys, holder_b: 2 keys, holder_c: 1 key → supply = 6
    for _ in 0..3 {
        client.buy_key(&creator, &holder_a, &KEY_PRICE, &None);
    }
    for _ in 0..2 {
        client.buy_key(&creator, &holder_b, &KEY_PRICE, &None);
    }
    client.buy_key(&creator, &holder_c, &KEY_PRICE, &None);

    let total_supply = client.get_total_key_supply(&creator);
    assert_eq!(total_supply, 6);

    let distribution_amount: i128 = 100_000;
    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, distribution_amount);

    let protocol_fee = (distribution_amount * DEFAULT_PROTOCOL_BPS as i128) / 10_000;
    let net_amount = distribution_amount - protocol_fee;

    let claimable_a = client.get_claimable_dividend(&creator, &holder_a);
    let claimable_b = client.get_claimable_dividend(&creator, &holder_b);
    let claimable_c = client.get_claimable_dividend(&creator, &holder_c);

    let total_claimable = claimable_a + claimable_b + claimable_c;

    assert!(
        total_claimable <= net_amount,
        "sum of claimable ({total_claimable}) must not exceed net distributed ({net_amount})"
    );

    let expected_a =
        compute_expected_holder_dividend(distribution_amount, 3, 6, DEFAULT_PROTOCOL_BPS);
    let expected_b =
        compute_expected_holder_dividend(distribution_amount, 2, 6, DEFAULT_PROTOCOL_BPS);
    let expected_c =
        compute_expected_holder_dividend(distribution_amount, 1, 6, DEFAULT_PROTOCOL_BPS);

    assert_claimable(&client, &creator, &holder_a, expected_a);
    assert_claimable(&client, &creator, &holder_b, expected_b);
    assert_claimable(&client, &creator, &holder_c, expected_c);
}

#[test]
fn test_rounding_dust_stays_within_contract() {
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

    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);
    let holder_c = Address::generate(&env);

    for _ in 0..3 {
        client.buy_key(&creator, &holder_a, &KEY_PRICE, &None);
    }
    for _ in 0..2 {
        client.buy_key(&creator, &holder_b, &KEY_PRICE, &None);
    }
    client.buy_key(&creator, &holder_c, &KEY_PRICE, &None);

    // Use an amount that doesn't divide evenly by 6 to trigger rounding.
    let distribution_amount: i128 = 99_999;
    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, distribution_amount);

    let protocol_fee = (distribution_amount * DEFAULT_PROTOCOL_BPS as i128) / 10_000;
    let net_amount = distribution_amount - protocol_fee;

    let claimable_a = client.get_claimable_dividend(&creator, &holder_a);
    let claimable_b = client.get_claimable_dividend(&creator, &holder_b);
    let claimable_c = client.get_claimable_dividend(&creator, &holder_c);

    let total_claimable = claimable_a + claimable_b + claimable_c;

    assert!(
        total_claimable <= net_amount,
        "rounding dust must stay within contract: sum={total_claimable} net={net_amount}"
    );
}
