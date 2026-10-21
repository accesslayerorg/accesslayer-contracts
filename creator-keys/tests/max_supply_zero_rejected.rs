//! Unit tests for a supply cap of zero at creator registration.
//!
//! Covers: a `Some(0)` supply cap is treated as unlimited (#997) and leaves cap
//! storage unwritten while `get_supply_info` reports cap `0` and unbounded
//! remaining, and `Some(1)` is accepted as the minimum meaningful cap.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn make_client(env: &Env) -> CreatorKeysContractClient<'_> {
    let id = env.register(CreatorKeysContract, ());
    CreatorKeysContractClient::new(env, &id)
}

// ---------------------------------------------------------------------------
// Some(0) reverts with NotPositiveAmount
// ---------------------------------------------------------------------------

#[test]
fn test_max_supply_zero_treated_as_unlimited_at_registration() {
    let env = Env::default();
    env.mock_all_auths();
    let client = make_client(&env);

    let creator = Address::generate(&env);
    // #997: a cap of 0 is normalized to "unlimited" and accepted.
    let result = client.try_register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &Some(0),
        &None,
        &None,
        &None,
        &None,
    );
    assert!(
        result.is_ok(),
        "max_supply: Some(0) must be accepted as unlimited (#997)"
    );
    assert!(
        client.is_creator_registered(&creator),
        "creator must be registered with max_supply: Some(0) as unlimited"
    );
}

// ---------------------------------------------------------------------------
// No creator state written after failed registration
// ---------------------------------------------------------------------------

#[test]
fn test_zero_supply_cap_leaves_cap_storage_unwritten() {
    let env = Env::default();
    env.mock_all_auths();
    let client = make_client(&env);

    let creator = Address::generate(&env);
    // #997: cap 0 registers as unlimited and writes no cap storage entry.
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &Some(0),
        &None,
        &None,
        &None,
        &None,
    );

    assert!(
        client.is_creator_registered(&creator),
        "creator must be registered with an unlimited cap"
    );

    // Max supply must not have been written for an uncapped key
    let stored_cap = client.get_max_supply(&creator);
    assert_eq!(
        stored_cap, None,
        "max_supply storage must stay empty for an unlimited (0) cap"
    );

    // get_supply_info reports cap 0 with unbounded remaining (#997)
    let info = client.get_supply_info(&creator);
    assert_eq!(info.supply, 0);
    assert_eq!(info.cap, 0);
    assert_eq!(info.remaining, u32::MAX);
}

// ---------------------------------------------------------------------------
// Some(1) accepted as the minimum valid cap
// ---------------------------------------------------------------------------

#[test]
fn test_max_supply_one_accepted_as_minimum() {
    let env = Env::default();
    env.mock_all_auths();
    let client = make_client(&env);

    let creator = Address::generate(&env);
    let result = client.try_register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &Some(1),
        &None,
        &None,
        &None,
        &None,
    );
    assert!(
        result.is_ok(),
        "max_supply: Some(1) must be accepted as the minimum valid cap"
    );
    assert!(
        client.is_creator_registered(&creator),
        "creator must be registered after successful registration with max_supply: Some(1)"
    );
    assert_eq!(
        client.get_max_supply(&creator),
        Some(1),
        "stored max_supply must equal 1"
    );
}

// ---------------------------------------------------------------------------
// None (no cap) is accepted
// ---------------------------------------------------------------------------

#[test]
fn test_max_supply_none_accepted_no_cap() {
    let env = Env::default();
    env.mock_all_auths();
    let client = make_client(&env);

    let creator = Address::generate(&env);
    let result = client.try_register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    assert!(result.is_ok(), "max_supply: None must be accepted (no cap)");
    assert!(client.is_creator_registered(&creator));
    assert_eq!(
        client.get_max_supply(&creator),
        None,
        "no max_supply must be stored when None is passed"
    );
}

// ---------------------------------------------------------------------------
// Some(2) and larger values accepted
// ---------------------------------------------------------------------------

#[test]
fn test_max_supply_two_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let client = make_client(&env);

    let creator = Address::generate(&env);
    let result = client.try_register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &Some(2),
        &None,
        &None,
        &None,
        &None,
    );
    assert!(result.is_ok(), "max_supply: Some(2) must be accepted");
    assert_eq!(client.get_max_supply(&creator), Some(2));
}

#[test]
fn test_max_supply_large_value_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let client = make_client(&env);

    let creator = Address::generate(&env);
    let result = client.try_register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &Some(1_000_000),
        &None,
        &None,
        &None,
        &None,
    );
    assert!(
        result.is_ok(),
        "max_supply: Some(1_000_000) must be accepted"
    );
    assert_eq!(client.get_max_supply(&creator), Some(1_000_000));
}
