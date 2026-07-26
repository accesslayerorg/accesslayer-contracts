//! Integration test for multi-buyer scenario (#588)

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::{testutils::Address as _, Address};

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

#[test]
fn test_multi_buyer_scenario_independent_balances() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let wallet_a = Address::generate(&env);
    let wallet_b = Address::generate(&env);

    // Wallet A buys 3 keys for creator X
    for _ in 0..3 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &wallet_a, &quote.total_amount, &None);
    }

    // Assert wallet A holds 3 keys
    assert_eq!(client.get_key_balance(&creator, &wallet_a), 3);
    // Assert wallet B holds 0 keys
    assert_eq!(client.get_key_balance(&creator, &wallet_b), 0);

    // Wallet B buys 2 keys for creator X
    for _ in 0..2 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &wallet_b, &quote.total_amount, &None);
    }

    // Assert wallet A holds 3 keys
    assert_eq!(client.get_key_balance(&creator, &wallet_a), 3);
    // Assert wallet B holds 2 keys
    assert_eq!(client.get_key_balance(&creator, &wallet_b), 2);

    // Assert creator X supply is 5 (3 + 2)
    assert_eq!(client.get_total_key_supply(&creator), 5);
}
