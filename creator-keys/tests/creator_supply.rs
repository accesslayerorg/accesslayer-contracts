//! Tests for the `get_creator_supply` read-only method.

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    client.set_key_price(&admin, &100_i128);

    let creator = Address::generate(env);
    client.register_creator(&creator, &String::from_str(env, "alice"));

    (client, admin, creator)
}

#[test]
fn test_get_creator_supply_returns_current_supply() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, creator) = setup(&env);
    let buyer_one = Address::generate(&env);
    let buyer_two = Address::generate(&env);

    client.buy_key(&creator, &buyer_one, &100_i128, &None);
    client.buy_key(&creator, &buyer_two, &100_i128, &None);

    assert_eq!(client.get_creator_supply(&creator), 2);
}

#[test]
fn test_get_creator_supply_is_read_only() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, creator) = setup(&env);
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100_i128, &None);

    let first_read = client.get_creator_supply(&creator);
    let second_read = client.get_creator_supply(&creator);

    assert_eq!(first_read, second_read);
    assert_eq!(first_read, 1);
}

#[test]
fn test_get_creator_supply_fails_for_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);

    let result = client.try_get_creator_supply(&creator);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}
