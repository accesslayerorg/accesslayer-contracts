//! Tests for fee split sum invariants on sell path.
//!
//! These tests verify that fee split sums are conserved during sell flow:
//! - creator_fee + protocol_fee + total_amount == price (for sell quotes)
//! - creator_fee + protocol_fee == price - total_amount (seller net)
//! - Boundary and nominal cases with deterministic assertions

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::fee;
use creator_keys::CreatorKeysContractClient;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup_holder_with_key(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    key_price: i128,
) -> Address {
    let holder = Address::generate(env);
    let buy_quote = client.get_buy_quote(creator);
    assert_eq!(buy_quote.price, key_price);
    client.buy_key(creator, &holder, &buy_quote.total_amount);
    holder
}

fn assert_sell_fee_split_invariant(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    holder: &Address,
    key_price: i128,
    creator_bps: u32,
    protocol_bps: u32,
) {
    let quote = client.get_sell_quote(creator, holder);

    // Verify fee split sum invariants
    assert_eq!(
        quote.creator_fee + quote.protocol_fee,
        key_price - quote.total_amount,
        "creator_fee + protocol_fee should equal price - total_amount (seller net)"
    );

    // Verify total conservation
    assert_eq!(
        quote.creator_fee + quote.protocol_fee + quote.total_amount,
        key_price,
        "creator_fee + protocol_fee + total_amount should equal price"
    );

    // Verify fee calculation matches expected split
    let (expected_creator, expected_protocol) =
        fee::compute_fee_split(key_price, creator_bps, protocol_bps);
    assert_eq!(
        quote.creator_fee, expected_creator,
        "creator_fee should match expected calculation"
    );
    assert_eq!(
        quote.protocol_fee, expected_protocol,
        "protocol_fee should match expected calculation"
    );
}

#[test]
fn sell_fee_split_invariant_90_10_nominal_case() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 1000_i128;
    set_pricing_and_fees(&env, &client, key_price, 9000, 1000);
    let creator = register_test_creator(&env, &client, "creator1");
    let holder = setup_holder_with_key(&env, &client, &creator, key_price);

    assert_sell_fee_split_invariant(&client, &creator, &holder, key_price, 9000, 1000);
}

#[test]
fn sell_fee_split_invariant_50_50_equal_split() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 100_i128;
    set_pricing_and_fees(&env, &client, key_price, 5000, 5000);
    let creator = register_test_creator(&env, &client, "creator2");
    let holder = setup_holder_with_key(&env, &client, &creator, key_price);

    assert_sell_fee_split_invariant(&client, &creator, &holder, key_price, 5000, 5000);
}

#[test]
fn sell_fee_split_invariant_100_creator_zero_protocol() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 500_i128;
    set_pricing_and_fees(&env, &client, key_price, 10000, 0);
    let creator = register_test_creator(&env, &client, "creator3");
    let holder = setup_holder_with_key(&env, &client, &creator, key_price);

    assert_sell_fee_split_invariant(&client, &creator, &holder, key_price, 10000, 0);
}

#[test]
fn sell_fee_split_invariant_max_protocol_50_percent() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 200_i128;
    set_pricing_and_fees(&env, &client, key_price, 5000, 5000);
    let creator = register_test_creator(&env, &client, "creator4");
    let holder = setup_holder_with_key(&env, &client, &creator, key_price);

    assert_sell_fee_split_invariant(&client, &creator, &holder, key_price, 5000, 5000);
}

#[test]
fn sell_fee_split_invariant_boundary_price_one() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 1_i128;
    set_pricing_and_fees(&env, &client, key_price, 9000, 1000);
    let creator = register_test_creator(&env, &client, "creator5");
    let holder = setup_holder_with_key(&env, &client, &creator, key_price);

    assert_sell_fee_split_invariant(&client, &creator, &holder, key_price, 9000, 1000);

    // Additional boundary verification for price=1
    let quote = client.get_sell_quote(&creator, &holder);
    assert_eq!(quote.price, 1);
    assert_eq!(quote.total_amount, 0); // Seller gets nothing due to fees
    assert_eq!(quote.creator_fee + quote.protocol_fee, 1); // All goes to fees
}

#[test]
fn sell_fee_split_invariant_boundary_price_two() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 2_i128;
    set_pricing_and_fees(&env, &client, key_price, 5000, 5000);
    let creator = register_test_creator(&env, &client, "creator6");
    let holder = setup_holder_with_key(&env, &client, &creator, key_price);

    assert_sell_fee_split_invariant(&client, &creator, &holder, key_price, 5000, 5000);

    // Verify specific boundary behavior
    let quote = client.get_sell_quote(&creator, &holder);
    assert_eq!(quote.price, 2);
    // floor(2 * 5000 / 10000) = 1 protocol, 1 creator, 0 net
    assert_eq!(quote.protocol_fee, 1);
    assert_eq!(quote.creator_fee, 1);
    assert_eq!(quote.total_amount, 0);
}

