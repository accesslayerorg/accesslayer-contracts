//! Regression tests: `transfer_keys` must revert with `InsufficientBalance`
//! when the sender tries to transfer more keys than they hold.
//! Sender balance and total supply must remain unchanged after the revert.

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, register_creator_keys, register_test_creator, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

const KEY_PRICE: i128 = 100;

#[test]
fn test_transfer_exceeding_balance_reverts_with_insufficient_balance() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    assert_eq!(client.get_key_balance(&creator, &sender), 2);

    let result = client.try_transfer_keys(&creator, &sender, &recipient, &3);
    assert_eq!(
        result,
        Err(Ok(ContractError::InsufficientBalance)),
        "transfer exceeding balance must revert with InsufficientBalance"
    );
}

#[test]
fn test_transfer_exceeding_balance_sender_balance_unchanged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    client.buy_key(&creator, &sender, &KEY_PRICE, &None);

    let before = capture_snapshot(&client, &creator, &sender);
    let _ = client.try_transfer_keys(&creator, &sender, &recipient, &3);
    let after = capture_snapshot(&client, &creator, &sender);

    before.assert_unchanged(&after);
}

#[test]
fn test_transfer_exceeding_balance_total_supply_unchanged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.buy_key(&creator, &sender, &KEY_PRICE, &None);

    let supply_before = client.get_total_key_supply(&creator);
    let _ = client.try_transfer_keys(&creator, &sender, &recipient, &5);
    let supply_after = client.get_total_key_supply(&creator);

    assert_eq!(
        supply_before, supply_after,
        "total supply must be unchanged after failed transfer"
    );
}
