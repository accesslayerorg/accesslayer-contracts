//! Regression test: TTL must be extended after sell, not only after buy.

mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, test_env_with_auths,
    set_key_price_for_tests, set_protocol_fee_bps, compute_expected_buy_price,
};
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn ttl_extended_after_sell() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client);
    let buyer = Address::generate(&env);

    // Buy a key to establish initial state
    let buy_price = compute_expected_buy_price(0, 100i128);
    client.buy_key(&creator, &buyer, &buy_price, &None);

    // Record TTL after buy
    let ttl_after_buy = client.get_creator_ttl_remaining(&creator);
    assert!(ttl_after_buy > 0, "TTL must be positive after buy");

    // Advance ledger to consume some TTL
    env.ledger().with_mut(|li| { li.sequence_number += 1000; });

    // Sell the key
    let sell_proceeds = client.get_sell_quote(&creator, &1i128);
    client.sell_key(&creator, &buyer, &1i128, &sell_proceeds);

    // TTL must be extended after sell
    let ttl_after_sell = client.get_creator_ttl_remaining(&creator);
    assert!(
        ttl_after_sell > ttl_after_buy - 1000,
        "TTL must be extended after sell: ttl_after_sell={} ttl_after_buy={}",
        ttl_after_sell, ttl_after_buy
    );
}
