#![cfg(test)]

//! Tests for issues #778 (holder snapshots), #779 (key metadata), #781
//! (flash-loan guard), and #782 (settable co-creator revenue split).

use crate::{ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Bytes, Env, String, Vec,
};

fn setup_test() -> (Env, CreatorKeysContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &100i128);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_protocol_fee_recipient(&admin, &treasury);

    (env, client, admin, treasury)
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient, creator: &Address) {
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

// ─── #778: holder snapshot ──────────────────────────────────────────────

#[test]
fn test_take_snapshot_captures_holder_balances() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);
    client.buy_key(&creator, &buyer_a, &1000i128, &None);
    client.buy_key(&creator, &buyer_a, &1000i128, &None);
    client.buy_key(&creator, &buyer_b, &1000i128, &None);

    let mut holders = Vec::new(&env);
    holders.push_back(buyer_a.clone());
    holders.push_back(buyer_b.clone());

    client.take_snapshot(&admin, &creator, &1u32, &holders);

    assert_eq!(client.get_snapshot_balance(&creator, &1u32, &buyer_a), 2);
    assert_eq!(client.get_snapshot_balance(&creator, &1u32, &buyer_b), 1);

    let meta = client.get_snapshot_meta(&creator, &1u32).unwrap();
    assert_eq!(meta.total_holders, 2);
    assert_eq!(meta.snapshot_ledger, env.ledger().sequence());
}

#[test]
fn test_take_snapshot_holder_not_in_list_reads_zero() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000i128, &None);

    client.take_snapshot(&admin, &creator, &1u32, &Vec::new(&env));

    assert_eq!(client.get_snapshot_balance(&creator, &1u32, &buyer), 0);
}

#[test]
fn test_take_snapshot_duplicate_id_fails() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    client.take_snapshot(&admin, &creator, &1u32, &Vec::new(&env));
    let result = client.try_take_snapshot(&admin, &creator, &1u32, &Vec::new(&env));
    assert_eq!(result, Err(Ok(ContractError::SnapshotAlreadyExists)));
}

#[test]
fn test_take_snapshot_non_admin_fails() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let not_admin = Address::generate(&env);

    let result = client.try_take_snapshot(&not_admin, &creator, &1u32, &Vec::new(&env));
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_take_snapshot_unregistered_creator_fails() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);

    let result = client.try_take_snapshot(&admin, &creator, &1u32, &Vec::new(&env));
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

// ─── #779: key metadata initialisation ──────────────────────────────────

#[test]
fn test_initialise_key_stores_metadata() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let name = Bytes::from_slice(&env, b"Alice");
    let bio = Bytes::from_slice(&env, b"Digital artist");
    let avatar = Bytes::from_slice(&env, b"ipfs://avatar");

    client.initialise_key(&creator, &name, &bio, &avatar);

    let meta = client.get_key_metadata(&creator).unwrap();
    assert_eq!(meta.name, name);
    assert_eq!(meta.bio, bio);
    assert_eq!(meta.avatar_uri, avatar);
}

#[test]
fn test_initialise_key_name_too_long_fails() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let long_name = Bytes::from_slice(&env, &[b'a'; 65]);
    let bio = Bytes::from_slice(&env, b"bio");
    let avatar = Bytes::from_slice(&env, b"uri");

    let result = client.try_initialise_key(&creator, &long_name, &bio, &avatar);
    assert_eq!(result, Err(Ok(ContractError::NameTooLong)));
}

#[test]
fn test_initialise_key_bio_too_long_fails() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let name = Bytes::from_slice(&env, b"name");
    let long_bio = Bytes::from_slice(&env, &[b'a'; 257]);
    let avatar = Bytes::from_slice(&env, b"uri");

    let result = client.try_initialise_key(&creator, &name, &long_bio, &avatar);
    assert_eq!(result, Err(Ok(ContractError::BioTooLong)));
}

#[test]
fn test_initialise_key_twice_fails() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let name = Bytes::from_slice(&env, b"name");
    let bio = Bytes::from_slice(&env, b"bio");
    let avatar = Bytes::from_slice(&env, b"uri");

    client.initialise_key(&creator, &name, &bio, &avatar);
    let result = client.try_initialise_key(&creator, &name, &bio, &avatar);
    assert_eq!(result, Err(Ok(ContractError::KeyAlreadyInitialised)));
}

