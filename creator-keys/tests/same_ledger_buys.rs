//! Integration test for multiple buy transactions in the same ledger.
//!
//! Confirms that when two wallets submit buy transactions in the same ledger,
//! the contract processes them sequentially and produces a final supply that
//! reflects both purchases.  Wallet B's price must reflect the supply after
//! wallet A's buy was applied, not the pre-buy price.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_curve_slope, set_pricing_and_fees,
    test_env_with_auths,
};
use soroban_sdk::{testutils::Address as _, Address};

const KEY_PRICE: i128 = 100_000_000;
const CURVE_SLOPE: i128 = 1_000_000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

#[test]
fn test_two_same_ledger_buys_produce_correct_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    let creator = register_test_creator(&env, &client, "alice");
    let wallet_a = Address::generate(&env);
    let wallet_b = Address::generate(&env);

    // Wallet A buys 2 keys (sequential transactions, same ledger)
    let quote_a1 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &wallet_a, &quote_a1.total_amount, &None);

    let quote_a2 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &wallet_a, &quote_a2.total_amount, &None);

    assert_eq!(
        client.get_key_balance(&creator, &wallet_a),
        2,
        "wallet A should hold 2 keys after buying 2 keys"
    );

    // Wallet B's quote must reflect supply = 2 (post-wallet-A buys)
    let quote_b = client.get_buy_quote(&creator);

    assert!(
        quote_b.price > quote_a1.price,
        "wallet B's price ({}) should be higher than wallet A's first price ({}) \
         because supply increased from 0 to 2",
        quote_b.price,
        quote_a1.price,
    );

    // Wallet B buys 1 key at the price that reflects A's consumption
    client.buy_key(&creator, &wallet_b, &quote_b.total_amount, &None);

    // Final supply should be 3 (2 + 1)
    assert_eq!(
        client.get_total_key_supply(&creator),
        3,
        "final supply should be 3 after two same-ledger buys totalling 3 keys"
    );

    // Individual balances correct
    assert_eq!(
        client.get_key_balance(&creator, &wallet_a),
        2,
        "wallet A should hold 2 keys"
    );
    assert_eq!(
        client.get_key_balance(&creator, &wallet_b),
        1,
        "wallet B should hold 1 key"
    );

    // No keys lost or double-counted
    let supply = client.get_total_key_supply(&creator);
    let bal_a = client.get_key_balance(&creator, &wallet_a);
    let bal_b = client.get_key_balance(&creator, &wallet_b);
    assert_eq!(
        bal_a + bal_b,
        supply,
        "sum of holder balances ({}) must equal total supply ({})",
        bal_a + bal_b,
        supply,
    );
}
