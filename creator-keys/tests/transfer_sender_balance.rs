//! Unit tests: transfer must decrement sender balance by exact transferred amount.

mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, test_env_with_auths,
    set_key_price_for_tests, set_protocol_fee_bps, compute_expected_buy_price,
};
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn transfer_decrements_sender_balance_by_exact_amount() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Buy 3 keys
    for i in 0..3i64 {
        let price = compute_expected_buy_price(i, 100i128);
        client.buy_key(&creator, &sender, &price, &None);
    }
    let balance_before = client.get_key_balance(&creator, &sender);
    assert_eq!(balance_before, 3);

    // Transfer 2 keys
    client.transfer_key(&creator, &sender, &recipient, &2i128);

    let balance_after = client.get_key_balance(&creator, &sender);
    assert_eq!(balance_after, 1,
        "Sender balance must be decremented by exactly 2");
    assert_eq!(client.get_key_balance(&creator, &recipient), 2);
}

#[test]
fn transfer_full_balance_leaves_sender_at_zero() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let price = compute_expected_buy_price(0, 100i128);
    client.buy_key(&creator, &sender, &price, &None);
    assert_eq!(client.get_key_balance(&creator, &sender), 1);

    client.transfer_key(&creator, &sender, &recipient, &1i128);

    assert_eq!(client.get_key_balance(&creator, &sender), 0,
        "Sender balance must be zero after full transfer");
}
