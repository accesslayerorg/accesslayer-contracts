mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{ContractError, HANDLE_LEN_MIN};
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_register_creator_minimum_handle_length_success() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = Address::generate(&env);

    let min_handle = "a".repeat(HANDLE_LEN_MIN as usize);
    let handle = String::from_str(&env, &min_handle);

    let result = client.try_register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: handle.clone(),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Happy path: the function succeeds
    assert_eq!(result, Ok(Ok(())));

    // State assertion: after a successful call, storage and derived views match expectations
    assert!(client.is_creator_registered(&creator));
    let profile = client.get_creator(&creator);
    assert_eq!(profile.handle, handle);
    assert_eq!(profile.creator, creator);
}

#[test]
fn test_register_creator_below_minimum_handle_length_fails() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = Address::generate(&env);

    let short_handle = "a".repeat((HANDLE_LEN_MIN - 1) as usize);
    let handle = String::from_str(&env, &short_handle);

    let result = client.try_register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: handle.clone(),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Error case: expected failure
    assert_eq!(result, Err(Ok(ContractError::HandleTooShort)));

    // State assertion: failed calls do not leave partial state behind
    assert!(!client.is_creator_registered(&creator));
}
