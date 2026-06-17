//! Tests for the get_creator_details read-only method.

mod contract_test_env;

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env, String};

#[test]
fn test_get_creator_details_unregistered_returns_defaults() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = soroban_sdk::Address::generate(&env);

    let details = client.get_creator_details(&creator);
    assert!(!details.is_registered);
    assert_eq!(details.creator, creator);
    assert_eq!(details.handle, String::from_str(&env, ""));
    assert_eq!(details.supply, 0);
}

#[test]
fn test_get_creator_details_registered_returns_correct_data() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = soroban_sdk::Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle, &None);

    let details = client.get_creator_details(&creator);
    assert!(details.is_registered);
    assert_eq!(details.creator, creator);
    assert_eq!(details.handle, handle);
    assert_eq!(details.supply, 0);
}

#[test]
fn test_get_creator_details_updates_after_buy() {
    let env = contract_test_env::test_env_with_auths();
    let (client, _) = contract_test_env::register_creator_keys(&env);
    let creator = contract_test_env::register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    let initial_details = client.get_creator_details(&creator);
    assert!(initial_details.is_registered);
    assert_eq!(initial_details.creator, creator);
    assert_eq!(initial_details.handle, String::from_str(&env, "alice"));
    assert_eq!(initial_details.supply, 0);

    contract_test_env::set_key_price_for_tests(&env, &client, 100i128);
    client.buy_key(&creator, &buyer, &100i128);

    let updated_details = client.get_creator_details(&creator);
    assert!(updated_details.is_registered);
    assert_eq!(updated_details.creator, creator);
    assert_eq!(updated_details.handle, String::from_str(&env, "alice"));
    assert_eq!(updated_details.supply, 1);
}

/// Regression test: creator detail reads must reflect the latest persisted state
/// after a sequence of state mutations (buy followed by sell).
///
/// Ensures `get_creator_details` is not reading stale or cached values after
/// multiple state-changing operations on the same creator.
#[test]
fn test_get_creator_details_reflects_latest_state_after_buy_then_sell() {
    let env = contract_test_env::test_env_with_auths();
    let (client, _) = contract_test_env::register_creator_keys(&env);
    let creator = contract_test_env::register_test_creator(&env, &client, "bob");
    let buyer = soroban_sdk::Address::generate(&env);

    // Baseline: supply starts at zero
    let details_before = client.get_creator_details(&creator);
    assert!(details_before.is_registered);
    assert_eq!(details_before.supply, 0);
    assert_eq!(details_before.handle, String::from_str(&env, "bob"));

    // State mutation 1: buy a key — supply must increment
    contract_test_env::set_key_price_for_tests(&env, &client, 100i128);
    client.buy_key(&creator, &buyer, &100i128);

    let details_after_buy = client.get_creator_details(&creator);
    assert_eq!(
        details_after_buy.supply, 1,
        "supply must be 1 immediately after buy"
    );
    assert_eq!(details_after_buy.creator, creator);
    assert_eq!(details_after_buy.handle, String::from_str(&env, "bob"));

    // State mutation 2: sell the key back — supply must decrement
    client.sell_key(&creator, &buyer);

    let details_after_sell = client.get_creator_details(&creator);
    assert_eq!(
        details_after_sell.supply, 0,
        "supply must return to 0 after sell; stale reads indicate a caching bug"
    );
    assert!(
        details_after_sell.is_registered,
        "creator must remain registered after sell"
    );
    assert_eq!(details_after_sell.creator, creator);
    assert_eq!(details_after_sell.handle, String::from_str(&env, "bob"));
}
