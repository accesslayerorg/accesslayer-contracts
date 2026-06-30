//! Focused tests for the `get_creator_fee_recipient` read-only method.
//! Reviewed and confirmed: 29 Mar 2026.

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_get_creator_fee_recipient_returns_creator_address() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    assert_eq!(client.get_creator_fee_recipient(&creator), creator);
}

#[test]
fn test_get_creator_fee_recipient_is_read_only() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let first_read = client.get_creator_fee_recipient(&creator);
    let second_read = client.get_creator_fee_recipient(&creator);

    assert_eq!(first_read, second_read);
}

#[test]
fn test_get_creator_fee_recipient_fails_for_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);

    let result = client.try_get_creator_fee_recipient(&creator);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}
