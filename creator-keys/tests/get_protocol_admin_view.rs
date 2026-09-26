//! Tests for the read-only protocol admin view `get_protocol_admin`.
//!
//! Verifies that:
//! - A freshly deployed contract reports no admin (`None`) before configuration
//! - After initialisation the view returns exactly the address stored by the
//!   first `set_protocol_admin(admin, admin)` call
//! - After an admin transfer the view reflects the new admin and not the old one
//! - Repeated reads are stable (read-only purity)
//! - Failed transfers (unauthorized caller, zero-address recipient) leave the
//!   reported admin unchanged
//! - An idempotent self-set leaves the reported admin unchanged

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, String};

/// A fresh contract has no configured admin, so the view must return `None`.
#[test]
fn test_admin_view_unset_on_fresh_contract() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    assert_eq!(client.get_protocol_admin(), None);
}

/// The view returns exactly the address provided at initialisation via the
/// first `set_protocol_admin(a, a)` call.
#[test]
fn test_admin_view_matches_initialisation_address() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    assert_eq!(client.get_protocol_admin(), None);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    assert_eq!(client.get_protocol_admin(), Some(admin));
}

/// After a successful transfer the view reports the new admin and explicitly
/// not the previous holder of the role.
#[test]
fn test_admin_view_reflects_transfer_target_not_old_holder() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let old_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.set_protocol_admin(&old_admin, &old_admin);
    assert_eq!(client.get_protocol_admin(), Some(old_admin.clone()));

    client.set_protocol_admin(&old_admin, &new_admin);

    assert_eq!(client.get_protocol_admin(), Some(new_admin.clone()));
    assert_ne!(
        client.get_protocol_admin(),
        Some(old_admin),
        "view must not report the pre-transfer admin"
    );
}

/// Consecutive reads return identical values, documenting read-only purity.
#[test]
fn test_admin_view_repeated_reads_are_stable() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    let first_read = client.get_protocol_admin();
    let second_read = client.get_protocol_admin();

    assert_eq!(first_read, second_read, "repeated reads must be consistent");
    assert_eq!(first_read, Some(admin));
}

/// Failed transfer attempts (unauthorized caller or zero-address recipient)
/// leave the reported admin unchanged.
#[test]
fn test_failed_transfers_leave_admin_view_unchanged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let old_admin = Address::generate(&env);
    let wrong_caller = Address::generate(&env);
    let candidate = Address::generate(&env);

    client.set_protocol_admin(&old_admin, &old_admin);

    let unauthorized = client.try_set_protocol_admin(&wrong_caller, &candidate);
    assert_eq!(unauthorized, Err(Ok(ContractError::Unauthorized)));
    assert_eq!(
        client.get_protocol_admin(),
        Some(old_admin.clone()),
        "unauthorized attempt must not change the admin view"
    );

    let zero_str = String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero_addr = Address::from_string(&zero_str);

    let zero_rejected = client.try_set_protocol_admin(&old_admin, &zero_addr);
    assert_eq!(zero_rejected, Err(Ok(ContractError::ZeroAddress)));
    assert_eq!(
        client.get_protocol_admin(),
        Some(old_admin),
        "rejected zero-address transfer must not change the admin view"
    );
}

/// An idempotent self-set `(old, old)` succeeds without altering the view.
#[test]
fn test_self_set_transfer_is_idempotent_for_view() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let old_admin = Address::generate(&env);
    client.set_protocol_admin(&old_admin, &old_admin);
    assert_eq!(client.get_protocol_admin(), Some(old_admin.clone()));

    let result = client.try_set_protocol_admin(&old_admin, &old_admin);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(
        client.get_protocol_admin(),
        Some(old_admin),
        "self-set must leave the admin view unchanged"
    );
}
