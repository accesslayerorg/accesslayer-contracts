//! Confirmation tests for the `setup_holders` fixture helper.
//!
//! Verifies that each wallet receives the expected key balance and that the
//! returned total supply equals the sum of all amounts passed to the helper.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, setup_holders,
    test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

const KEY_PRICE: i128 = 100;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

#[test]
fn test_setup_holders_produces_correct_balances() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "creator");

    let wallet_a = soroban_sdk::Address::generate(&env);
    let wallet_b = soroban_sdk::Address::generate(&env);
    let wallet_c = soroban_sdk::Address::generate(&env);

    setup_holders(
        &env,
        &client,
        &creator,
        &[
            (wallet_a.clone(), 2),
            (wallet_b.clone(), 3),
            (wallet_c.clone(), 1),
        ],
    );

    assert_eq!(
        client.get_key_balance(&creator, &wallet_a),
        2,
        "wallet_a must hold 2 keys"
    );
    assert_eq!(
        client.get_key_balance(&creator, &wallet_b),
        3,
        "wallet_b must hold 3 keys"
    );
    assert_eq!(
        client.get_key_balance(&creator, &wallet_c),
        1,
        "wallet_c must hold 1 key"
    );
}

#[test]
fn test_setup_holders_returns_correct_total_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "creator");

    let wallet_a = soroban_sdk::Address::generate(&env);
    let wallet_b = soroban_sdk::Address::generate(&env);

    let total_supply = setup_holders(&env, &client, &creator, &[(wallet_a, 4), (wallet_b, 2)]);

    assert_eq!(
        total_supply, 6,
        "returned supply must equal the sum of all bought amounts"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        6,
        "on-chain supply must match the returned value"
    );
}
