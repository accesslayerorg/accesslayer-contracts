//! Integration tests for the `transfer_keys` entrypoint.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_pricing_and_fees, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, String};

#[test]
fn test_transfer_keys_sender_balance_decreases() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);
    client.buy_key(&creator, &sender, &100, &None);
    client.buy_key(&creator, &sender, &100, &None);

    client.transfer_keys(&creator, &sender, &recipient, &1);

    assert_eq!(
        client.get_key_balance(&creator, &sender),
        2,
        "sender balance must decrease"
    );
}

#[test]
fn test_transfer_keys_recipient_balance_increases() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);

    client.transfer_keys(&creator, &sender, &recipient, &1);

    assert_eq!(
        client.get_key_balance(&creator, &recipient),
        1,
        "recipient balance must increase"
    );
}

#[test]
fn test_transfer_keys_total_supply_unchanged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);
    client.buy_key(&creator, &sender, &100, &None);

    let supply_before = client.get_total_key_supply(&creator);
    client.transfer_keys(&creator, &sender, &recipient, &1);
    let supply_after = client.get_total_key_supply(&creator);

    assert_eq!(supply_before, supply_after, "total supply must not change");
}

#[test]
fn test_transfer_keys_buy_quote_unchanged_after_transfer() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);
    client.buy_key(&creator, &sender, &100, &None);

    let quote_before = client.get_buy_quote(&creator);
    client.transfer_keys(&creator, &sender, &recipient, &1);
    let quote_after = client.get_buy_quote(&creator);

    assert_eq!(
        quote_before.price, quote_after.price,
        "buy price must not change after transfer"
    );
    assert_eq!(
        quote_before.total_amount, quote_after.total_amount,
        "buy total must not change after transfer"
    );
}

#[test]
fn test_transfer_keys_sell_quote_unchanged_after_transfer() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);
    client.buy_key(&creator, &sender, &100, &None);

    let quote_before = client.get_sell_quote(&creator, &sender);
    client.transfer_keys(&creator, &sender, &recipient, &1);
    let quote_after = client.get_sell_quote(&creator, &sender);

    assert_eq!(
        quote_before.price, quote_after.price,
        "sell price must not change after transfer"
    );
}

#[test]
fn test_transfer_keys_holder_count_unaffected_when_sender_zero_but_recipient_new() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);

    let holders_before = client.get_creator_holder_count(&creator);
    client.transfer_keys(&creator, &sender, &recipient, &1);
    let holders_after = client.get_creator_holder_count(&creator);

    assert_eq!(
        holders_before, holders_after,
        "holder count must not change when sender becomes zero but recipient was zero"
    );
    assert_eq!(
        client.get_key_balance(&creator, &sender),
        0,
        "sender must have zero balance"
    );
}

#[test]
fn test_transfer_keys_holder_count_increments_when_recipient_new() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender_a = Address::generate(&env);
    let sender_b = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender_a, &100, &None);
    client.buy_key(&creator, &sender_b, &100, &None);

    let holders_before = client.get_creator_holder_count(&creator);
    // Transfer 1 from sender_a (who has 2 keys) so sender_a stays above zero
    client.buy_key(&creator, &sender_a, &100, &None);
    client.transfer_keys(&creator, &sender_a, &recipient, &1);
    let holders_after = client.get_creator_holder_count(&creator);

    assert_eq!(
        holders_before + 1,
        holders_after,
        "holder count must increment when recipient was new"
    );
}

#[test]
fn test_transfer_keys_self_transfer_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);

    let result = client.try_transfer_keys(&creator, &sender, &sender, &1);
    assert_eq!(
        result,
        Err(Ok(ContractError::SelfTransfer)),
        "self-transfer must revert"
    );
}

#[test]
fn test_transfer_keys_zero_amount_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);

    let result = client.try_transfer_keys(&creator, &sender, &recipient, &0);
    assert_eq!(
        result,
        Err(Ok(ContractError::ZeroTransferAmount)),
        "zero amount must revert"
    );
}

#[test]
fn test_transfer_keys_exceeding_balance_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);

    let result = client.try_transfer_keys(&creator, &sender, &recipient, &2);
    assert_eq!(
        result,
        Err(Ok(ContractError::InsufficientBalance)),
        "transfer exceeding balance must revert"
    );
}

#[test]
fn test_transfer_keys_unregistered_creator_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let result = client.try_transfer_keys(&creator, &sender, &recipient, &1);
    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "unregistered creator must revert"
    );
}

#[test]
fn test_transfer_keys_self_transfer_sender_balance_unchanged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);
    client.buy_key(&creator, &sender, &100, &None);

    let balance_before = client.get_key_balance(&creator, &sender);
    let result = client.try_transfer_keys(&creator, &sender, &sender, &1);
    let balance_after = client.get_key_balance(&creator, &sender);

    assert_eq!(
        result,
        Err(Ok(ContractError::SelfTransfer)),
        "self-transfer must revert"
    );
    assert_eq!(
        balance_before, balance_after,
        "sender balance must be unchanged after self-transfer revert"
    );
}

#[test]
fn test_transfer_keys_self_transfer_total_supply_unchanged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);
    client.buy_key(&creator, &sender, &100, &None);

    let supply_before = client.get_total_key_supply(&creator);
    let result = client.try_transfer_keys(&creator, &sender, &sender, &1);
    let supply_after = client.get_total_key_supply(&creator);

    assert_eq!(
        result,
        Err(Ok(ContractError::SelfTransfer)),
        "self-transfer must revert"
    );
    assert_eq!(
        supply_before, supply_after,
        "total supply must be unchanged after self-transfer revert"
    );
}

#[test]
fn test_transfer_keys_preserves_other_holders() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let bystander = Address::generate(&env);

    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None);
    client.buy_key(&creator, &sender, &100, &None);
    client.buy_key(&creator, &bystander, &100, &None);
    client.buy_key(&creator, &bystander, &100, &None);

    client.transfer_keys(&creator, &sender, &recipient, &1);

    assert_eq!(
        client.get_key_balance(&creator, &bystander),
        2,
        "bystander balance must be unchanged"
    );
}
