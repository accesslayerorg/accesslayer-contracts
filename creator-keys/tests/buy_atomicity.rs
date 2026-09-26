//! Integration test: buy correctly updates both the creator supply and the
//! buyer's holder balance atomically.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_buy_updates_supply_and_balance_atomically() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);

    let quote1 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote1.total_amount, &None);
    let quote2 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote2.total_amount, &None);

    let supply = client.get_total_key_supply(&creator);
    let balance = client.get_key_balance(&creator, &buyer);

    assert_eq!(supply, 2, "creator supply should be 2 after buying 2 keys");
    assert_eq!(
        balance, 2,
        "buyer holder balance should be 2 after buying 2 keys"
    );
}

#[test]
fn test_buy_supply_and_balance_consistent_in_same_block() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let quote2 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote2.total_amount, &None);

    let supply = client.get_total_key_supply(&creator);
    let balance = client.get_key_balance(&creator, &buyer);

    assert_eq!(
        supply, balance,
        "supply and balance must be equal after buys"
    );
    assert_eq!(supply, 2);
    assert_eq!(balance, 2);
}

#[test]
fn test_buy_neither_value_is_stale_relative_to_other() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let supply = client.get_total_key_supply(&creator);
    let balance = client.get_key_balance(&creator, &buyer);

    assert_eq!(supply, 1);
    assert_eq!(balance, 1);
    assert_eq!(supply, balance);

    let quote2 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote2.total_amount, &None);

    let supply_after = client.get_total_key_supply(&creator);
    let balance_after = client.get_key_balance(&creator, &buyer);

    assert_eq!(supply_after, 2);
    assert_eq!(balance_after, 2);
    assert_eq!(supply_after, balance_after);
}
