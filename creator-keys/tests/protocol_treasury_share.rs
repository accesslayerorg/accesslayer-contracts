//! Unit tests for the `get_protocol_treasury_share_bps` read-only method.
//!
//! Covers default, configured, and invalid-state scenarios with explicit
//! assertions on basis-point units (1 bps = 0.01%).

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

/// Default scenario: protocol treasury share bps before any configuration.
///
/// When fee configuration is not set, the method returns FeeConfigNotSet error
/// rather than a default zero value.
#[test]
fn test_get_protocol_treasury_share_bps_returns_error_when_unconfigured() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let result = client.try_get_protocol_treasury_share_bps();
    assert_eq!(result, Err(Ok(ContractError::FeeConfigNotSet)));
}

/// Configured scenario: protocol treasury share bps after valid configuration.
///
/// After setting a valid fee configuration, the method returns the exact
/// protocol BPS value configured (in basis points, where 10000 = 100%).
#[test]
fn test_get_protocol_treasury_share_bps_returns_configured_value() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.set_fee_config(&admin, &8000u32, &2000u32);

    // Protocol treasury share = 2000 bps = 20%
    assert_eq!(client.get_protocol_treasury_share_bps(), 2000u32);
}

/// Multiple configurations: protocol treasury share bps tracks config changes.
///
/// After updating fee configuration multiple times, the method always returns
/// the current configured protocol BPS value, confirming the method reads
/// live state and is not cached.
#[test]
fn test_get_protocol_treasury_share_bps_tracks_configuration_updates() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    // First configuration: 2000 bps protocol share
    client.set_fee_config(&admin, &8000u32, &2000u32);
    assert_eq!(client.get_protocol_treasury_share_bps(), 2000u32);

    // Update to 3000 bps protocol share
    client.set_fee_config(&admin, &7000u32, &3000u32);
    assert_eq!(client.get_protocol_treasury_share_bps(), 3000u32);

    // Update to maximum valid: 5000 bps (50%)
    client.set_fee_config(&admin, &5000u32, &5000u32);
    assert_eq!(client.get_protocol_treasury_share_bps(), 5000u32);
}

/// Read-only verification: repeated calls return same value without side effects.
///
/// Multiple consecutive calls to the method must return identical values,
/// confirming it does not mutate state and is truly read-only.
#[test]
fn test_get_protocol_treasury_share_bps_is_read_only() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.set_fee_config(&admin, &7000u32, &3000u32);

    // Protocol treasury share = 3000 bps = 30%
    let call1 = client.get_protocol_treasury_share_bps();
    let call2 = client.get_protocol_treasury_share_bps();
    let call3 = client.get_protocol_treasury_share_bps();

    assert_eq!(call1, 3000u32);
    assert_eq!(call2, 3000u32);
    assert_eq!(call3, 3000u32);
}

/// Explicit units test: verify basis point representation (1 bps = 0.01%).
///
/// Tests that the returned value represents basis points where:
/// - 0 bps = 0%
/// - 100 bps = 1%
/// - 1000 bps = 10%
/// - 10000 bps = 100% (maximum possible)
#[test]
fn test_get_protocol_treasury_share_bps_explicit_basis_point_units() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    // Test: 100 bps = 1% protocol share
    client.set_fee_config(&admin, &9900u32, &100u32);
    assert_eq!(client.get_protocol_treasury_share_bps(), 100u32);

    // Test: 1000 bps = 10% protocol share
    client.set_fee_config(&admin, &9000u32, &1000u32);
    assert_eq!(client.get_protocol_treasury_share_bps(), 1000u32);

    // Test: 5000 bps = 50% protocol share (maximum)
    client.set_fee_config(&admin, &5000u32, &5000u32);
    assert_eq!(client.get_protocol_treasury_share_bps(), 5000u32);
}
