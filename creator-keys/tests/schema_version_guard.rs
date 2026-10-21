//! Unit tests for the schema version guard (issue #707).
//!
//! Tests that `check_schema_version` (and the underlying `assert_schema_version`
//! pure function) reject calls whose client schema version is outdated or from
//! the future, and accept only the current supported version.

mod contract_test_env;

use contract_test_env::test_env_with_auths;
use creator_keys::{
    assert_schema_version, ContractError, CreatorKeysContract, CreatorKeysContractClient,
    CURRENT_SCHEMA_VERSION, MIN_SCHEMA_VERSION,
};
use soroban_sdk::{testutils::Address as _, Address};

// ---------------------------------------------------------------------------
// Pure guard – no Soroban Env needed
// ---------------------------------------------------------------------------

#[test]
fn pure_guard_current_version_succeeds() {
    assert_eq!(
        assert_schema_version(CURRENT_SCHEMA_VERSION),
        Ok(()),
        "current schema version must be accepted"
    );
}

#[test]
fn pure_guard_version_zero_is_too_old() {
    assert_eq!(
        assert_schema_version(0),
        Err(ContractError::SchemaVersionTooOld),
        "version 0 must be rejected as too old"
    );
}

#[test]
fn pure_guard_below_min_is_too_old() {
    if MIN_SCHEMA_VERSION > 1 {
        // Only meaningful when there is a range of outdated versions.
        assert_eq!(
            assert_schema_version(MIN_SCHEMA_VERSION - 1),
            Err(ContractError::SchemaVersionTooOld),
            "version below MIN_SCHEMA_VERSION must be rejected as too old"
        );
    } else {
        // MIN_SCHEMA_VERSION == 1: version 0 is the only sub-minimum case,
        // covered by the zero test above.
        assert_eq!(
            assert_schema_version(0),
            Err(ContractError::SchemaVersionTooOld)
        );
    }
}

#[test]
fn pure_guard_future_version_is_unsupported() {
    assert_eq!(
        assert_schema_version(CURRENT_SCHEMA_VERSION + 1),
        Err(ContractError::SchemaVersionUnsupported),
        "version beyond CURRENT_SCHEMA_VERSION must be rejected as unsupported"
    );
}

// ---------------------------------------------------------------------------
// Contract entrypoint – via Soroban test client
// ---------------------------------------------------------------------------

// AC-1: current schema version is accepted
#[test]
fn contract_current_version_succeeds() {
    let env = test_env_with_auths();
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &id);

    let result = client.try_check_schema_version(&CURRENT_SCHEMA_VERSION);
    assert!(result.is_ok(), "current schema version must succeed");
}

// AC-2: current_version - 1 panics with schema_version_too_old
#[test]
fn contract_one_below_current_is_too_old() {
    let env = test_env_with_auths();
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &id);

    // When CURRENT_SCHEMA_VERSION == 1, saturating_sub gives 0, which is the
    // correct "one below current" value and must still be rejected as too old.
    let outdated = CURRENT_SCHEMA_VERSION.saturating_sub(1);

    let result = client.try_check_schema_version(&outdated);
    assert_eq!(
        result,
        Err(Ok(ContractError::SchemaVersionTooOld)),
        "version below current must be rejected as schema_version_too_old"
    );
}

// AC-3: schema version 0 panics with schema_version_too_old
#[test]
fn contract_version_zero_is_too_old() {
    let env = test_env_with_auths();
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &id);

    let result = client.try_check_schema_version(&0u32);
    assert_eq!(
        result,
        Err(Ok(ContractError::SchemaVersionTooOld)),
        "version 0 must be rejected as schema_version_too_old"
    );
}

// AC-4: future version (current + 1) panics with schema_version_unsupported
#[test]
fn contract_future_version_is_unsupported() {
    let env = test_env_with_auths();
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &id);

    let future = CURRENT_SCHEMA_VERSION + 1;
    let result = client.try_check_schema_version(&future);
    assert_eq!(
        result,
        Err(Ok(ContractError::SchemaVersionUnsupported)),
        "version beyond current must be rejected as schema_version_unsupported"
    );
}

// AC-5: no state mutation on any rejected call
//
// The contract stores no state during `check_schema_version`. We verify this by
// calling a state-reading entrypoint before and after a rejected version check
// and asserting the observable state (key supply for an unregistered creator) is
// unchanged.
#[test]
fn contract_no_state_mutation_on_too_old_rejection() {
    let env = test_env_with_auths();
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &id);

    let creator = Address::generate(&env);

    let supply_before = client.get_total_key_supply(&creator);
    let _ = client.try_check_schema_version(&0u32);
    let supply_after = client.get_total_key_supply(&creator);

    assert_eq!(
        supply_before, supply_after,
        "rejected schema version check must not mutate contract state"
    );
}

#[test]
fn contract_no_state_mutation_on_unsupported_rejection() {
    let env = test_env_with_auths();
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &id);

    let creator = Address::generate(&env);

    let supply_before = client.get_total_key_supply(&creator);
    let _ = client.try_check_schema_version(&(CURRENT_SCHEMA_VERSION + 1));
    let supply_after = client.get_total_key_supply(&creator);

    assert_eq!(
        supply_before, supply_after,
        "rejected schema version check must not mutate contract state"
    );
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn pure_guard_max_u32_version_is_unsupported() {
    assert_eq!(
        assert_schema_version(u32::MAX),
        Err(ContractError::SchemaVersionUnsupported),
        "u32::MAX must be rejected as unsupported"
    );
}

#[test]
fn pure_guard_min_version_constant_is_accepted() {
    // MIN_SCHEMA_VERSION itself must always be accepted (it is the lowest valid version).
    assert_eq!(
        assert_schema_version(MIN_SCHEMA_VERSION),
        Ok(()),
        "MIN_SCHEMA_VERSION itself must be accepted"
    );
}
