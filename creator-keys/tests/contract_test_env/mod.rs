//! Shared setup helpers for `creator-keys` integration tests.
//!
//! Compose the small functions here instead of one monolithic setup so each test
//! can opt in only to what it needs (pricing without fees, fees, registered creators, etc.).
//!
//! Not every integration-test binary uses every helper; this crate is compiled once per
//! `tests/*.rs` target, so we allow dead code at module scope.
#![allow(dead_code)]

use creator_keys::{constants, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

/// Stable timestamp used by integration tests unless a test needs to override it.
pub const DEFAULT_TEST_TIMESTAMP: u64 = 1_700_000_000;

/// Sets ledger timestamp to a deterministic value for reproducible test snapshots.
pub fn set_test_timestamp(env: &Env, timestamp: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp = timestamp;
    env.ledger().set(ledger);
}

/// Default [`Env`] for tests: enables mocked authorization for authed entrypoints.
pub fn test_env_with_auths() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

/// Register [`CreatorKeysContract`] and return a client and the contract id.
pub fn register_creator_keys<'a>(env: &'a Env) -> (CreatorKeysContractClient<'a>, Address) {
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &id);
    (client, id)
}

/// Admin sets a positive key price. Returns the admin address used.
pub fn set_key_price_for_tests(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    key_price: i128,
) -> Address {
    let admin = Address::generate(env);
    client.set_key_price(&admin, &key_price);
    admin
}

/// Set global fee split. Returns the admin address used.
pub fn set_protocol_fee_bps(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    creator_bps: u32,
    protocol_bps: u32,
) -> Address {
    let admin = Address::generate(env);
    client.set_fee_config(&admin, &creator_bps, &protocol_bps);
    admin
}

/// Set key price and fee config using the same admin (typical for quote and fee tests).
pub fn set_pricing_and_fees(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    key_price: i128,
    creator_bps: u32,
    protocol_bps: u32,
) -> Address {
    let admin = Address::generate(env);
    client.set_key_price(&admin, &key_price);
    client.set_fee_config(&admin, &creator_bps, &protocol_bps);
    admin
}

/// Register a new creator with the given display handle.
pub fn register_test_creator(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    handle: &str,
) -> Address {
    let creator = Address::generate(env);
    client.register_creator(&creator, &String::from_str(env, handle));
    creator
}

/// Write the persistent key price directly (bypassing `set_key_price`), for state edge cases.
pub fn set_stored_key_price(env: &Env, contract_id: &Address, price: i128) {
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .set(&constants::storage::KEY_PRICE, &price);
    });
}
