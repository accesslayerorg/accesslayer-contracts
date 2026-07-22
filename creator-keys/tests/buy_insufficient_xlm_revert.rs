//! Integration test for buy transaction reverting when buyer sends insufficient XLM / attached amount < current price.

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, compute_expected_buy_price, register_creator_keys, register_test_creator,
    set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_buy_key_insufficient_xlm_reverts_without_state_mutation() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let base_price = 100i128;
    set_key_price_for_tests(&env, &client, base_price);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let current_price = compute_expected_buy_price(0, base_price);
    let insufficient_amount = current_price - 1;

    let snapshot_before = capture_snapshot(&client, &creator, &buyer);
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);

    // Invoke buy with attached amount 1 stroop below the current price
    let result = client.try_buy_key(&creator, &buyer, &insufficient_amount, &None);

    // Assert transaction reverts with InsufficientPayment error code
    assert_eq!(result, Err(Ok(ContractError::InsufficientPayment)));

    // Assert creator's key supply and buyer's holdings are unchanged after revert
    let snapshot_after = capture_snapshot(&client, &creator, &buyer);
    snapshot_before.assert_unchanged(&snapshot_after);

    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);
}
