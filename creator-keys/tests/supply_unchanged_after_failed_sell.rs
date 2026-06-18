//! Regression test: total supply unchanged after a failed sell.
//!
//! When a sell fails (e.g. insufficient balance), total supply must remain
//! exactly the value it was before the call. This test confirms no partial
//! state mutation occurs on the sell error path.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

/// Records total supply, attempts a sell that fails with InsufficientBalance,
/// then asserts the supply is unchanged.
#[test]
fn test_total_supply_unchanged_after_failed_sell_insufficient_balance() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100_i128);
    let creator = register_test_creator(&env, &client, "alice");

    // Buy one key so there is a non-zero supply to observe.
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100_i128);

    // A different address that holds zero keys.
    let non_holder = Address::generate(&env);

    // Record supply before the failing sell.
    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(supply_before, 1, "setup: supply should be 1 after one buy");

    // Attempt to sell — should fail because non_holder has no keys.
    let result = client.try_sell_key(&creator, &non_holder);
    assert_eq!(
        result,
        Err(Ok(ContractError::InsufficientBalance)),
        "sell should fail with InsufficientBalance"
    );

    // Assert supply is unchanged after the failed sell.
    let supply_after = client.get_total_key_supply(&creator);
    assert_eq!(
        supply_after, supply_before,
        "total supply must be unchanged after a failed sell"
    );
}

/// Records total supply, attempts a sell for an unregistered creator,
/// then asserts the supply is unchanged.
#[test]
fn test_total_supply_unchanged_after_failed_sell_not_registered() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100_i128);

    // Register one creator and buy a key so there is observable state.
    let creator = register_test_creator(&env, &client, "bob");
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100_i128);

    let supply_before = client.get_total_key_supply(&creator);

    // A completely different unregistered creator address.
    let unregistered = Address::generate(&env);
    let result = client.try_sell_key(&unregistered, &buyer);
    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "sell should fail with NotRegistered"
    );

    // The registered creator's supply must be untouched.
    let supply_after = client.get_total_key_supply(&creator);
    assert_eq!(
        supply_after, supply_before,
        "total supply must be unchanged after a failed sell against unregistered creator"
    );
}
