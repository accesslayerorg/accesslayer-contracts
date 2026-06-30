//! Regression coverage for quote outputs across supply edge transitions.

mod contract_test_env;

use contract_test_env::{
    compute_expected_buy_price, register_creator_keys, register_test_creator, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::CurvePreset;
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_buy_quote_deterministic_across_zero_supply_transition() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100_i128, 9000, 1000);

    let creator = register_test_creator(&env, &client, "edge1");
    let buyer = Address::generate(&env);

    let q_before = client.get_buy_quote(&creator);
    assert!(
        q_before.total_amount >= 0,
        "buy quote total must be bounded"
    );
    client.buy_key(&creator, &buyer, &q_before.total_amount, &None);

    // Transition back to zero supply.
    client.sell_key(&creator, &buyer, &None);
    assert_eq!(client.get_total_key_supply(&creator), 0);

    let q_after = client.get_buy_quote(&creator);
    assert_eq!(q_before.price, q_after.price);
    assert_eq!(q_before.creator_fee, q_after.creator_fee);
    assert_eq!(q_before.protocol_fee, q_after.protocol_fee);
    assert_eq!(q_before.total_amount, q_after.total_amount);
}

#[test]
fn test_sell_quote_zero_supply_boundary_is_rejected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100_i128, 9000, 1000);

    let creator = register_test_creator(&env, &client, "edge2");
    let holder = Address::generate(&env);

    let err = client.try_get_sell_quote(&creator, &holder);
    assert!(
        err.is_err(),
        "zero-supply holder must not receive sell quote"
    );
}

#[test]
fn test_buy_quote_recomputed_after_sell_reduces_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 250_i128;
    set_pricing_and_fees(&env, &client, key_price, 9000, 1000);

    let creator = register_test_creator(&env, &client, "edge3");
    let holder = Address::generate(&env);
    let quote_at_zero = client.get_buy_quote(&creator);
    client.buy_key(&creator, &holder, &quote_at_zero.total_amount, &None);
    client.buy_key(&creator, &holder, &quote_at_zero.total_amount, &None);

    let supply_before_sell = client.get_total_key_supply(&creator);
    let quote_before_sell = client.get_buy_quote(&creator);
    assert_eq!(supply_before_sell, 2);
    assert_eq!(
        quote_before_sell.price,
        compute_expected_buy_price(supply_before_sell, CurvePreset::Linear),
        "pre-sell quote must match bonding curve helper"
    );

    client.sell_key(&creator, &holder, &None);

    let supply_after_sell = client.get_total_key_supply(&creator);
    let quote_after_sell = client.get_buy_quote(&creator);
    let expected_price_after_sell =
        compute_expected_buy_price(supply_after_sell, CurvePreset::Linear);
    let expected_price_delta = expected_price_after_sell
        - compute_expected_buy_price(supply_before_sell, CurvePreset::Linear);

    assert_eq!(supply_after_sell, supply_before_sell - 1);
    assert_eq!(
        quote_after_sell.price, expected_price_after_sell,
        "post-sell quote must be recomputed from the reduced supply"
    );
    assert_eq!(
        quote_after_sell.price - quote_before_sell.price,
        expected_price_delta,
        "quote movement after sell must match the bonding curve helper"
    );
}
