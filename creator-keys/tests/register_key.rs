//! Tests for admin-authorised `register_key`: standard and auction-mode
//! registration, stored configuration, the `KeyRegistered` event, and
//! rejection of unauthorised callers.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::events::{KeyRegisteredEvent, KEY_REGISTERED_EVENT_NAME};
use creator_keys::{ContractError, CreatorKeysContractClient, CurvePreset, KeyMetadata};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol,
};

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    (client, admin)
}

fn metadata(env: &Env) -> KeyMetadata {
    KeyMetadata {
        name: String::from_str(env, "Alice Key"),
        bio: String::from_str(env, "bio"),
        avatar_uri: String::from_str(env, "ipfs://avatar"),
    }
}

fn registered_events(env: &Env) -> soroban_sdk::Vec<KeyRegisteredEvent> {
    let mut found = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == KEY_REGISTERED_EVENT_NAME {
            found.push_back(data.into_val(env));
        }
    }
    found
}

#[test]
fn test_standard_registration_initialises_key_config() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);

    client.register_key(
        &admin,
        &creator,
        &String::from_str(&env, "alice"),
        &metadata(&env),
        &CurvePreset::Quadratic,
        &12,
        &false,
    );
    // `env.events()` only holds the last invocation's events.
    let events = registered_events(&env);

    let profile = client.get_creator(&creator);
    assert_eq!(profile.supply, 0);
    assert_eq!(profile.holder_count, 0);
    assert_eq!(client.get_curve_preset(&creator), CurvePreset::Quadratic);
    assert_eq!(client.get_buy_cooldown(&creator), 12);
    assert!(!client.is_auction_pending(&creator));
    assert_eq!(client.get_key_metadata(&creator), Some(metadata(&env)));

    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.key_id, creator);
    assert_eq!(event.creator, creator);
    assert!(!event.auction_pending);
}

#[test]
fn test_auction_mode_registration_sets_auction_pending() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);

    client.register_key(
        &admin,
        &creator,
        &String::from_str(&env, "alice"),
        &metadata(&env),
        &CurvePreset::Linear,
        &0,
        &true,
    );
    // `env.events()` only holds the last invocation's events.
    let events = registered_events(&env);

    assert!(client.is_auction_pending(&creator));
    assert_eq!(client.get_creator(&creator).supply, 0);
    assert_eq!(events.len(), 1);
    assert!(events.get(0).unwrap().auction_pending);
}

#[test]
fn test_non_admin_caller_is_rejected() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let creator = Address::generate(&env);
    let outsider = Address::generate(&env);

    let result = client.try_register_key(
        &outsider,
        &creator,
        &String::from_str(&env, "alice"),
        &metadata(&env),
        &CurvePreset::Linear,
        &0,
        &false,
    );
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    assert!(!client.is_creator_registered(&creator));
}

#[test]
fn test_duplicate_registration_is_rejected() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);

    client.register_key(
        &admin,
        &creator,
        &String::from_str(&env, "alice"),
        &metadata(&env),
        &CurvePreset::Linear,
        &0,
        &false,
    );
    let result = client.try_register_key(
        &admin,
        &creator,
        &String::from_str(&env, "alice"),
        &metadata(&env),
        &CurvePreset::Linear,
        &0,
        &false,
    );
    assert_eq!(result, Err(Ok(ContractError::AlreadyRegistered)));
}
