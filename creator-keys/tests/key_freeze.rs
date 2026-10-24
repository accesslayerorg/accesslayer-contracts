//! Tests for the key-level freeze / unfreeze administration (#1174).
//!
//! A frozen key rejects buys, sells and transfers with
//! [`ContractError::KeyFrozen`], and every freeze/unfreeze action is appended
//! to an on-chain audit log.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_ledger_sequence,
    set_test_timestamp, test_env_with_auths,
};
use creator_keys::key_freeze::{FreezeAction, FreezeLogEntry, KeyStatus};
use creator_keys::{events, ContractError, CreatorKeysContractClient, SwapDirection};
use soroban_sdk::{
    testutils::{Address as _, Events},
    vec, Address, Env, IntoVal, String, Vec,
};

const KEY_PRICE: i128 = 100;

/// Fixture: one registered creator, a holder that owns one key and the
/// creator's multi-sig admin set.
struct Fixture<'a> {
    client: CreatorKeysContractClient<'a>,
    creator: Address,
    holder: Address,
    admin: Address,
}

fn setup(env: &Env) -> Fixture<'_> {
    let (client, _contract_id) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    let holder = Address::generate(env);
    let admin = Address::generate(env);
    set_ledger_sequence(env, 100);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    set_ledger_sequence(env, 101);
    client.set_multisig_admins(&creator, &vec![&env, admin.clone()]);
    Fixture {
        client,
        creator,
        holder,
        admin,
    }
}

#[test]
fn test_freeze_sets_status_and_blocks_buy_sell_and_transfer() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let other = Address::generate(&env);

    assert_eq!(f.client.get_key_status(&f.creator), KeyStatus::Active);
    assert!(!f.client.is_key_frozen(&f.creator));

    f.client
        .freeze(&f.creator, &String::from_str(&env, "abuse"), &f.admin);

    assert_eq!(f.client.get_key_status(&f.creator), KeyStatus::Frozen);
    assert!(f.client.is_key_frozen(&f.creator));

    assert_eq!(
        f.client
            .try_buy_key(&f.creator, &f.holder, &KEY_PRICE, &None),
        Err(Ok(ContractError::KeyFrozen))
    );
    assert_eq!(
        f.client.try_sell_key(&f.creator, &f.holder, &None),
        Err(Ok(ContractError::KeyFrozen))
    );
    assert_eq!(
        f.client
            .try_transfer_keys(&f.creator, &f.holder, &other, &1),
        Err(Ok(ContractError::KeyFrozen))
    );
    assert_eq!(f.client.get_key_balance(&f.creator, &f.holder), 1);
}

#[test]
fn test_unfreeze_restores_trading() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let other = Address::generate(&env);

    f.client
        .freeze(&f.creator, &String::from_str(&env, "abuse"), &f.admin);
    f.client.unfreeze(&f.creator, &f.admin);

    assert_eq!(f.client.get_key_status(&f.creator), KeyStatus::Active);
    assert!(!f.client.is_key_frozen(&f.creator));

    f.client.transfer_keys(&f.creator, &f.holder, &other, &1);
    assert_eq!(f.client.get_key_balance(&f.creator, &other), 1);
}

#[test]
fn test_freeze_log_records_every_action() {
    let env = test_env_with_auths();
    let f = setup(&env);

    assert!(f.client.get_freeze_log(&f.creator).is_empty());

    set_test_timestamp(&env, 1_700_000_000);
    f.client
        .freeze(&f.creator, &String::from_str(&env, "rug pull"), &f.admin);
    set_test_timestamp(&env, 1_700_000_500);
    f.client.unfreeze(&f.creator, &f.admin);
    set_test_timestamp(&env, 1_700_001_000);
    f.client.freeze(
        &f.creator,
        &String::from_str(&env, "second offence"),
        &f.admin,
    );

    let log = f.client.get_freeze_log(&f.creator);
    assert_eq!(log.len(), 3);

    let first = log.get(0).unwrap();
    assert_eq!(first.action, FreezeAction::Freeze);
    assert_eq!(first.reason, String::from_str(&env, "rug pull"));
    assert_eq!(first.admin_wallet, f.admin);
    assert_eq!(first.timestamp, 1_700_000_000);

    let second = log.get(1).unwrap();
    assert_eq!(second.action, FreezeAction::Unfreeze);
    assert_eq!(second.reason, String::from_str(&env, ""));
    assert_eq!(second.admin_wallet, f.admin);
    assert_eq!(second.timestamp, 1_700_000_500);

    let third = log.get(2).unwrap();
    assert_eq!(third.action, FreezeAction::Freeze);
    assert_eq!(third.reason, String::from_str(&env, "second offence"));
    assert_eq!(third.timestamp, 1_700_001_000);
}

