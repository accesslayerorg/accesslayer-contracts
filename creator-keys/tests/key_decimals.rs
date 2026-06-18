//! Unit tests for creator key decimals consistency with token standard.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};

/// The expected decimals value conforming to the Soroban token standard.
const EXPECTED_DECIMALS: u32 = 7;

#[test]
fn test_get_key_decimals_matches_token_standard() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let decimals = client.get_key_decimals();
    assert_eq!(decimals, EXPECTED_DECIMALS);
}

#[test]
fn test_get_key_decimals_consistent_across_creator_instances() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    register_test_creator(&env, &client, "alice");
    assert_eq!(client.get_key_decimals(), EXPECTED_DECIMALS);

    register_test_creator(&env, &client, "bob");
    assert_eq!(client.get_key_decimals(), EXPECTED_DECIMALS);
}
