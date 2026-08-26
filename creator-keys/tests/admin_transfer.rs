//! Tests for admin role transfer via `set_protocol_admin`.
//!
//! Verifies that:
//! - Admin role transfers to a new address and is persisted in storage
//! - The old admin loses admin privileges after transfer
//! - The new admin gains admin privileges after transfer
//! - A non-admin caller is rejected with `ContractError::Unauthorized`
//! - Transfer to the zero address is rejected with `ContractError::ZeroAddress`

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_admin_transfer_updates_stored_admin() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let old_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    assert_eq!(client.get_protocol_admin(), None);

    client.set_protocol_admin(&old_admin, &old_admin);
    assert_eq!(client.get_protocol_admin(), Some(old_admin.clone()));

    let result = client.try_set_protocol_admin(&old_admin, &new_admin);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(client.get_protocol_admin(), Some(new_admin));
}

#[test]
fn test_old_admin_cannot_pause_after_transfer() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let old_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.set_protocol_admin(&old_admin, &old_admin);
    client.set_protocol_admin(&old_admin, &new_admin);

    let result = client.try_pause(&old_admin);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_new_admin_can_pause_after_transfer() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let old_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.set_protocol_admin(&old_admin, &old_admin);
    client.set_protocol_admin(&old_admin, &new_admin);

    let result = client.try_pause(&new_admin);
    assert_eq!(result, Ok(Ok(())));
    assert!(client.get_is_paused());
}

#[test]
fn test_non_admin_cannot_transfer_admin_role() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);

    let result = client.try_set_protocol_admin(&non_admin, &new_admin);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));

    assert_eq!(client.get_protocol_admin(), Some(admin));
}

#[test]
fn test_transfer_to_zero_address_rejected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);

    let zero_str = String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero_addr = Address::from_string(&zero_str);

    let result = client.try_set_protocol_admin(&admin, &zero_addr);
    assert_eq!(
        result,
        Err(Ok(ContractError::ZeroAddress)),
        "zero address must be rejected"
    );

    assert_eq!(client.get_protocol_admin(), Some(admin));
}
