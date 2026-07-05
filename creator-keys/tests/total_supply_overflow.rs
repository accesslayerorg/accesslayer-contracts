//! Regression test for total-supply overflow protection on sequential buys.
//!
//! `buy_key` increments `CreatorProfile.supply` (a `u32`) by one per call using
//! checked arithmetic. Driving the supply to its ceiling with real buys would
//! require billions of calls, so we seed the stored supply at `u32::MAX` and then
//! attempt one more buy — exercising the same `checked_add` guard that protects
//! against accumulation overflow.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_key_price_for_tests, test_env_with_auths};
use creator_keys::{constants, ContractError, CreatorProfile};
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn buy_at_max_supply_is_rejected_with_overflow_and_no_state_corruption() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let _admin = set_key_price_for_tests(&env, &client, 100);

    let creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "maxed"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Seed supply at the ceiling to simulate "many sequential buys" cheaply.
    env.as_contract(&contract_id, || {
        let key = constants::storage::creator(&creator);
        let mut profile: CreatorProfile = env.storage().persistent().get(&key).unwrap();
        profile.supply = u32::MAX;
        env.storage().persistent().set(&key, &profile);
    });

    let buyer = Address::generate(&env);
    let result = client.try_buy_key(&creator, &buyer, &100i128, &None);

    // The buy that would push supply past the ceiling is rejected with Overflow.
    assert!(matches!(result, Err(Ok(ContractError::Overflow))));

    // No state corruption: supply is unchanged and the buyer was never credited.
    assert_eq!(client.get_creator_supply(&creator), u32::MAX);
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);
    assert_eq!(client.get_creator_holder_count(&creator), 0);
}
