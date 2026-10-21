//! Unit tests for CurvePreset storage and retrieval.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{ContractError, CurvePreset};
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_curve_preset_variants_and_error_handling() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator_linear = Address::generate(&env);
    let creator_quadratic = Address::generate(&env);
    let creator_flat = Address::generate(&env);

    // Register creator with Linear preset
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator_linear.clone(),
            handle: String::from_str(&env, "linear_c"),
        },
        &None,
        &None,
        &None,
        &Some(CurvePreset::Linear),
        &None,
        &None,
    );

    // Register creator with Quadratic preset
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator_quadratic.clone(),
            handle: String::from_str(&env, "quadratic_c"),
        },
        &None,
        &None,
        &None,
        &Some(CurvePreset::Quadratic),
        &None,
        &None,
    );

    // Register creator with Flat preset
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator_flat.clone(),
            handle: String::from_str(&env, "flat_c"),
        },
        &None,
        &None,
        &None,
        &Some(CurvePreset::Flat),
        &None,
        &None,
    );

    // Assert each returns the correct variant
    assert_eq!(
        client.get_curve_preset(&creator_linear),
        CurvePreset::Linear
    );
    assert_eq!(
        client.get_curve_preset(&creator_quadratic),
        CurvePreset::Quadratic
    );
    assert_eq!(client.get_curve_preset(&creator_flat), CurvePreset::Flat);

    // Assert querying a non-existent creator returns the expected error (NotRegistered)
    let non_existent = Address::generate(&env);
    let result = client.try_get_curve_preset(&non_existent);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}
