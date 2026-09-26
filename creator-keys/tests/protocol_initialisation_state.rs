//! Tests for protocol initialisation state (accesslayerorg/accesslayer-contracts#723).
//!
//! There is no single `initialise` entrypoint: protocol initialisation is the
//! setter sequence `set_protocol_admin`, `set_key_price`, `set_fee_config`,
//! `set_treasury_address`. Verifies that:
//! - After a full initialisation sequence every state field reads back exactly
//!   what was set (protocol admin, fee config bps split, protocol fee bps,
//!   treasury address, key price) and `is_protocol_config_initialized` is true.
//! - Before any initialisation all views report unset defaults (`None` /
//!   `false`) and the key-price storage key is absent.
//! - Repeating the full setter sequence with identical values succeeds without
//!   error, leaves every observable field unchanged, keeps
//!   `is_protocol_config_initialized` true, and emits the `CONTRACT_INITIALIZED`
//!   event exactly once across both passes (idempotent re-initialisation).
//! - `is_protocol_config_initialized` reflects fee-config presence only: it
//!   stays false after `set_protocol_admin` alone, and becomes true after
//!   `set_fee_config` even while treasury and key price remain unset.

mod contract_test_env;

use contract_test_env::{assert_storage_absent, register_creator_keys, test_env_with_auths};
use creator_keys::{constants, events, ContractError, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

/// Creator share of the fee split used by these fixtures.
const CREATOR_BPS: u32 = 8_500;

/// Protocol share of the fee split used by these fixtures.
const PROTOCOL_BPS: u32 = 1_500;

/// Key price written during initialisation.
const KEY_PRICE: i128 = 123_456;

/// Runs the four-setter protocol initialisation sequence with the given admin.
fn run_initialisation(client: &CreatorKeysContractClient<'_>, admin: &Address, treasury: &Address) {
    client.set_protocol_admin(admin, admin);
    client.set_key_price(admin, &KEY_PRICE);
    client.set_fee_config(admin, &CREATOR_BPS, &PROTOCOL_BPS);
    client.set_treasury_address(admin, treasury);
}

/// Field-level view of the stored fee config. `FeeConfig` does not implement
/// `Debug`, so assertions compare its bps fields as a tuple instead.
fn fee_config_bps(client: &CreatorKeysContractClient<'_>) -> Option<(u32, u32)> {
    client
        .get_fee_config()
        .map(|config| (config.creator_bps, config.protocol_bps))
}

/// Counts `ContractInitializedEvent` records emitted by the most recent contract
/// invocation. `env.events().all()` exposes only the last invocation's events,
/// not a cumulative log, so callers must invoke the contract call under test
/// immediately before counting.
fn count_initialization_events(env: &Env) -> usize {
    env.events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(env);
                    name == events::CONTRACT_INITIALIZED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .count()
}

/// After the full init sequence every state field equals the value that was set.
#[test]
fn test_full_initialization_sets_all_state_fields() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    run_initialisation(&client, &admin, &treasury);

    assert_eq!(client.get_protocol_admin(), Some(admin.clone()));
    assert_eq!(
        fee_config_bps(&client),
        Some((CREATOR_BPS, PROTOCOL_BPS)),
        "fee config must store the initialised bps split"
    );
    assert_eq!(
        client.get_protocol_fee_bps(),
        PROTOCOL_BPS,
        "protocol fee bps must match the configured split"
    );
    assert_eq!(client.get_treasury_address(), Some(treasury));
    assert!(client.is_protocol_config_initialized());

    let price_reader = Address::generate(&env);
    assert_eq!(
        client.try_query_price(&price_reader, &0),
        Ok(Ok(KEY_PRICE)),
        "query_price must surface the initialised base key price"
    );

    let stored_price: Option<i128> = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
    });
    assert_eq!(
        stored_price,
        Some(KEY_PRICE),
        "KEY_PRICE storage must hold the initialised price"
    );
}