#[test]
fn sell_fee_split_invariant_boundary_odd_price_999() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 999_i128;
    set_pricing_and_fees(&env, &client, key_price, 9000, 1000);
    let creator = register_test_creator(&env, &client, "creator7");
    let holder = setup_holder_with_key(&env, &client, &creator, key_price);

    assert_sell_fee_split_invariant(&client, &creator, &holder, key_price, 9000, 1000);

    // Verify remainder goes to creator
    let quote = client.get_sell_quote(&creator, &holder);
    // 999 * 1000 / 10000 = 99.9 -> 99 protocol, 900 creator
    assert_eq!(quote.protocol_fee, 99);
    assert_eq!(quote.creator_fee, 900);
    assert_eq!(quote.total_amount, 0);
}

#[test]
fn sell_fee_split_invariant_large_amount() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 1_000_000_000_i128;
    set_pricing_and_fees(&env, &client, key_price, 9000, 1000);
    let creator = register_test_creator(&env, &client, "creator8");
    let holder = setup_holder_with_key(&env, &client, &creator, key_price);

    assert_sell_fee_split_invariant(&client, &creator, &holder, key_price, 9000, 1000);
}

#[test]
fn sell_fee_split_invariant_across_multiple_fee_configs() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 1000_i128;

    let test_cases = [
        (10000, 0),   // 100% creator
        (9000, 1000), // 90/10 split
        (7500, 2500), // 75/25 split
        (5000, 5000), // 50/50 split (max protocol)
    ];

    for (i, (creator_bps, protocol_bps)) in test_cases.iter().enumerate() {
        if creator_bps + protocol_bps != 10000 {
            continue; // Skip invalid configs
        }

        let creator = register_test_creator(&env, &client, &format!("creator{}", i));
        set_pricing_and_fees(&env, &client, key_price, *creator_bps, *protocol_bps);
        let holder = setup_holder_with_key(&env, &client, &creator, key_price);

        assert_sell_fee_split_invariant(
            &client,
            &creator,
            &holder,
            key_price,
            *creator_bps,
            *protocol_bps,
        );
    }
}

#[test]
fn sell_fee_split_invariant_across_price_range() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let test_prices = vec![1, 2, 3, 10, 99, 100, 101, 999, 1000, 10000];

    for (i, price) in test_prices.iter().enumerate() {
        let creator = register_test_creator(&env, &client, &format!("creator{}", i));
        set_pricing_and_fees(&env, &client, *price, 9000, 1000);
        let holder = setup_holder_with_key(&env, &client, &creator, *price);

        assert_sell_fee_split_invariant(&client, &creator, &holder, *price, 9000, 1000);
    }
}

#[test]
fn sell_fee_split_invariant_deterministic_assertions() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 123_i128;
    set_pricing_and_fees(&env, &client, key_price, 8750, 1250); // 87.5/12.5 split
    let creator = register_test_creator(&env, &client, "creator_det");
    let holder = setup_holder_with_key(&env, &client, &creator, key_price);

    let quote = client.get_sell_quote(&creator, &holder);

    // Deterministic assertions with exact expected values
    // floor(123 * 1250 / 10000) = 15 protocol, 108 creator, 0 net
    assert_eq!(quote.price, 123, "price should be exactly 123");
    assert_eq!(quote.protocol_fee, 15, "protocol_fee should be exactly 15");
    assert_eq!(quote.creator_fee, 108, "creator_fee should be exactly 108");
    assert_eq!(quote.total_amount, 0, "total_amount should be exactly 0");

    // Verify invariants with exact arithmetic
    assert_eq!(
        quote.creator_fee + quote.protocol_fee,
        123,
        "fees should sum to price"
    );
    assert_eq!(
        quote.creator_fee + quote.protocol_fee + quote.total_amount,
        123,
        "all components should sum to price"
    );
}

#[test]
fn sell_fee_split_invariant_zero_net_boundary() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Test cases where seller gets zero net due to fees
    let zero_net_cases = [
        (1, 9000, 1000), // Price 1, 90/10 split
        (2, 5000, 5000), // Price 2, 50/50 split
        (3, 7000, 3000), // Price 3, 70/30 split
        (10, 9500, 500), // Price 10, 95/5 split
    ];

    for (i, (price, creator_bps, protocol_bps)) in zero_net_cases.iter().enumerate() {
        let creator = register_test_creator(&env, &client, &format!("creator{}", i));
        set_pricing_and_fees(&env, &client, *price, *creator_bps, *protocol_bps);
        let holder = setup_holder_with_key(&env, &client, &creator, *price);

        let quote = client.get_sell_quote(&creator, &holder);
        assert_eq!(
            quote.total_amount, 0,
            "total_amount should be zero for price {}",
            price
        );
        assert_eq!(
            quote.creator_fee + quote.protocol_fee,
            *price,
            "fees should equal price for price {}",
            price
        );
        assert_sell_fee_split_invariant(
            &client,
            &creator,
            &holder,
            *price,
            *creator_bps,
            *protocol_bps,
        );
    }
}
