//! Integration tests for `get_creators_batch` and the `registered_at` field.
//!
//! Covers:
//! - `registered_at` is captured at registration time and returned by
//!   `get_creator_details` and `get_creators_batch`.
//! - Unregistered addresses in a batch return `is_registered: false` and
//!   `registered_at: 0` without panicking.
//! - Batch output length and order match the input slice exactly.
//! - Multiple creators registered at different ledger sequences carry
//!   independent `registered_at` values, enabling chronological sorting.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_ledger_sequence, test_env_with_auths,
};
use soroban_sdk::{testutils::Address as _, Address, Vec};

// ---------------------------------------------------------------------------
// registered_at — single-creator tests
// ---------------------------------------------------------------------------

#[test]
fn test_registered_at_is_captured_at_registration_sequence() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Pin the ledger sequence to a known value before registering.
    set_ledger_sequence(&env, 42);
    let creator = register_test_creator(&env, &client, "alice");

    let details = client.get_creator_details(&creator);
    assert!(details.is_registered);
    assert_eq!(
        details.registered_at, 42,
        "registered_at must equal the ledger sequence at registration time"
    );
}

#[test]
fn test_registered_at_is_zero_for_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let unknown = Address::generate(&env);
    let details = client.get_creator_details(&unknown);

    assert!(!details.is_registered);
    assert_eq!(
        details.registered_at, 0,
        "unregistered creator must return registered_at: 0"
    );
}

#[test]
fn test_registered_at_is_immutable_after_buy_and_sell() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_ledger_sequence(&env, 100);
    let creator = register_test_creator(&env, &client, "alice");

    // Advance the ledger sequence and perform state-mutating operations.
    set_ledger_sequence(&env, 200);
    let admin = Address::generate(&env);
    client.set_key_price(&admin, &500_i128);
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &500_i128);
    client.sell_key(&creator, &buyer);

    // registered_at must still reflect the original registration sequence.
    let details = client.get_creator_details(&creator);
    assert_eq!(
        details.registered_at, 100,
        "registered_at must not change after buy/sell mutations"
    );
}

#[test]
fn test_registered_at_is_read_only() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_ledger_sequence(&env, 77);
    let creator = register_test_creator(&env, &client, "alice");

    // Multiple reads must return the same value without mutating state.
    let r1 = client.get_creator_details(&creator).registered_at;
    let r2 = client.get_creator_details(&creator).registered_at;
    let r3 = client.get_creator_details(&creator).registered_at;

    assert_eq!(r1, 77);
    assert_eq!(r1, r2);
    assert_eq!(r2, r3);
}

// ---------------------------------------------------------------------------
// get_creators_batch — core behaviour
// ---------------------------------------------------------------------------

#[test]
fn test_batch_returns_empty_vec_for_empty_input() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let empty: Vec<Address> = Vec::new(&env);
    let results = client.get_creators_batch(&empty);

    assert_eq!(results.len(), 0, "empty input must produce empty output");
}

#[test]
fn test_batch_length_matches_input_length() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let alice = register_test_creator(&env, &client, "alice");
    let bob = register_test_creator(&env, &client, "bob");
    let unknown = Address::generate(&env);

    let mut input: Vec<Address> = Vec::new(&env);
    input.push_back(alice);
    input.push_back(bob);
    input.push_back(unknown);

    let results = client.get_creators_batch(&input);
    assert_eq!(
        results.len(),
        3,
        "output length must equal input length regardless of registration status"
    );
}

#[test]
fn test_batch_preserves_input_order() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_ledger_sequence(&env, 10);
    let alice = register_test_creator(&env, &client, "alice");
    set_ledger_sequence(&env, 20);
    let bob = register_test_creator(&env, &client, "bob");
    set_ledger_sequence(&env, 30);
    let carol = register_test_creator(&env, &client, "carol");

    let mut input: Vec<Address> = Vec::new(&env);
    input.push_back(alice.clone());
    input.push_back(bob.clone());
    input.push_back(carol.clone());

    let results = client.get_creators_batch(&input);

    assert_eq!(
        results.get(0).unwrap().creator,
        alice,
        "index 0 must be alice"
    );
    assert_eq!(results.get(1).unwrap().creator, bob, "index 1 must be bob");
    assert_eq!(
        results.get(2).unwrap().creator,
        carol,
        "index 2 must be carol"
    );
}

