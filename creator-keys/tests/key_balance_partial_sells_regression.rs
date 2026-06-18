//! Regression test for key balance reads after multiple partial sells (#299).
//!
//! Confirms that the balance decrements accurately after each individual sell
//! and that the final balance equals the initial balance minus the total sold.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_key_price_for_tests};
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_key_balance_decrements_correctly_after_each_partial_sell() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();

    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100_i128);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    // Buy 5 keys to establish the initial balance.
    for _ in 0..5 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }
    assert_eq!(client.get_key_balance(&creator, &holder), 5);

    // Partial sell 1: sell 1 key → balance should be 4.
    client.sell_key(&creator, &holder, &None);
    assert_eq!(client.get_key_balance(&creator, &holder), 4);

    // Partial sell 2: sell 1 key → balance should be 3.
    client.sell_key(&creator, &holder, &None);
    assert_eq!(client.get_key_balance(&creator, &holder), 3);

    // Partial sell 3: sell 1 key → balance should be 2.
    client.sell_key(&creator, &holder, &None);
    assert_eq!(client.get_key_balance(&creator, &holder), 2);

    // Final balance: 5 bought − 3 sold = 2.
    assert_eq!(client.get_key_balance(&creator, &holder), 5 - 3);
}
