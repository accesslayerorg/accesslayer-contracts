//! Integration tests for the read-only `get_protocol_status` view function.
//!
//! Covers the `GET /protocol/status` data source: `global_trading_paused`,
//! `protocol_fee_bps`, `treasury_address`, `lockup_duration_seconds` and
//! `min_investment_amount`, all read from persistent config storage.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, test_env_with_auths,
};
use creator_keys::constants::storage;
use creator_keys::{CreatorKeysContractClient, ProtocolStatus, DEFAULT_LOCKUP_DURATION_SECS};
use soroban_sdk::testutils::storage::Persistent as _;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{vec, Address, Env, Vec};

/// Returns a fresh contract client with a protocol admin configured.
fn deploy(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    (client, admin)
}

#[test]
fn defaults_when_storage_unconfigured() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let status = client.get_protocol_status();
    assert_eq!(
        status,
        ProtocolStatus {
            global_trading_paused: false,
            protocol_fee_bps: 0,
            treasury_address: None,
            lockup_duration_seconds: DEFAULT_LOCKUP_DURATION_SECS,
            min_investment_amount: None,
        }
    );
}

#[test]
fn all_values_returned_correctly_when_configured() {
    let env = test_env_with_auths();
    let (client, admin) = deploy(&env);

    let treasury = Address::generate(&env);
    client.set_protocol_fee(&admin, &Some(250), &treasury);
    client.set_lockup_duration(&admin, &3600);
    client.set_min_investment_amount(&admin, &5000);

    let status = client.get_protocol_status();
    assert_eq!(status.global_trading_paused, false);
    assert_eq!(status.protocol_fee_bps, 250);
    assert_eq!(status.treasury_address, Some(treasury.clone()));
    assert_eq!(status.lockup_duration_seconds, 3600);
    assert_eq!(status.min_investment_amount, Some(5000));

    assert_eq!(client.get_treasury_address(), Some(treasury));
    assert_eq!(client.get_lockup_duration(), 3600);
    assert_eq!(client.get_min_investment_amount(), Some(5000));
}

#[test]
fn global_trading_paused_tracks_global_pause_lifecycle() {
    let env = test_env_with_auths();
    let (client, admin) = deploy(&env);

    let signers = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    client.set_global_pause_admins(
        &admin,
        &vec![
            &env,
            signers[0].clone(),
            signers[1].clone(),
            signers[2].clone(),
        ],
    );

    assert_eq!(client.get_protocol_status().global_trading_paused, false);

    client.global_pause(&signers[0]);
    assert_eq!(client.get_protocol_status().global_trading_paused, false);

    client.global_pause(&signers[1]);
    assert_eq!(client.get_protocol_status().global_trading_paused, true);

    client.global_resume(&signers[0]);
    assert_eq!(client.get_protocol_status().global_trading_paused, true);

    client.global_resume(&signers[1]);
    assert_eq!(client.get_protocol_status().global_trading_paused, false);
}

#[test]
fn bumps_ttl_of_every_read_config_entry() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    let treasury = Address::generate(&env);
    client.set_protocol_fee(&admin, &Some(150), &treasury);
    client.set_lockup_duration(&admin, &1800);
    client.set_min_investment_amount(&admin, &2500);

    let keys = [
        storage::PROTOCOL_FEE_BPS,
        storage::TREASURY_ADDRESS,
        storage::LOCKUP_DURATION_SECS,
        storage::MIN_INVESTMENT_AMOUNT,
    ];
    let ttl_before: Vec<u32> = keys
        .iter()
        .map(|k| env.as_contract(&contract_id, || env.storage().persistent().get_ttl(k)))
        .collect();

    env.ledger().set_sequence_number(100_000);

    client.get_protocol_status();

    for (k, before) in keys.iter().zip(ttl_before.iter()) {
        let after = env.as_contract(&contract_id, || env.storage().persistent().get_ttl(k));
        assert!(
            after >= *before,
            "expected get_protocol_status to bump TTL of {k:?} (before={before}, after={after})"
        );
    }
}

#[test]
fn does_not_panic_on_empty_storage() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let status = client.get_protocol_status();
    assert_eq!(status.global_trading_paused, false);
    assert_eq!(status.protocol_fee_bps, 0);
    assert_eq!(status.treasury_address, None);
    assert_eq!(status.lockup_duration_seconds, DEFAULT_LOCKUP_DURATION_SECS);
    assert_eq!(status.min_investment_amount, None);
}

#[test]
fn requires_no_authentication() {
    let env = Env::default();
    let (client, _) = register_creator_keys(&env);

    let status = client.get_protocol_status();
    assert_eq!(status.protocol_fee_bps, 0);
}
