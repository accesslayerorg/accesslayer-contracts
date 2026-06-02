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
use std::string::String as StdString;

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

/// Return a deterministic test wallet address for a seed string.
pub fn test_wallet_address(env: &Env, seed: &str) -> Address {
    Address::from_string(&String::from_str(env, &account_strkey_from_seed(seed)))
}

/// Return a deterministic test wallet address for an index.
pub fn test_wallet_address_from_index(env: &Env, index: u32) -> Address {
    test_wallet_address(env, &std::format!("wallet-{index}"))
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

fn account_strkey_from_seed(seed: &str) -> StdString {
    let mut payload = [0u8; 32];
    let mut state = 0xcbf2_9ce4_8422_2325u64;

    for byte in seed.as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }

    for chunk in payload.chunks_mut(8) {
        state ^= state >> 33;
        state = state.wrapping_mul(0xff51_afd7_ed55_8ccd);
        state ^= state >> 33;
        state = state.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        state ^= state >> 33;
        chunk.copy_from_slice(&state.to_be_bytes());
    }

    let mut raw = [0u8; 35];
    raw[0] = 6 << 3;
    raw[1..33].copy_from_slice(&payload);
    let checksum = crc16_xmodem(&raw[..33]).to_le_bytes();
    raw[33..].copy_from_slice(&checksum);

    base32_encode(&raw)
}

fn crc16_xmodem(bytes: &[u8]) -> u16 {
    let mut crc = 0u16;

    for byte in bytes {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}

fn base32_encode(bytes: &[u8]) -> StdString {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

    let mut encoded = StdString::new();
    let mut buffer = 0u16;
    let mut bits = 0u8;

    for byte in bytes {
        buffer = (buffer << 8) | u16::from(*byte);
        bits += 8;

        while bits >= 5 {
            bits -= 5;
            let index = ((buffer >> bits) & 0x1f) as usize;
            encoded.push(ALPHABET[index] as char);
        }

        if bits > 0 {
            buffer &= (1 << bits) - 1;
        } else {
            buffer = 0;
        }
    }

    if bits > 0 {
        let index = ((buffer << (5 - bits)) & 0x1f) as usize;
        encoded.push(ALPHABET[index] as char);
    }

    encoded
}