/// Before initialisation all views report their unset defaults and key price is absent.
#[test]
fn test_views_report_unset_defaults_before_initialization() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);

    assert_eq!(client.get_protocol_admin(), None);
    assert_eq!(fee_config_bps(&client), None);
    assert_eq!(client.get_treasury_address(), None);
    assert!(!client.is_protocol_config_initialized());

    assert_eq!(
        client.try_get_protocol_fee_bps(),
        Err(Ok(ContractError::FeeConfigNotSet)),
        "fee bps view must fail with FeeConfigNotSet before initialisation"
    );

    let reader = Address::generate(&env);
    assert_eq!(
        client.try_query_price(&reader, &0),
        Err(Ok(ContractError::KeyPriceNotSet)),
        "key price view must fail with KeyPriceNotSet before initialisation"
    );

    env.as_contract(&contract_id, || {
        assert_storage_absent(&env, &constants::storage::KEY_PRICE);
    });
}

/// Repeating the full init sequence with identical values succeeds, mutates nothing,
/// and emits CONTRACT_INITIALIZED exactly once across both passes. Note:
/// `env.events().all()` only exposes the events of the most recent contract
/// invocation, so each pass ends on its `set_fee_config` call before counting.
#[test]
fn test_repeated_initialization_is_idempotent_and_state_invariant() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Pass 1: fee config last so its initialization event is visible to all().
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &KEY_PRICE);
    client.set_treasury_address(&admin, &treasury);
    client.set_fee_config(&admin, &CREATOR_BPS, &PROTOCOL_BPS);

    assert_eq!(
        count_initialization_events(&env),
        1,
        "first initialisation must emit exactly one ContractInitialized event"
    );

    let admin_before = client.get_protocol_admin();
    let config_before = fee_config_bps(&client);
    let bps_before = client.get_protocol_fee_bps();
    let treasury_before = client.get_treasury_address();

    // Pass 2: repeat every setter with identical values; each must succeed.
    assert_eq!(
        client.try_set_protocol_admin(&admin, &admin),
        Ok(Ok(())),
        "re-setting the same admin must succeed"
    );
    assert_eq!(
        client.try_set_key_price(&admin, &KEY_PRICE),
        Ok(Ok(())),
        "re-setting the same key price must succeed"
    );
    client.set_treasury_address(&admin, &treasury);
    assert_eq!(
        client.try_set_fee_config(&admin, &CREATOR_BPS, &PROTOCOL_BPS),
        Ok(Ok(())),
        "re-setting the same fee config must succeed"
    );

    assert_eq!(client.get_protocol_admin(), admin_before);
    assert_eq!(fee_config_bps(&client), config_before);
    assert_eq!(client.get_protocol_fee_bps(), bps_before);
    assert_eq!(client.get_treasury_address(), treasury_before);
    assert!(client.is_protocol_config_initialized());
    assert_eq!(
        count_initialization_events(&env),
        0,
        "the idempotent second pass must not emit another init event"
    );
}

/// is_protocol_config_initialized tracks fee-config presence, not the other setters.
#[test]
fn test_is_protocol_config_initialized_reflects_fee_config_presence_only() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    assert!(!client.is_protocol_config_initialized());

    client.set_protocol_admin(&admin, &admin);
    assert!(
        !client.is_protocol_config_initialized(),
        "admin setup alone must not mark the protocol configuration initialized"
    );

    client.set_fee_config(&admin, &CREATOR_BPS, &PROTOCOL_BPS);
    assert!(client.is_protocol_config_initialized());

    assert_eq!(
        client.get_treasury_address(),
        None,
        "treasury may remain unset once the fee config exists"
    );
    let reader = Address::generate(&env);
    assert_eq!(
        client.try_query_price(&reader, &0),
        Err(Ok(ContractError::KeyPriceNotSet)),
        "key price may remain unset once the fee config exists"
    );
}
