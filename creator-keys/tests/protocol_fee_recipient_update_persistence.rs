//! Persistence tests for the protocol fee recipient (treasury) update entrypoint (#740).
//!
//! `update_protocol_fee_recipient` rotates the address that protocol-side trade
//! fees accrue to. Existing coverage checks that the read view reflects an update
//! (`protocol_fee_recipient.rs`) and that the `set_` entrypoint rejects the zero
//! address (`set_protocol_fee_recipient.rs`). What is not covered is the update
//! entrypoint's own guard rails and the downstream effect of a rotation:
//!
//! - the new address is what is actually persisted, and the old one is gone
//! - fees accrued *after* the rotation are attributed to the new recipient
//! - a non-admin caller is rejected with [`ContractError::Unauthorized`]
//! - the zero address is rejected with [`ContractError::ZeroAddress`] and the
//!   previously stored recipient survives the rejected call

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use creator_keys::{constants, ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, String};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

/// The Stellar all-zero account address, rejected by the address validators.
fn zero_address(env: &Env) -> Address {
    Address::from_string(&String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ))
}

/// Read the raw persisted recipient straight out of contract storage.
///
/// The view function is the normal way in, but reading the storage key directly
/// is what lets these tests distinguish "the view returns the new address" from
/// "the new address is what is on the ledger".
fn stored_recipient(env: &Env, contract_id: &Address) -> Option<Address> {
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .get(&constants::storage::PROTOCOL_FEE_RECIPIENT)
    })
}

/// Register the contract with an admin, a fee split and an initial recipient.
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address, Address) {
    let (client, contract_id) = register_creator_keys(env);
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &KEY_PRICE);
    client.set_fee_config(&admin, &CREATOR_BPS, &PROTOCOL_BPS);

    let original_recipient = Address::generate(env);
    client.set_protocol_fee_recipient(&admin, &original_recipient);

    (client, contract_id, admin, original_recipient)
}

#[test]
fn update_persists_new_recipient_in_contract_state() {
    let env = test_env_with_auths();
    let (client, contract_id, admin, original) = setup(&env);

    assert_eq!(
        stored_recipient(&env, &contract_id),
        Some(original.clone()),
        "precondition: the original recipient should be on the ledger"
    );

    let new_recipient = Address::generate(&env);
    let result = client.try_update_protocol_fee_recipient(&admin, &new_recipient);
    assert_eq!(result, Ok(Ok(())), "admin rotation should succeed");

    assert_eq!(
        stored_recipient(&env, &contract_id),
        Some(new_recipient),
        "the new recipient should be the persisted value"
    );
}

#[test]
fn view_returns_the_updated_recipient() {
    let env = test_env_with_auths();
    let (client, _contract_id, admin, _original) = setup(&env);

    let new_recipient = Address::generate(&env);
    client.update_protocol_fee_recipient(&admin, &new_recipient);

    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(new_recipient),
        "get_protocol_fee_recipient should report the rotated address"
    );
}

#[test]
fn old_recipient_is_absent_from_state_after_update() {
    let env = test_env_with_auths();
    let (client, contract_id, admin, original) = setup(&env);

    let new_recipient = Address::generate(&env);
    client.update_protocol_fee_recipient(&admin, &new_recipient);

    let stored = stored_recipient(&env, &contract_id).expect("recipient should still be set");
    assert_ne!(
        stored, original,
        "the superseded recipient must not remain on the ledger"
    );
    assert_ne!(
        client.get_protocol_fee_recipient(),
        Some(original),
        "the view must not report the superseded recipient"
    );
}

/// The point of the rotation is where the money goes next, so assert on the
/// accrued balance rather than only on the stored address.
#[test]
fn fees_accrued_after_the_update_are_attributed_to_the_new_recipient() {
    let env = test_env_with_auths();
    let (client, _contract_id, admin, _original) = setup(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    // Accrue some protocol fees against the original recipient.
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &holder, &None);
    let balance_before_rotation = client.get_protocol_recipient_balance();
    assert!(
        balance_before_rotation > 0,
        "precondition: a round trip should accrue protocol fees, got {balance_before_rotation}"
    );

    let new_recipient = Address::generate(&env);
    client.update_protocol_fee_recipient(&admin, &new_recipient);

    // A second round trip after the rotation must keep accruing, and the
    // recipient of record for that accrual is now the new address.
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &holder, &None);

    assert!(
        client.get_protocol_recipient_balance() > balance_before_rotation,
        "post-rotation trades should keep accruing protocol fees"
    );
    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(new_recipient),
        "the accrued balance should be payable to the rotated recipient"
    );
}

#[test]
fn non_admin_caller_is_rejected_as_unauthorized() {
    let env = test_env_with_auths();
    let (client, contract_id, _admin, original) = setup(&env);

    let impostor = Address::generate(&env);
    let attacker_recipient = Address::generate(&env);

    let result = client.try_update_protocol_fee_recipient(&impostor, &attacker_recipient);
    assert_eq!(
        result,
        Err(Ok(ContractError::Unauthorized)),
        "only the protocol admin may rotate the fee recipient"
    );

    assert_eq!(
        stored_recipient(&env, &contract_id),
        Some(original),
        "a rejected rotation must leave the stored recipient untouched"
    );
}

#[test]
fn zero_address_is_rejected_and_leaves_the_recipient_intact() {
    let env = test_env_with_auths();
    let (client, contract_id, admin, original) = setup(&env);

    let result = client.try_update_protocol_fee_recipient(&admin, &zero_address(&env));
    assert_eq!(
        result,
        Err(Ok(ContractError::ZeroAddress)),
        "rotating to the zero address would silently burn protocol fees"
    );

    assert_eq!(
        stored_recipient(&env, &contract_id),
        Some(original),
        "a rejected rotation must leave the stored recipient untouched"
    );
}
