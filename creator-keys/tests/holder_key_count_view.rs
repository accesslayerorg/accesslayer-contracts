//! Tests for the `get_holder_key_count` read-only view method.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup_with_creator(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let creator = Address::generate(env);
    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(env, "test"));
    (client, creator, admin)
}

#[test]
fn test_holder_key_count_view_starts_at_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, creator, _admin) = setup_with_creator(&env);
    let holder = Address::generate(&env);

    let view = client.get_holder_key_count(&creator, &holder);
    assert_eq!(view.key_count, 0);
    assert!(view.creator_exists);
    assert_eq!(view.creator, creator);
    assert_eq!(view.holder, holder);
}

#[test]
fn test_holder_key_count_view_increments_on_buy() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, creator, _admin) = setup_with_creator(&env);
    let holder = Address::generate(&env);

    // Initial state: zero keys
    let view = client.get_holder_key_count(&creator, &holder);
    assert_eq!(view.key_count, 0);

    // First purchase
    client.buy_key(&creator, &holder, &100i128, &None);
    let view = client.get_holder_key_count(&creator, &holder);
    assert_eq!(view.key_count, 1);
    assert!(view.creator_exists);

    // Second purchase
    client.buy_key(&creator, &holder, &100i128, &None);
    let view = client.get_holder_key_count(&creator, &holder);
    assert_eq!(view.key_count, 2);
}

#[test]
fn test_holder_key_count_view_multiple_holders() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, creator, _admin) = setup_with_creator(&env);
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);

    // Holder A buys 3 keys
    client.buy_key(&creator, &holder_a, &100i128, &None);
    client.buy_key(&creator, &holder_a, &100i128, &None);
    client.buy_key(&creator, &holder_a, &100i128, &None);

    // Holder B buys 1 key
    client.buy_key(&creator, &holder_b, &100i128, &None);

    // Verify holder A has 3 keys
    let view_a = client.get_holder_key_count(&creator, &holder_a);
    assert_eq!(view_a.key_count, 3);
    assert!(view_a.creator_exists);

    // Verify holder B has 1 key
    let view_b = client.get_holder_key_count(&creator, &holder_b);
    assert_eq!(view_b.key_count, 1);
    assert!(view_b.creator_exists);
}

#[test]
fn test_holder_key_count_view_unregistered_creator() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);
    let holder = Address::generate(&env);

    let view = client.get_holder_key_count(&unregistered_creator, &holder);
    assert_eq!(view.key_count, 0);
    assert!(!view.creator_exists);
    assert_eq!(view.creator, unregistered_creator);
    assert_eq!(view.holder, holder);
}

#[test]
fn test_holder_key_count_view_registered_creator_unseen_wallet() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, creator, _admin) = setup_with_creator(&env);
    let holder_with_keys = Address::generate(&env);
    let unseen_wallet = Address::generate(&env);

    // Holder with keys buys some keys
    client.buy_key(&creator, &holder_with_keys, &100i128, &None);
    client.buy_key(&creator, &holder_with_keys, &100i128, &None);

    // Unseen wallet should have zero keys
    let view = client.get_holder_key_count(&creator, &unseen_wallet);
    assert_eq!(view.key_count, 0);
    assert!(view.creator_exists);
}

#[test]
fn test_holder_key_count_view_consistency_with_get_key_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, creator, _admin) = setup_with_creator(&env);
    let holder = Address::generate(&env);

    // Buy some keys
    client.buy_key(&creator, &holder, &100i128, &None);
    client.buy_key(&creator, &holder, &100i128, &None);
    client.buy_key(&creator, &holder, &100i128, &None);

    // Both methods should return the same key count
    let view = client.get_holder_key_count(&creator, &holder);
    let balance = client.get_key_balance(&creator, &holder);
    assert_eq!(view.key_count, balance);
}

#[test]
fn test_holder_key_count_view_no_state_mutation() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, creator, _admin) = setup_with_creator(&env);
    let holder = Address::generate(&env);

    // Buy some keys first
    client.buy_key(&creator, &holder, &100i128, &None);
    client.buy_key(&creator, &holder, &100i128, &None);

    // Multiple reads should return the same result (no mutation)
    let view1 = client.get_holder_key_count(&creator, &holder);
    let view2 = client.get_holder_key_count(&creator, &holder);
    let view3 = client.get_holder_key_count(&creator, &holder);

    assert_eq!(view1.key_count, view2.key_count);
    assert_eq!(view2.key_count, view3.key_count);
    assert_eq!(view1.key_count, 2);
}

#[test]
fn test_holder_key_count_view_zero_keys_different_creators() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);
    let holder = Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator_a, &String::from_str(&env, "alice"));
    client.register_creator(&creator_b, &String::from_str(&env, "bob"));

    // Holder buys keys only from creator A
    client.buy_key(&creator_a, &holder, &100i128, &None);
    client.buy_key(&creator_a, &holder, &100i128, &None);

    // Check holder has 0 keys for creator B
    let view_b = client.get_holder_key_count(&creator_b, &holder);
    assert_eq!(view_b.key_count, 0);
    assert!(view_b.creator_exists);

    // Check holder has 2 keys for creator A
    let view_a = client.get_holder_key_count(&creator_a, &holder);
    assert_eq!(view_a.key_count, 2);
    assert!(view_a.creator_exists);
}

#[test]
fn test_holder_key_count_view_structure_fields() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, creator, _admin) = setup_with_creator(&env);
    let holder = Address::generate(&env);

    client.buy_key(&creator, &holder, &100i128, &None);

    let view = client.get_holder_key_count(&creator, &holder);

    // Verify all fields are populated correctly
    assert_eq!(view.creator, creator);
    assert_eq!(view.holder, holder);
    assert_eq!(view.key_count, 1);
    assert!(view.creator_exists);
}

#[test]
fn test_holder_key_count_view_unregistered_creator_fields() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);
    let holder = Address::generate(&env);

    let view = client.get_holder_key_count(&unregistered_creator, &holder);

    // Verify all fields are populated correctly for unregistered creator
    assert_eq!(view.creator, unregistered_creator);
    assert_eq!(view.holder, holder);
    assert_eq!(view.key_count, 0);
    assert!(!view.creator_exists);
}
