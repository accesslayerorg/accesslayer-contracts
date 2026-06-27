//! Unit tests: transfer must update holder count correctly.

mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, test_env_with_auths,
    set_key_price_for_tests, set_protocol_fee_bps, compute_expected_buy_price,
};
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn transfer_removes_sender_from_holders_when_balance_reaches_zero() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Buy 1 key for sender
    let price = compute_expected_buy_price(0, 100i128);
    client.buy_key(&creator, &sender, &price, &None);
    let holders_before = client.get_holder_count(&creator);
    assert!(holders_before > 0);

    // Transfer all keys to recipient
    client.transfer_key(&creator, &sender, &recipient, &1i128);

    // Sender balance should be zero
    assert_eq!(client.get_key_balance(&creator, &sender), 0);

    // Holder count should reflect sender removed, recipient added (net zero change)
    let holders_after = client.get_holder_count(&creator);
    assert_eq!(holders_before, holders_after,
        "Holder count unchanged: sender removed, recipient added");
}

#[test]
fn transfer_adds_recipient_to_holders_on_first_receipt() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client);
    let sender = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    // Buy 2 keys for sender
    let price = compute_expected_buy_price(0, 100i128);
    client.buy_key(&creator, &sender, &price, &None);
    let price2 = compute_expected_buy_price(1, 100i128);
    client.buy_key(&creator, &sender, &price2, &None);
    let holders_before = client.get_holder_count(&creator);

    // Transfer 1 key to new recipient (first time they receive)
    client.transfer_key(&creator, &sender, &new_recipient, &1i128);

    // Holder count should increase by 1
    let holders_after = client.get_holder_count(&creator);
    assert_eq!(holders_after, holders_before + 1,
        "New recipient added to holder count on first receipt");
    assert_eq!(client.get_key_balance(&creator, &new_recipient), 1);
}
