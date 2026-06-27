//! Regression: sell with min_proceeds == exact quote must succeed (not revert).

mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, test_env_with_auths,
    set_key_price_for_tests, set_protocol_fee_bps, compute_expected_buy_price,
};
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn sell_succeeds_when_proceeds_equal_min_proceeds_exactly() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client);
    let buyer = Address::generate(&env);

    // Buy a key first
    let buy_price = compute_expected_buy_price(0, 100i128);
    client.buy_key(&creator, &buyer, &buy_price, &None);

    // Get exact sell quote
    let exact_proceeds = client.get_sell_quote(&creator, &1i128);

    // Sell with min_proceeds == exact quote: must NOT revert
    let result = client.try_sell_key(&creator, &buyer, &1i128, &exact_proceeds);
    assert!(
        result.is_ok(),
        "Sell must succeed when min_proceeds equals exact proceeds, got: {:?}", result
    );
}

#[test]
fn sell_reverts_when_proceeds_below_min() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client);
    let buyer = Address::generate(&env);
    let buy_price = compute_expected_buy_price(0, 100i128);
    client.buy_key(&creator, &buyer, &buy_price, &None);
    let exact_proceeds = client.get_sell_quote(&creator, &1i128);

    // min_proceeds = exact + 1: must revert (proceeds < min)
    let result = client.try_sell_key(&creator, &buyer, &1i128, &(exact_proceeds + 1));
    assert!(result.is_err(), "Sell must revert when min_proceeds exceeds actual proceeds");
}
