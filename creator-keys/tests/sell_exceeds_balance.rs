//! Integration test: sell reverts when quantity exceeds holder balance.
//!
//! Verifies that selling more keys than owned reverts with InsufficientBalance,
//! and both holder balance and creator supply remain unchanged.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests,
    test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup(env: &Env) -> (creator_keys::CreatorKeysContractClient<'_>, Address, Address) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, 100_i128);
    let creator = register_test_creator(env, &client, "alice");
    let holder = Address::generate(env);
    (client, creator, holder)
}

#[test]
fn test_sell_exceeds_balance_reverts_insufficient_balance() {
    let env = test_env_with_auths();
    let (client, creator, holder) = setup(&env);

    // Buy exactly 2 keys
    client.buy_key(&creator, &holder, &100_i128, &None);
    client.buy_key(&creator, &holder, &100_i128, &None);

    assert_eq!(client.get_key_balance(&creator, &holder), 2);
    let supply_before = client.get_total_key_supply(&creator);

    // Try to sell 3 keys — must revert
    let result = client.try_sell_key(&creator, &holder, &Some(3u32));

    assert!(result.is_err());
    match result {
        Err(Ok(ContractError::InsufficientBalance)) => {}
        other => panic!("expected InsufficientBalance, got {:?}", other),
    }

    // Holder still owns exactly 2 keys
    assert_eq!(client.get_key_balance(&creator, &holder), 2);
    // Creator supply unchanged
    assert_eq!(client.get_total_key_supply(&creator), supply_before);
}
