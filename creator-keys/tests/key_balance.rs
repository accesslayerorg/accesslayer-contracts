//! Tests for the `get_key_balance` read-only view method.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env, String};

#[test]
fn test_key_balance_starts_at_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = soroban_sdk::Address::generate(&env);
    let wallet = soroban_sdk::Address::generate(&env);

    assert_eq!(client.get_key_balance(&creator, &wallet), 0);
}

#[test]
fn test_key_balance_increments_on_buy() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let buyer = soroban_sdk::Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"));

    assert_eq!(client.get_key_balance(&creator, &buyer), 0);

    client.buy_key(&creator, &buyer, &100i128);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);

    client.buy_key(&creator, &buyer, &100i128);
    assert_eq!(client.get_key_balance(&creator, &buyer), 2);
}

#[test]
fn test_key_balance_is_per_buyer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let creator = soroban_sdk::Address::generate(&env);
    let buyer_a = soroban_sdk::Address::generate(&env);
    let buyer_b = soroban_sdk::Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"));

    client.buy_key(&creator, &buyer_a, &100i128);
    client.buy_key(&creator, &buyer_a, &100i128);
    client.buy_key(&creator, &buyer_b, &100i128);

    assert_eq!(client.get_key_balance(&creator, &buyer_a), 2);
    assert_eq!(client.get_key_balance(&creator, &buyer_b), 1);
}

#[test]
fn test_key_balance_is_per_creator() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let creator_a = soroban_sdk::Address::generate(&env);
    let creator_b = soroban_sdk::Address::generate(&env);
    let buyer = soroban_sdk::Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator_a, &String::from_str(&env, "alice"));
    client.register_creator(&creator_b, &String::from_str(&env, "bob"));

    client.buy_key(&creator_a, &buyer, &100i128);

    assert_eq!(client.get_key_balance(&creator_a, &buyer), 1);
    assert_eq!(client.get_key_balance(&creator_b, &buyer), 0);
}
