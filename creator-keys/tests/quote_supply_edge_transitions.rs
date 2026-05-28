//! Regression coverage for quote outputs across supply edge transitions.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
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
    client.buy_key(&creator, &buyer, &q_before.total_amount);

    // Transition back to zero supply.
    client.sell_key(&creator, &buyer);
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
