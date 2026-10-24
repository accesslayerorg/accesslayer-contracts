//! Shared setup helpers for `key-insurance` integration tests.
#![allow(dead_code)]

use key_insurance::{KeyInsuranceContract, KeyInsuranceContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

/// Stable test key price used by helpers unless a test overrides it.
pub const DEFAULT_KEY_PRICE: i128 = 1_000;

/// Default coverage amount used across fixture helpers.
pub const DEFAULT_COVERAGE: i128 = 10_000;

/// Default threshold: payout when price drops below 50% of purchase price.
pub const DEFAULT_THRESHOLD_BPS: u32 = 5_000;

/// Default [`Env`] with mocked authorisation.
pub fn test_env_with_auths() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

/// Register [`KeyInsuranceContract`] and return a client and the contract id.
pub fn register_key_insurance(env: &Env) -> (KeyInsuranceContractClient<'_>, Address) {
    let id = env.register(KeyInsuranceContract, ());
    let client = KeyInsuranceContractClient::new(env, &id);
    (client, id)
}

/// Bootstrap: register contract, set admin, return (client, contract_id, admin).
pub fn setup_with_admin(env: &Env) -> (KeyInsuranceContractClient<'_>, Address, Address) {
    let (client, id) = register_key_insurance(env);
    let admin = Address::generate(env);
    client.set_admin(&admin);
    (client, id, admin)
}

/// Bootstrap with admin + a key price already set. Returns (client, id, admin, key_id).
pub fn setup_with_price(
    env: &Env,
    price: i128,
) -> (KeyInsuranceContractClient<'_>, Address, Address, Address) {
    let (client, id, admin) = setup_with_admin(env);
    let key_id = Address::generate(env);
    client.set_key_price(&admin, &key_id, &price);
    (client, id, admin, key_id)
}

/// Purchase a policy with default parameters; returns the wallet address used.
pub fn buy_default_policy(
    env: &Env,
    client: &KeyInsuranceContractClient<'_>,
    key_id: &Address,
) -> Address {
    let wallet = Address::generate(env);
    client.purchase_insurance(
        &wallet,
        key_id,
        &DEFAULT_COVERAGE,
        &DEFAULT_THRESHOLD_BPS,
    );
    wallet
}

/// Advance the ledger timestamp by `seconds`.
pub fn advance_time(env: &Env, seconds: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp = ledger.timestamp.saturating_add(seconds);
    env.ledger().set(ledger);
}