#[test]
fn test_freeze_log_is_scoped_to_the_key() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let other_creator = register_test_creator(&env, &f.client, "bob");
    let other_admin = Address::generate(&env);
    f.client
        .set_multisig_admins(&other_creator, &vec![&env, other_admin]);

    f.client
        .freeze(&f.creator, &String::from_str(&env, "abuse"), &f.admin);

    assert_eq!(f.client.get_freeze_log(&f.creator).len(), 1);
    assert!(f.client.get_freeze_log(&other_creator).is_empty());
    assert_eq!(f.client.get_key_status(&other_creator), KeyStatus::Active);
}

#[test]
fn test_freeze_requires_a_multisig_admin() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let stranger = Address::generate(&env);

    assert_eq!(
        f.client
            .try_freeze(&f.creator, &String::from_str(&env, "abuse"), &stranger),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(f.client.get_key_status(&f.creator), KeyStatus::Active);
    assert!(f.client.get_freeze_log(&f.creator).is_empty());
}

#[test]
fn test_protocol_admin_can_freeze_and_unfreeze() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let protocol_admin = Address::generate(&env);
    f.client
        .set_protocol_admin(&protocol_admin, &protocol_admin);

    f.client.freeze(
        &f.creator,
        &String::from_str(&env, "governance"),
        &protocol_admin,
    );
    assert_eq!(f.client.get_key_status(&f.creator), KeyStatus::Frozen);

    f.client.unfreeze(&f.creator, &protocol_admin);
    assert_eq!(f.client.get_key_status(&f.creator), KeyStatus::Active);
}

#[test]
fn test_freeze_requires_the_admin_authorization() {
    // No auths mocked: the admin has not authorized the call, so it must fail.
    let env = Env::default();
    let (client, _) = register_creator_keys(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    assert!(client
        .try_freeze(&creator, &String::from_str(&env, "abuse"), &admin)
        .is_err());
    assert_eq!(client.get_key_status(&creator), KeyStatus::Active);
}

#[test]
fn test_freeze_of_an_unregistered_key_is_rejected() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let unknown = Address::generate(&env);

    assert_eq!(
        f.client
            .try_freeze(&unknown, &String::from_str(&env, "abuse"), &f.admin),
        Err(Ok(ContractError::NotRegistered))
    );
}

#[test]
fn test_double_freeze_and_unfreeze_of_an_active_key_are_rejected() {
    let env = test_env_with_auths();
    let f = setup(&env);

    assert_eq!(
        f.client.try_unfreeze(&f.creator, &f.admin),
        Err(Ok(ContractError::KeyNotFrozen))
    );

    f.client
        .freeze(&f.creator, &String::from_str(&env, "abuse"), &f.admin);
    assert_eq!(
        f.client
            .try_freeze(&f.creator, &String::from_str(&env, "again"), &f.admin),
        Err(Ok(ContractError::KeyAlreadyFrozen))
    );
    assert_eq!(f.client.get_freeze_log(&f.creator).len(), 1);
}

#[test]
fn test_freeze_blocks_the_batch_and_swap_paths() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let buyer = Address::generate(&env);

    f.client
        .freeze(&f.creator, &String::from_str(&env, "abuse"), &f.admin);

    let buy_orders = Vec::from_array(&env, [(f.creator.clone(), 1u32)]);
    assert_eq!(
        f.client.try_batch_buy(&buyer, &buy_orders),
        Err(Ok(ContractError::KeyFrozen))
    );

    let sell_orders = Vec::from_array(&env, [(f.creator.clone(), 1u32)]);
    assert_eq!(
        f.client.try_batch_sell(&f.holder, &sell_orders),
        Err(Ok(ContractError::KeyFrozen))
    );

    assert_eq!(
        f.client
            .try_swap(&f.creator, &f.holder, &1u32, &SwapDirection::Sell, &0u32),
        Err(Ok(ContractError::KeyFrozen))
    );
}

#[test]
fn test_freeze_emits_a_key_frozen_event() {
    let env = test_env_with_auths();
    let f = setup(&env);
    set_test_timestamp(&env, 1_700_000_000);

    f.client
        .freeze(&f.creator, &String::from_str(&env, "abuse"), &f.admin);

    let payload: events::KeyFrozenEvent = env.events().all().last().unwrap().2.into_val(&env);
    assert_eq!(payload.key_id, f.creator);
    assert_eq!(payload.admin_wallet, f.admin);
    assert_eq!(payload.reason, String::from_str(&env, "abuse"));
    assert_eq!(payload.timestamp, 1_700_000_000);
}

#[test]
fn test_freeze_log_entry_shape_is_stable() {
    let env = test_env_with_auths();
    let f = setup(&env);
    set_test_timestamp(&env, 1_700_000_000);

    f.client
        .freeze(&f.creator, &String::from_str(&env, "abuse"), &f.admin);

    let entry: FreezeLogEntry = f.client.get_freeze_log(&f.creator).get(0).unwrap();
    assert_eq!(entry.action, FreezeAction::Freeze);
    assert_eq!(entry.reason, String::from_str(&env, "abuse"));
    assert_eq!(entry.admin_wallet, f.admin);
    assert_eq!(entry.timestamp, 1_700_000_000);
}
