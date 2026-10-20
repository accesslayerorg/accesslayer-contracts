//! Shared setup helpers for `key-spotlight` integration tests.
#![allow(dead_code)]

use key_spotlight::{KeySpotlightContract, KeySpotlightContractClient, SECS_PER_DAY};
use soroban_sdk::{testutils::Address as _, Address, Env};

/// Stable future offset used when constructing bid start times.
/// Far enough ahead that `start_time > now` always holds at test setup.
pub const FUTURE_OFFSET_SECS: u64 = SECS_PER_DAY * 7; // one week out

/// Default bid amount.
pub const DEFAULT_BID: i128 = 1_000;

/// Default spotlight duration.
pub const DEFAULT_DURATION_DAYS: u32 = 7;

/// Default [`Env`] with mocked authorisation.
pub fn test_env_with_auths() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

/// Register [`KeySpotlightContract`] and return a client and contract id.
pub fn register_key_spotlight(env: &Env) -> (KeySpotlightContractClient<'_>, Address) {
    let id = env.register(KeySpotlightContract, ());
    let client = KeySpotlightContractClient::new(env, &id);
    (client, id)
}

/// Bootstrap: register + set admin + set treasury.
/// Returns (client, id, admin, treasury).
pub fn setup_full(
    env: &Env,
) -> (KeySpotlightContractClient<'_>, Address, Address, Address) {
    let (client, id) = register_key_spotlight(env);
    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    client.set_admin(&admin);
    client.set_treasury(&admin, &treasury);
    (client, id, admin, treasury)
}

/// Return `now + FUTURE_OFFSET_SECS` as the start_time for a bid.
pub fn future_start(env: &Env) -> u64 {
    env.ledger().timestamp() + FUTURE_OFFSET_SECS
}

/// Place a bid with default parameters and return the key_id used.
pub fn place_default_bid(
    env: &Env,
    client: &KeySpotlightContractClient<'_>,
) -> (Address, u64) {
    let creator = Address::generate(env);
    let key_id = Address::generate(env);
    let start = future_start(env);
    client.place_spotlight_bid(&key_id, &creator, &start, &DEFAULT_DURATION_DAYS, &DEFAULT_BID);
    let slot_id = key_spotlight::day_floor(start);
    (key_id, slot_id)
}

/// Advance the ledger timestamp by `seconds`.
pub fn advance_time(env: &Env, seconds: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp = ledger.timestamp.saturating_add(seconds);
    env.ledger().set(ledger);
}
