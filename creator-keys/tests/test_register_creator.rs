mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{ContractError, HANDLE_LEN_MIN};
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_register_creator_minimum_handle_length_success() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = Address::generate(&env);

    // Create a handle of exactly HANDLE_LEN_MIN characters
    let handle_bytes = [b'a'; HANDLE_LEN_MIN as usize];
    let handle_str = core::str::from_utf8(&handle_bytes).unwrap();
    let handle = String::from_str(&env, handle_str);

    let result = client.try_register_creator(&creator, &handle);

    // Happy path: the function succeeds
    assert_eq!(result, Ok(Ok(())));

    // State assertion: after a successful call, storage and derived views match expectations
    assert!(client.is_creator_registered(&creator));
    let profile = client.get_creator(&creator);
    assert_eq!(profile.handle, handle);
}

#[test]
fn test_register_creator_below_minimum_handle_length_fails() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = Address::generate(&env);

    // Create a handle one character below HANDLE_LEN_MIN
    let handle_bytes = [b'a'; (HANDLE_LEN_MIN - 1) as usize];
    let handle_str = core::str::from_utf8(&handle_bytes).unwrap();
    let handle = String::from_str(&env, handle_str);

    let result = client.try_register_creator(&creator, &handle);

    // Error case: expected failure
    assert_eq!(result, Err(Ok(ContractError::HandleTooShort)));

    // State assertion: failed calls do not leave partial state behind
    assert!(!client.is_creator_registered(&creator));
}
