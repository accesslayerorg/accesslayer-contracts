//! Unit tests for the `get_creator` view returning all creator fields correctly
//! after registration.
//!
//! `get_creator` returns a [`CreatorProfile`] containing:
//! - `creator`: the creator's address
//! - `handle`: display name set at registration
//! - `supply`: total key supply (0 at registration)
//! - `holder_count`: number of unique holders (0 at registration)
//! - `fee_recipient`: address designated to receive creator fees (defaults to
//!   the creator's own address)
//! - `registered_at`: ledger sequence number captured at registration time
//!
//! Each test below asserts a specific field or group of fields to satisfy the
//! acceptance criteria defined in issue #677.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_ledger_sequence, test_env_with_auths};
use creator_keys::CreatorKeysContract;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

// ---------------------------------------------------------------------------
// Acceptance criteria: all fields present and correct after registration
// ---------------------------------------------------------------------------

#[test]
fn test_get_creator_returns_all_fields_correctly_after_registration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    // Pin the ledger so registered_at is deterministic.
    set_ledger_sequence(&env, 42);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: handle.clone(),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let profile = client.get_creator(&creator);

    // Display name matches registered value
    assert_eq!(
        profile.handle, handle,
        "handle must match the value provided at registration"
    );

    // Fee recipient defaults to the creator's own address
    assert_eq!(
        profile.fee_recipient, creator,
        "fee_recipient must default to the creator address when not explicitly set"
    );

    // Total supply is 0 at registration
    assert_eq!(
        profile.supply, 0,
        "total supply must be 0 for a newly registered creator"
    );

    // Holder count is 0 at registration
    assert_eq!(
        profile.holder_count, 0,
        "holder count must be 0 for a newly registered creator"
    );

    // registered_at matches the ledger sequence at registration time
    assert_eq!(
        profile.registered_at, 42,
        "registered_at must equal the ledger sequence at registration time"
    );

    // Creator address is echoed back correctly
    assert_eq!(
        profile.creator, creator,
        "creator address must be echoed back correctly"
    );
}

#[test]
fn test_get_creator_display_name_matches_registered_value() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: handle.clone(),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let profile = client.get_creator(&creator);

    assert_eq!(
        profile.handle, handle,
        "display name (handle) must match the value registered"
    );
    assert_eq!(
        profile.handle,
        String::from_str(&env, "alice"),
        "display name must be the exact string provided at registration"
    );
}

#[test]
fn test_get_creator_fee_recipient_matches_registered_address() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "bob"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let profile = client.get_creator(&creator);

    // Fee recipient defaults to the creator's own address
    assert_eq!(
        profile.fee_recipient, creator,
        "fee_recipient must match the creator's own address by default"
    );
}

#[test]
fn test_get_creator_total_supply_is_zero_at_registration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "carol"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let profile = client.get_creator(&creator);

    assert_eq!(
        profile.supply, 0,
        "total key supply must be 0 at registration"
    );
}

#[test]
fn test_get_creator_holder_count_is_zero_at_registration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "dave"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let profile = client.get_creator(&creator);

    assert_eq!(
        profile.holder_count, 0,
        "holder count must be 0 at registration (no keys have been bought yet)"
    );
}

#[test]
fn test_get_creator_registered_at_matches_ledger_sequence() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);

    // Pin ledger sequence to a known value before registering.
    set_ledger_sequence(&env, 99);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "eve"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let profile = client.get_creator(&creator);

    assert_eq!(
        profile.registered_at, 99,
        "registered_at must equal the ledger sequence at the time of registration"
    );
}

#[test]
fn test_get_creator_registered_at_is_immutable_after_buy_and_sell() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_ledger_sequence(&env, 100);
    let creator = Address::generate(&env);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "frank"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Advance the ledger and perform trades.
    set_ledger_sequence(&env, 200);
    let admin = Address::generate(&env);
    client.set_key_price(&admin, &500_i128);
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &500_i128, &None);
    client.sell_key(&creator, &buyer, &None);

    // registered_at must still reflect the original registration sequence.
    let profile = client.get_creator(&creator);
    assert_eq!(
        profile.registered_at, 100,
        "registered_at must not change after buy/sell mutations"
    );
    // Supply and holder count are updated by trades, but registered_at stays.
    assert_eq!(profile.supply, 0, "supply returned to 0 after sell");
}

// ---------------------------------------------------------------------------
// Edge case: unregistered creator returns Err(NotRegistered)
// ---------------------------------------------------------------------------

#[test]
fn test_get_creator_unregistered_returns_not_registered_error() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = creator_keys::CreatorKeysContractClient::new(&env, &contract_id);

    let unknown = Address::generate(&env);
    let result = client.try_get_creator(&unknown);

    assert_eq!(
        result,
        Err(Ok(creator_keys::ContractError::NotRegistered)),
        "get_creator must fail with NotRegistered for an unknown address"
    );
}

// ---------------------------------------------------------------------------
// Consistency: multiple reads return identical profiles
// ---------------------------------------------------------------------------

#[test]
fn test_get_creator_identical_across_consecutive_reads() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "grace");

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: handle.clone(),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Read multiple times with no intervening state changes.
    let r1 = client.get_creator(&creator);
    let r2 = client.get_creator(&creator);
    let r3 = client.get_creator(&creator);

    assert_eq!(r1.handle, r2.handle);
    assert_eq!(r2.handle, r3.handle);
    assert_eq!(r1.handle, handle);

    assert_eq!(r1.fee_recipient, r2.fee_recipient);
    assert_eq!(r2.fee_recipient, r3.fee_recipient);
    assert_eq!(r1.fee_recipient, creator);

    assert_eq!(r1.supply, r2.supply);
    assert_eq!(r2.supply, r3.supply);
    assert_eq!(r1.supply, 0);

    assert_eq!(r1.holder_count, r2.holder_count);
    assert_eq!(r2.holder_count, r3.holder_count);
    assert_eq!(r1.holder_count, 0);

    assert_eq!(r1.registered_at, r2.registered_at);
    assert_eq!(r2.registered_at, r3.registered_at);
}
