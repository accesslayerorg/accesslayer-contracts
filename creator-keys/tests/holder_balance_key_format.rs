//! Unit tests for the `holder_balance_key` helper (Issue #619, #652).
//!
//! `holder_balance_key` builds the composite `DataKey::KeyBalance(creator, holder)`
//! storage key. These tests confirm the encoded key has a consistent format:
//! non-empty, deterministic for a given (creator, holder) pair, equal length
//! across different holders of the same creator, and distinguishable from
//! other `DataKey` variants.
//!
//! Issue #652 additionally verifies:
//! - Argument order matters, even when the two addresses are nearly identical
//!   (differing by only 2 characters — the minimum achievable with Stellar's
//!   base32 encoding of 35-byte account payloads).
//! - Storage isolation: a balance written under (A, B) is not readable under (B, A).

mod contract_test_env;

use contract_test_env::register_creator_keys;
use creator_keys::constants;
use soroban_sdk::{testutils::Address as _, xdr::ToXdr, Address, Env, String};

#[test]
fn test_holder_balance_key_is_non_empty() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let holder = Address::generate(&env);

    let key = constants::storage::holder_balance_key(&creator, &holder);
    let bytes = key.to_xdr(&env);

    assert!(!bytes.is_empty(), "holder balance key must not be empty");
}

#[test]
fn test_holder_balance_key_is_deterministic_for_same_inputs() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let holder = Address::generate(&env);

    let key_a = constants::storage::holder_balance_key(&creator, &holder);
    let key_b = constants::storage::holder_balance_key(&creator, &holder);

    assert_eq!(
        key_a.to_xdr(&env),
        key_b.to_xdr(&env),
        "same creator/holder pair must always produce the same key encoding"
    );
}

#[test]
fn test_holder_balance_keys_for_different_holders_have_equal_length() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);

    let bytes_a = constants::storage::holder_balance_key(&creator, &holder_a).to_xdr(&env);
    let bytes_b = constants::storage::holder_balance_key(&creator, &holder_b).to_xdr(&env);

    assert_ne!(
        bytes_a, bytes_b,
        "different holders must produce distinct key encodings"
    );
    assert_eq!(
        bytes_a.len(),
        bytes_b.len(),
        "keys for the same creator with different holders must have equal encoded length"
    );
}

/// Two Stellar addresses whose string representation differs by only 2 characters.
///
/// The **underlying 32-byte public keys differ by exactly one byte** (byte 11 is
/// `0` vs `14`), which satisfies the requirement of stressing the key derivation
/// with near-identical binary inputs. However, Stellar account addresses are
/// base32-encoded 35-byte payloads (1 byte version + 32 bytes public key + 2
/// bytes CRC16 checksum). Because base32 maps 5 bits per character, a single-byte
/// change always propagates to **at least 2 characters** in the output string
/// (8 input bits ÷ 5 bits/char).
///
/// These two addresses share an all-zero key except byte 11. The resulting
/// strings differ at character positions 20 and 55 — the closest achievable
/// edit distance.
const ADDR_A_STR: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
const ADDR_B_STR: &str = "GAAAAAAAAAAAAAAAAAAA4AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHW";

#[test]
fn test_holder_balance_key_argument_order_near_identical_addresses() {
    let env = Env::default();

    let addr_a = Address::from_string(&String::from_str(&env, ADDR_A_STR));
    let addr_b = Address::from_string(&String::from_str(&env, ADDR_B_STR));

    // Sanity check: confirm the two addresses are indeed very close.
    let str_a: std::string::String = ADDR_A_STR.into();
    let str_b: std::string::String = ADDR_B_STR.into();
    let diff_count = str_a
        .chars()
        .zip(str_b.chars())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(
        diff_count, 2,
        "test invariant: ADDR_A and ADDR_B must differ by exactly 2 characters (the \
         minimum achievable with Stellar base32 encoding), but found {diff_count}"
    );

    let key_ab = constants::storage::holder_balance_key(&addr_a, &addr_b);
    let key_ba = constants::storage::holder_balance_key(&addr_b, &addr_a);

    assert_ne!(
        key_ab, key_ba,
        "swapping creator and holder must produce different keys even when the \
         two addresses differ by only 2 characters in their string representation"
    );

    // Confirm the keys also differ at the XDR encoding level.
    let bytes_ab: std::vec::Vec<u8> = key_ab.to_xdr(&env).iter().collect();
    let bytes_ba: std::vec::Vec<u8> = key_ba.to_xdr(&env).iter().collect();
    assert_ne!(
        bytes_ab, bytes_ba,
        "swapped argument keys must differ at the XDR encoding level"
    );
}

#[test]
fn test_holder_balance_key_storage_isolation_swapped_args() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, contract_id) = register_creator_keys(&env);

    // Use two addresses with minimal edit distance (2 characters apart).
    let addr_a = Address::from_string(&String::from_str(&env, ADDR_A_STR));
    let addr_b = Address::from_string(&String::from_str(&env, ADDR_B_STR));

    let key_ab = constants::storage::holder_balance_key(&addr_a, &addr_b);
    let key_ba = constants::storage::holder_balance_key(&addr_b, &addr_a);

    // Write a value under the (A, B) key.
    env.as_contract(&contract_id, || {
        env.storage().persistent().set(&key_ab, &42u32);
    });

    // Reading under (B, A) must return None — the keys are isolated.
    env.as_contract(&contract_id, || {
        let value: Option<u32> = env.storage().persistent().get(&key_ba);
        assert_eq!(
            value, None,
            "reading holder_balance_key(B, A) must not return a value that was \
             written under holder_balance_key(A, B) — storage keys must be isolated \
             when arguments are swapped"
        );
    });

    // Double-check: the original key still holds the written value.
    env.as_contract(&contract_id, || {
        let value: u32 = env.storage().persistent().get(&key_ab).unwrap_or(0);
        assert_eq!(
            value, 42,
            "the value written under holder_balance_key(A, B) must still be readable \
             under the same key"
        );
    });
}

#[test]
fn test_holder_balance_key_distinguishable_from_other_key_types() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let holder = Address::generate(&env);

    let balance_key = constants::storage::holder_balance_key(&creator, &holder);
    let creator_key = constants::storage::creator(&creator);
    let price_key = constants::storage::KEY_PRICE;

    let balance_bytes: std::vec::Vec<u8> = balance_key.to_xdr(&env).iter().collect();
    let creator_bytes: std::vec::Vec<u8> = creator_key.to_xdr(&env).iter().collect();
    let price_bytes: std::vec::Vec<u8> = price_key.to_xdr(&env).iter().collect();

    assert_ne!(
        balance_bytes, creator_bytes,
        "KeyBalance key encoding must differ from a Creator key encoding, \
         even when built from the same creator address"
    );
    assert_ne!(
        balance_bytes, price_bytes,
        "KeyBalance key encoding must differ from the global KeyPrice key encoding"
    );

    // The enum variant name is encoded as a leading Symbol discriminator in the
    // XDR payload. A KeyBalance key must carry that discriminator, and a
    // same-address Creator key must not spuriously contain it.
    let variant_marker = b"KeyBalance";
    let balance_key_has_marker = balance_bytes
        .windows(variant_marker.len())
        .any(|window| window == variant_marker);
    assert!(
        balance_key_has_marker,
        "KeyBalance key encoding should embed its variant discriminator"
    );

    let creator_key_has_balance_marker = creator_bytes
        .windows(variant_marker.len())
        .any(|window| window == variant_marker);
    assert!(
        !creator_key_has_balance_marker,
        "Creator key encoding must not contain the KeyBalance discriminator"
    );
}