#[test]
fn test_batch_registered_creators_have_correct_fields() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_ledger_sequence(&env, 55);
    let alice = register_test_creator(&env, &client, "alice");

    let mut input: Vec<Address> = Vec::new(&env);
    input.push_back(alice.clone());

    let results = client.get_creators_batch(&input);
    let view = results.get(0).unwrap();

    assert!(view.is_registered);
    assert_eq!(view.creator, alice);
    assert_eq!(view.registered_at, 55);
    assert_eq!(view.supply, 0);
}

#[test]
fn test_batch_unregistered_address_returns_safe_defaults() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let unknown = Address::generate(&env);

    let mut input: Vec<Address> = Vec::new(&env);
    input.push_back(unknown.clone());

    let results = client.get_creators_batch(&input);
    let view = results.get(0).unwrap();

    assert!(
        !view.is_registered,
        "unregistered must have is_registered: false"
    );
    assert_eq!(view.creator, unknown, "creator address must be echoed back");
    assert_eq!(
        view.registered_at, 0,
        "unregistered must have registered_at: 0"
    );
    assert_eq!(view.supply, 0, "unregistered must have supply: 0");
}

#[test]
fn test_batch_mixed_registered_and_unregistered() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_ledger_sequence(&env, 11);
    let alice = register_test_creator(&env, &client, "alice");
    let unknown = Address::generate(&env);
    set_ledger_sequence(&env, 22);
    let bob = register_test_creator(&env, &client, "bob");

    let mut input: Vec<Address> = Vec::new(&env);
    input.push_back(alice.clone());
    input.push_back(unknown.clone());
    input.push_back(bob.clone());

    let results = client.get_creators_batch(&input);

    // alice — registered at sequence 11
    let v0 = results.get(0).unwrap();
    assert!(v0.is_registered);
    assert_eq!(v0.creator, alice);
    assert_eq!(v0.registered_at, 11);

    // unknown — not registered
    let v1 = results.get(1).unwrap();
    assert!(!v1.is_registered);
    assert_eq!(v1.creator, unknown);
    assert_eq!(v1.registered_at, 0);

    // bob — registered at sequence 22
    let v2 = results.get(2).unwrap();
    assert!(v2.is_registered);
    assert_eq!(v2.creator, bob);
    assert_eq!(v2.registered_at, 22);
}

// ---------------------------------------------------------------------------
// registered_at — chronological ordering invariant
// ---------------------------------------------------------------------------

#[test]
fn test_registered_at_enables_chronological_sort() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Register three creators at strictly increasing ledger sequences.
    set_ledger_sequence(&env, 100);
    let first = register_test_creator(&env, &client, "first");
    set_ledger_sequence(&env, 200);
    let second = register_test_creator(&env, &client, "second");
    set_ledger_sequence(&env, 300);
    let third = register_test_creator(&env, &client, "third");

    let mut input: Vec<Address> = Vec::new(&env);
    // Deliberately pass in reverse order to prove the client can re-sort by registered_at.
    input.push_back(third.clone());
    input.push_back(first.clone());
    input.push_back(second.clone());

    let results = client.get_creators_batch(&input);

    // Collect registered_at values in the order returned.
    let seq_third = results.get(0).unwrap().registered_at;
    let seq_first = results.get(1).unwrap().registered_at;
    let seq_second = results.get(2).unwrap().registered_at;

    assert_eq!(seq_third, 300);
    assert_eq!(seq_first, 100);
    assert_eq!(seq_second, 200);

    // A client sorting by registered_at ascending would produce: first, second, third.
    assert!(seq_first < seq_second);
    assert!(seq_second < seq_third);
}

