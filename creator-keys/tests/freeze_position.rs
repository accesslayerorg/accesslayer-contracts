//! Tests for the owner-only boolean `freeze_position` / `unfreeze_position`
//! lock, which blocks buy, sell and transfer for the frozen position.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_ledger_sequence,
    test_env_with_auths,
};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

const KEY_PRICE: i128 = 100;

/// Contract with one creator and a holder who owns one key.
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    let holder = Address::generate(env);
    set_ledger_sequence(env, 100);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    set_ledger_sequence(env, 101);
    (client, creator, holder)
}

#[test]
fn test_freeze_blocks_buy_sell_and_transfer_until_unfrozen() {
    let env = test_env_with_auths();
    let (client, creator, holder) = setup(&env);
    let other = Address::generate(&env);

    assert!(!client.get_position_frozen(&creator, &holder));
    client.freeze_position(&creator, &holder);
    assert!(client.get_position_frozen(&creator, &holder));

    assert_eq!(
        client.try_buy_key(&creator, &holder, &KEY_PRICE, &None),
        Err(Ok(ContractError::FrozenPosition))
    );
    assert_eq!(
        client.try_sell_key(&creator, &holder, &None),
        Err(Ok(ContractError::FrozenPosition))
    );
    assert_eq!(
        client.try_transfer_keys(&creator, &holder, &other, &1),
        Err(Ok(ContractError::FrozenPosition))
    );
    assert_eq!(client.get_key_balance(&creator, &holder), 1);

    client.unfreeze_position(&creator, &holder);
    assert!(!client.get_position_frozen(&creator, &holder));

    client.transfer_keys(&creator, &holder, &other, &1);
    assert_eq!(client.get_key_balance(&creator, &other), 1);
}

#[test]
fn test_unfreeze_restores_sell() {
    let env = test_env_with_auths();
    let (client, creator, holder) = setup(&env);

    client.freeze_position(&creator, &holder);
    client.unfreeze_position(&creator, &holder);

    client.sell_key(&creator, &holder, &None);
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
}

#[test]
fn test_freeze_only_affects_the_owner_position() {
    let env = test_env_with_auths();
    let (client, creator, holder) = setup(&env);
    let other = Address::generate(&env);

    client.freeze_position(&creator, &holder);
    client.buy_key(&creator, &other, &KEY_PRICE, &None);
    assert_eq!(client.get_key_balance(&creator, &other), 1);
    assert!(!client.get_position_frozen(&creator, &other));
}

#[test]
fn test_freeze_requires_owner_authorization() {
    let env = Env::default();
    let (client, _) = register_creator_keys(&env);
    let creator = Address::generate(&env);
    let holder = Address::generate(&env);

    // No auths mocked: the holder has not authorized the call, so it must fail.
    assert!(client.try_freeze_position(&creator, &holder).is_err());
    assert!(!client.get_position_frozen(&creator, &holder));
}

#[test]
fn test_freeze_without_position_is_rejected() {
    let env = test_env_with_auths();
    let (client, creator, _holder) = setup(&env);
    let stranger = Address::generate(&env);

    assert_eq!(
        client.try_freeze_position(&creator, &stranger),
        Err(Ok(ContractError::InsufficientBalance))
    );
}
