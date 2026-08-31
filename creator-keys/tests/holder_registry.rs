//! Integration tests for the append-only holder registry (#831).
//!
//! The `holder_registry` is a `Vec<Address>` persisted per key_id (the creator
//! address) that records every wallet that has ever held a key for a creator.
//! Addresses are appended on first acquisition (buy, transfer-in) and never
//! removed even when the holder's balance later reaches zero, preserving
//! historical accuracy for snapshot and airdrop entrypoints.
//!
//! The companion read-views `has_ever_held`, `get_holder_registry` and
//! `get_historical_holder_count` all derive from the same registry.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_key_price_for_tests};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

const KEY_PRICE: i128 = 100;

fn setup(env: &Env) -> (creator_keys::CreatorKeysContractClient<'_>, Address) {
    let (client, _contract_id) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

fn registry_contains(registry: &Vec<Address>, wallet: &Address) -> bool {
    registry.contains(wallet.clone())
}

/// Registry starts empty for a freshly registered creator and for unknown keys.
#[test]
fn registry_starts_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);

    let wallet = Address::generate(&env);
    assert!(
        client.get_holder_registry(&creator).len() == 0,
        "empty registry before any buys"
    );
    assert!(!client.has_ever_held(&creator, &wallet));
    assert_eq!(client.get_historical_holder_count(&creator), 0);

    let unknown_key = Address::generate(&env);
    assert!(
        client.get_holder_registry(&unknown_key).len() == 0,
        "unregistered key returns an empty registry"
    );
}

/// New buyer's address is appended to the registry on first purchase.
#[test]
fn first_buy_appends_buyer() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);

    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);

    client.buy_key(&creator, &buyer_a, &KEY_PRICE, &None);
    let registry = client.get_holder_registry(&creator);
    assert_eq!(registry.len(), 1);
    assert!(registry_contains(&registry, &buyer_a));
    assert!(client.has_ever_held(&creator, &buyer_a));
    assert_eq!(client.get_historical_holder_count(&creator), 1);

    client.buy_key(&creator, &buyer_b, &KEY_PRICE, &None);
    let registry = client.get_holder_registry(&creator);
    assert_eq!(registry.len(), 2);
    assert!(registry_contains(&registry, &buyer_b));
}

/// Address is not duplicated on subsequent buys by the same wallet, and remains
/// even after the wallet sells all its keys (historical accuracy).
#[test]
fn repeat_buys_do_not_duplicate_and_sell_does_not_remove() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);

    let wallet = Address::generate(&env);

    client.buy_key(&creator, &wallet, &KEY_PRICE, &None);
    client.buy_key(&creator, &wallet, &KEY_PRICE, &None);
    client.buy_key(&creator, &wallet, &KEY_PRICE, &None);

    let registry = client.get_holder_registry(&creator);
    assert_eq!(
        registry.len(),
        1,
        "a wallet buying three keys is still recorded only once"
    );
    assert_eq!(client.get_historical_holder_count(&creator), 1);

    // Full exit: the wallet's balance reaches zero but the registry entry stays.
    client.sell_key(&creator, &wallet, &None);
    client.sell_key(&creator, &wallet, &None);
    client.sell_key(&creator, &wallet, &None);

    let registry = client.get_holder_registry(&creator);
    assert_eq!(
        registry.len(),
        1,
        "registry is never shrunk after a full exit"
    );
    assert!(
        client.has_ever_held(&creator, &wallet),
        "has_ever_held remains true after a full exit"
    );
    assert_eq!(client.get_historical_holder_count(&creator), 1);
}

/// Transfer recipient address is appended on transfer-in if not already present.
#[test]
fn transfer_recipient_is_appended_to_registry() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    client.transfer_keys(&creator, &sender, &recipient, &1);

    let registry = client.get_holder_registry(&creator);
    assert_eq!(registry.len(), 2);
    assert!(registry_contains(&registry, &sender));
    assert!(registry_contains(&registry, &recipient));
    assert!(client.has_ever_held(&creator, &recipient));
    assert_eq!(client.get_historical_holder_count(&creator), 2);
}

/// A recipient that is already part of the registry is not appended twice.
#[test]
fn transfer_to_existing_holder_does_not_duplicate() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Both are already holders.
    client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    client.buy_key(&creator, &recipient, &KEY_PRICE, &None);

    client.transfer_keys(&creator, &sender, &recipient, &1);

    let registry = client.get_holder_registry(&creator);
    assert_eq!(
        registry.len(),
        2,
        "existing recipient must not be duplicated"
    );
    assert_eq!(client.get_historical_holder_count(&creator), 2);
}

/// The registry membership is stable after a full exit and a later re-entry.
#[test]
fn re_entry_after_full_exit_does_not_duplicate() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);

    let wallet = Address::generate(&env);

    client.buy_key(&creator, &wallet, &KEY_PRICE, &None);
    client.sell_key(&creator, &wallet, &None);
    client.buy_key(&creator, &wallet, &KEY_PRICE, &None);

    let registry = client.get_holder_registry(&creator);
    assert_eq!(
        registry.len(),
        1,
        "re-entry after a full exit is not a new holder"
    );
    assert_eq!(client.get_historical_holder_count(&creator), 1);
}
