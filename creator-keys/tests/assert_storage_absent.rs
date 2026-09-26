//! Unit tests for the `assert_storage_absent` test helper.
//!
//! Confirms the helper:
//! - Passes silently when the storage key does not exist.
//! - Panics when the storage key is present.
//! - Includes the key identifier in the panic message.
//! - Passes after a write-then-delete sequence.

mod contract_test_env;

use contract_test_env::{assert_storage_absent, register_creator_keys};
use creator_keys::constants;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;
use std::panic;

/// Key that is never written by any test setup → should pass silently.
#[test]
fn test_passes_when_key_absent() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (_, contract_id) = register_creator_keys(&env);

    env.as_contract(&contract_id, || {
        assert_storage_absent(&env, &constants::storage::KEY_PRICE);
    });
}

/// Key that is explicitly written → assert_storage_absent must panic.
#[test]
fn test_panics_when_key_present() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (_, contract_id) = register_creator_keys(&env);

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&constants::storage::KEY_PRICE, &100i128);
    });

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        env.as_contract(&contract_id, || {
            assert_storage_absent(&env, &constants::storage::KEY_PRICE);
        });
    }));

    assert!(
        result.is_err(),
        "assert_storage_absent should panic when key is present"
    );
}

/// Confirm the panic message includes the debug representation of the key.
#[test]
fn test_panic_message_contains_key_identifier() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (_, contract_id) = register_creator_keys(&env);

    let creator = Address::generate(&env);

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&constants::storage::creator(&creator), &true);
    });

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        env.as_contract(&contract_id, || {
            assert_storage_absent(&env, &constants::storage::creator(&creator));
        });
    }));

    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<String>()
        .expect("panic payload should be a String");

    assert!(
        msg.contains("unexpectedly present"),
        "panic message should contain the expected text"
    );
    assert!(
        msg.contains("Creator("),
        "panic message should contain the key variant name"
    );
}

/// After writing a key and then removing it, assert_storage_absent should pass.
#[test]
fn test_passes_after_write_then_delete() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (_, contract_id) = register_creator_keys(&env);

    let key = constants::storage::KEY_PRICE;

    env.as_contract(&contract_id, || {
        env.storage().persistent().set(&key, &100i128);
    });

    env.as_contract(&contract_id, || {
        env.storage().persistent().remove(&key);
    });

    env.as_contract(&contract_id, || {
        assert_storage_absent(&env, &key);
    });
}