#[test]
fn test_batch_matches_individual_get_creator_details_calls() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_ledger_sequence(&env, 15);
    let alice = register_test_creator(&env, &client, "alice");
    set_ledger_sequence(&env, 25);
    let bob = register_test_creator(&env, &client, "bob");

    let mut input: Vec<Address> = Vec::new(&env);
    input.push_back(alice.clone());
    input.push_back(bob.clone());

    let batch = client.get_creators_batch(&input);

    // Each batch entry must be identical to the corresponding single-address call.
    let alice_single = client.get_creator_details(&alice);
    let bob_single = client.get_creator_details(&bob);

    let alice_batch = batch.get(0).unwrap();
    let bob_batch = batch.get(1).unwrap();

    assert_eq!(alice_batch.creator, alice_single.creator);
    assert_eq!(alice_batch.handle, alice_single.handle);
    assert_eq!(alice_batch.supply, alice_single.supply);
    assert_eq!(alice_batch.is_registered, alice_single.is_registered);
    assert_eq!(alice_batch.registered_at, alice_single.registered_at);

    assert_eq!(bob_batch.creator, bob_single.creator);
    assert_eq!(bob_batch.handle, bob_single.handle);
    assert_eq!(bob_batch.supply, bob_single.supply);
    assert_eq!(bob_batch.is_registered, bob_single.is_registered);
    assert_eq!(bob_batch.registered_at, bob_single.registered_at);
}

// ---------------------------------------------------------------------------
// Acceptance-criteria test: multi-profile retrieval + defensive zero-state fallback
// ---------------------------------------------------------------------------

/// Verifies that `get_creators_batch` returns correct data for registered creators
/// and safe defaults for unregistered addresses in the same call.
///
/// This is the canonical acceptance-criteria test for the batch endpoint:
/// - Registered creators carry their real `handle`, `supply`, and `registered_at`.
/// - Unregistered addresses return `is_registered: false` and `registered_at: 0`
///   without causing the call to panic or error.
/// - Output length and order match the input slice exactly.
#[test]
fn test_get_creators_batch_success() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Register two creators at known ledger sequences.
    set_ledger_sequence(&env, 10);
    let alice = register_test_creator(&env, &client, "alice");

    set_ledger_sequence(&env, 20);
    let bob = register_test_creator(&env, &client, "bob");

    // A third address that is never registered — the defensive fallback case.
    let unknown = Address::generate(&env);

    // Build the input batch: registered, unregistered, registered.
    let mut input: Vec<Address> = Vec::new(&env);
    input.push_back(alice.clone());
    input.push_back(unknown.clone());
    input.push_back(bob.clone());

    let results = client.get_creators_batch(&input);

    // Output length must equal input length.
    assert_eq!(
        results.len(),
        3,
        "batch output length must match input length"
    );

    // --- alice (index 0): registered at sequence 10 ---
    let v_alice = results.get(0).unwrap();
    assert!(v_alice.is_registered, "alice must be registered");
    assert_eq!(v_alice.creator, alice, "alice address must be echoed");
    assert_eq!(v_alice.registered_at, 10, "alice registered_at must be 10");
    assert_eq!(v_alice.supply, 0, "alice supply must start at 0");

    // --- unknown (index 1): defensive zero-state fallback ---
    let v_unknown = results.get(1).unwrap();
    assert!(
        !v_unknown.is_registered,
        "unregistered address must have is_registered: false"
    );
    assert_eq!(
        v_unknown.creator, unknown,
        "unregistered creator address must be echoed back"
    );
    assert_eq!(
        v_unknown.registered_at, 0,
        "unregistered address must return registered_at: 0"
    );
    assert_eq!(
        v_unknown.supply, 0,
        "unregistered address must return supply: 0"
    );

    // --- bob (index 2): registered at sequence 20 ---
    let v_bob = results.get(2).unwrap();
    assert!(v_bob.is_registered, "bob must be registered");
    assert_eq!(v_bob.creator, bob, "bob address must be echoed");
    assert_eq!(v_bob.registered_at, 20, "bob registered_at must be 20");
    assert_eq!(v_bob.supply, 0, "bob supply must start at 0");

    // Chronological ordering invariant: alice registered before bob.
    assert!(
        v_alice.registered_at < v_bob.registered_at,
        "alice must have a lower registered_at than bob for chronological sorting"
    );
}
