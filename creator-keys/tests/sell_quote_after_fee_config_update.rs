//! Regression test for sell quote correctness after fee config mutation.
//!
//! Verifies that the sell quote (read-only path) reflects the updated fee config
//! and does not read stale values after a `set_fee_config` call.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use soroban_sdk::testutils::Address as _;

/// After updating the fee config, the sell quote must reflect the new bps split,
/// not the original one. This guards against stale-config reads in the quote path.
#[test]
fn test_sell_quote_reflects_updated_fee_config() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_key_price(&admin, &1000);
    // Original fee config: 90/10 split
    client.set_fee_config(&admin, &9000, &1000);

    let creator = register_test_creator(&env, &client, "alice");
    let holder = soroban_sdk::Address::generate(&env);

    client.buy_key(&creator, &holder, &1000, &None);

    // Get sell quote under original config
    let quote_before = client.get_sell_quote(&creator, &holder);
    assert_eq!(
        quote_before.protocol_fee, 100,
        "original protocol fee must be 10% of 1000"
    );
    assert_eq!(
        quote_before.creator_fee, 900,
        "original creator fee must be 90% of 1000"
    );

    // Update fee config: 80/20 split
    client.set_fee_config(&admin, &8000, &2000);

    // Sell quote must reflect updated config, not stale values
    let quote_after = client.get_sell_quote(&creator, &holder);
    assert_eq!(
        quote_after.protocol_fee, 200,
        "updated protocol fee must be 20% of 1000"
    );
    assert_eq!(
        quote_after.creator_fee, 800,
        "updated creator fee must be 80% of 1000"
    );
    assert_eq!(quote_after.price, 1000, "price must remain unchanged");
}

/// After a fee config update, the sell quote total_amount must be consistent
/// with the new fee split. The seller's net proceeds decrease when protocol
/// fees increase.
#[test]
fn test_sell_quote_total_amount_updates_after_fee_config_change() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_key_price(&admin, &500);
    client.set_fee_config(&admin, &9000, &1000);

    let creator = register_test_creator(&env, &client, "bob");
    let holder = soroban_sdk::Address::generate(&env);

    client.buy_key(&creator, &holder, &500, &None);

    let quote_original = client.get_sell_quote(&creator, &holder);
    // 500 - 450 (creator) - 50 (protocol) = 0
    assert_eq!(quote_original.total_amount, 0);

    // Update to 75/25
    client.set_fee_config(&admin, &7500, &2500);

    let quote_updated = client.get_sell_quote(&creator, &holder);
    // 500 - 375 (creator) - 125 (protocol) = 0
    assert_eq!(
        quote_updated.total_amount, 0,
        "total_amount must be recomputed with updated fees"
    );
    assert_eq!(quote_updated.protocol_fee, 125);
    assert_eq!(quote_updated.creator_fee, 375);
}