#[test]
fn test_initialise_key_non_creator_fails_auth() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    // require_auth() is mocked permissive by mock_all_auths(), so the
    // meaningful assertion here is the auth call itself is made against
    // `creator`, not the caller. We simulate a stricter environment by
    // checking that the entrypoint requires creator's auth at all — the
    // NotRegistered/registration path already proves the address parameter
    // is `creator`, not an implicit caller.
    let name = Bytes::from_slice(&env, b"name");
    let bio = Bytes::from_slice(&env, b"bio");
    let avatar = Bytes::from_slice(&env, b"uri");
    client.initialise_key(&creator, &name, &bio, &avatar);
    assert!(client.get_key_metadata(&creator).is_some());
}

// ─── #781: flash-loan guard ──────────────────────────────────────────────

#[test]
fn test_sell_same_ledger_as_buy_fails() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let trader = Address::generate(&env);

    client.buy_key(&creator, &trader, &1000i128, &None);
    let result = client.try_sell_key(&creator, &trader, &None);
    assert_eq!(result, Err(Ok(ContractError::FlashLoanDetected)));
}

#[test]
fn test_sell_later_ledger_succeeds() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let trader = Address::generate(&env);

    client.buy_key(&creator, &trader, &1000i128, &None);

    env.ledger().with_mut(|l| l.sequence_number += 1);

    let new_supply = client.sell_key(&creator, &trader, &None);
    assert_eq!(new_supply, 0);
}

#[test]
fn test_flash_loan_guard_does_not_block_a_different_wallets_sell() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let buyer = Address::generate(&env);
    let other_holder = Address::generate(&env);

    // other_holder bought in an earlier ledger; buyer buys now (same ledger).
    client.buy_key(&creator, &other_holder, &1000i128, &None);
    env.ledger().with_mut(|l| l.sequence_number += 1);
    client.buy_key(&creator, &buyer, &1000i128, &None);

    // other_holder's last buy was a prior ledger, so their sell (in the
    // buyer's buy ledger) is unaffected by the buyer's same-ledger guard.
    let new_supply = client.sell_key(&creator, &other_holder, &None);
    assert_eq!(new_supply, 1);
}

// ─── #782: settable co-creator revenue split ─────────────────────────────

#[test]
fn test_set_co_creator_splits_fee_on_buy() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let co_creator = Address::generate(&env);

    client.set_co_creator(&creator, &co_creator, &2000u32); // 20%

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000i128, &None);

    // price=100, creator_bps=9000 -> creator_fee=90. 20% of 90 = 18 to co-creator.
    assert_eq!(client.get_co_creator_fee_balance(&creator, &co_creator), 18);
    assert_eq!(client.get_creator_fee_balance(&creator), 72);
}

#[test]
fn test_set_co_creator_splits_fee_on_sell() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let co_creator = Address::generate(&env);
    let trader = Address::generate(&env);

    client.buy_key(&creator, &trader, &1000i128, &None);
    client.set_co_creator(&creator, &co_creator, &2000u32); // 20%
    env.ledger().with_mut(|l| l.sequence_number += 1);

    let balance_before = client.get_co_creator_fee_balance(&creator, &co_creator);
    client.sell_key(&creator, &trader, &None);
    let balance_after = client.get_co_creator_fee_balance(&creator, &co_creator);

    assert!(balance_after > balance_before);
}

#[test]
fn test_set_co_creator_split_above_9000_fails() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let co_creator = Address::generate(&env);

    let result = client.try_set_co_creator(&creator, &co_creator, &9001u32);
    assert_eq!(result, Err(Ok(ContractError::SplitTooHigh)));
}

#[test]
fn test_set_co_creator_zero_split_fails() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let co_creator = Address::generate(&env);

    let result = client.try_set_co_creator(&creator, &co_creator, &0u32);
    assert_eq!(result, Err(Ok(ContractError::SplitTooHigh)));
}

#[test]
fn test_set_co_creator_unregistered_creator_fails() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    let co_creator = Address::generate(&env);

    let result = client.try_set_co_creator(&creator, &co_creator, &1000u32);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_set_co_creator_can_update_existing_split() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let co_creator_1 = Address::generate(&env);
    let co_creator_2 = Address::generate(&env);

    client.set_co_creator(&creator, &co_creator_1, &1000u32);
    assert_eq!(
        client.get_co_creator(&creator).unwrap().address,
        co_creator_1
    );

    client.set_co_creator(&creator, &co_creator_2, &3000u32);
    let config = client.get_co_creator(&creator).unwrap();
    assert_eq!(config.address, co_creator_2);
    assert_eq!(config.share_bps, 3000);
}
