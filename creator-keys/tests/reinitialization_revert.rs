//! Integration tests for re-initialization attempt reverting without modifying contract state (#584).
//!
//! Scope:
//! - Initialize the contract with admin A and fee bps 300
//! - Attempt to re-initialize with admin B and fee bps 500
//! - Assert the second call reverts with the already-initialized error code
//! - Assert admin and fee bps remain as set in the first initialization
//!
//! Acceptance Criteria:
//! - Re-initialization reverts with already-initialized error code
//! - Admin unchanged after failed re-init
//! - Fee bps unchanged after failed re-init
//! - Fee recipient unchanged after failed re-init

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_reinitialization_attempt_reverts_without_modifying_state() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin_a = Address::generate(&env);
    let fee_bps_a = 300u32;

    // 1. Initialize the contract with admin A and fee bps 300
    client.initialize(&admin_a, &fee_bps_a);

    // Assert initial state is set as expected
    assert_eq!(
        client.get_protocol_admin(),
        Some(admin_a.clone()),
        "admin must be set to admin A"
    );
    assert_eq!(
        client.get_protocol_fee_bps(),
        fee_bps_a,
        "protocol fee bps must be 300"
    );
    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(admin_a.clone()),
        "fee recipient must default to admin A"
    );
    assert!(
        client.is_protocol_config_initialized(),
        "protocol config must be marked initialized"
    );

    let config_a = client.get_fee_config().unwrap();
    assert_eq!(config_a.creator_bps, 10_000 - fee_bps_a);
    assert_eq!(config_a.protocol_bps, fee_bps_a);

    // 2. Attempt to re-initialize with admin B and fee bps 500
    let admin_b = Address::generate(&env);
    let fee_bps_b = 500u32;
    let result = client.try_initialize(&admin_b, &fee_bps_b);

    // 3. Acceptance criterion: Re-initialization reverts with already-initialized error code
    assert_eq!(
        result,
        Err(Ok(ContractError::AlreadyInitialized)),
        "re-initialization call must revert with AlreadyInitialized error code"
    );

    // 4. Acceptance criterion: Admin unchanged after failed re-init
    assert_eq!(
        client.get_protocol_admin(),
        Some(admin_a.clone()),
        "admin must remain unchanged after failed re-init"
    );

    // 5. Acceptance criterion: Fee bps unchanged after failed re-init
    assert_eq!(
        client.get_protocol_fee_bps(),
        fee_bps_a,
        "fee bps must remain unchanged after failed re-init"
    );

    // 6. Acceptance criterion: Fee recipient unchanged after failed re-init
    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(admin_a.clone()),
        "fee recipient must remain unchanged after failed re-init"
    );

    // Confirm fee config structure is also unchanged
    let config_after = client.get_fee_config().unwrap();
    assert_eq!(config_after.creator_bps, 10_000 - fee_bps_a);
    assert_eq!(config_after.protocol_bps, fee_bps_a);
    assert!(client.is_protocol_config_initialized());
}

#[test]
fn test_reinitialization_preserves_custom_fee_recipient() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin_a = Address::generate(&env);
    let custom_recipient = Address::generate(&env);
    let fee_bps_a = 300u32;

    // Initialize with admin A and fee bps 300
    client.initialize(&admin_a, &fee_bps_a);

    // Admin A updates fee recipient to a custom address
    client.set_protocol_fee_recipient(&admin_a, &custom_recipient);
    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(custom_recipient.clone())
    );

    // Attempt re-initialization with admin B and fee bps 500
    let admin_b = Address::generate(&env);
    let fee_bps_b = 500u32;
    let result = client.try_initialize(&admin_b, &fee_bps_b);

    assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));

    // Admin, fee bps, and custom fee recipient must all remain unchanged
    assert_eq!(
        client.get_protocol_admin(),
        Some(admin_a),
        "admin must remain unchanged"
    );
    assert_eq!(
        client.get_protocol_fee_bps(),
        fee_bps_a,
        "fee bps must remain unchanged"
    );
    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(custom_recipient),
        "custom fee recipient must remain unchanged after failed re-init"
    );
}

#[test]
fn test_reinitialization_with_same_params_also_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin_a = Address::generate(&env);
    let fee_bps_a = 300u32;

    // Initialize
    client.initialize(&admin_a, &fee_bps_a);

    // Attempt re-initialization with identical parameters
    let result = client.try_initialize(&admin_a, &fee_bps_a);
    assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));

    assert_eq!(client.get_protocol_admin(), Some(admin_a.clone()));
    assert_eq!(client.get_protocol_fee_bps(), fee_bps_a);
    assert_eq!(client.get_protocol_fee_recipient(), Some(admin_a));
}

#[test]
fn test_already_initialized_error_code_numeric_value() {
    assert_eq!(ContractError::AlreadyInitialized as u32, 110);
    assert_eq!(ContractError::AlreadyInitialised as u32, 110);
}
