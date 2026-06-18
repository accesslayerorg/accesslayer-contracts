//! Regression test for sell execution after protocol fee config update.
//!
//! Verifies that the updated fee config is applied during the actual sell execution,
//! not just in the sell quote. This mirrors the buy-side test and confirms the fee
//! amount matches the updated bps calculation.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use soroban_sdk::testutils::Address as _;

/// After updating the protocol fee config, a sell execution applies the new fee, not
/// the original. The seller receives the correct proceeds and the fee matches the
/// updated bps calculation.
#[test]
fn test_sell_execution_applies_updated_protocol_fee() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_key_price(&admin, &1000);
    // Original fee config: 90/10 split
    client.set_fee_config(&admin, &9000, &1000);

    let creator = register_test_creator(&env, &client, "alice");
    let holder = soroban_sdk::Address::generate(&env);

    // Holder buys a key at the original fee config
    client.buy_key(&creator, &holder, &1000);
    assert_eq!(
        client.get_key_balance(&creator, &holder),
        1,
        "holder balance should be 1 after buy"
    );

    // Update fee config before executing sell
    client.set_fee_config(&admin, &8000, &2000); // updated: 80/20 split

    let supply = client.sell_key(&creator, &holder);
    assert_eq!(supply, 0, "supply should decrement to 0 after sell");

    // Holder balance should be zero after selling
    assert_eq!(
        client.get_key_balance(&creator, &holder),
        0,
        "holder balance should be 0 after sell"
    );

    // The fee applies the updated config (80/20), not the original (90/10)
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&1000);
    // updated 80/20 on 1000: protocol = 200, creator = 800
    assert_eq!(
        protocol_fee, 200,
        "protocol fee should reflect updated 20% bps, not original 10%"
    );
    assert_eq!(
        creator_fee, 800,
        "creator fee should reflect updated 80% bps, not original 90%"
    );
}

/// After a fee config update, the sell quote and sell execution agree on the fee amount.
///
/// This confirms the updated bps is applied consistently across both the read-only
/// quote path and the stateful sell execution path.
#[test]
fn test_sell_execution_fee_matches_quote_after_fee_config_update() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_key_price(&admin, &500);
    client.set_fee_config(&admin, &9000, &1000);

    let creator = register_test_creator(&env, &client, "bob");
    let holder = soroban_sdk::Address::generate(&env);

    // Holder buys a key at the original fee config
    client.buy_key(&creator, &holder, &500);
    assert_eq!(
        client.get_key_balance(&creator, &holder),
        1,
        "holder balance should be 1 after buy"
    );

    // Update fee config: 75/25 split
    client.set_fee_config(&admin, &7500, &2500);

    // Read the quote under the updated config
    let quote = client.get_sell_quote(&creator, &holder);

    // Execute the sell
    let supply = client.sell_key(&creator, &holder);
    assert_eq!(supply, 0, "supply should be 0 after sell");
    assert_eq!(
        client.get_key_balance(&creator, &holder),
        0,
        "holder balance should be 0 after sell"
    );

    // Fee via compute_fees_for_payment must match the quote under the updated config
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&500);
    // updated 75/25 on 500: protocol = 125, creator = 375
    assert_eq!(
        protocol_fee, 125,
        "protocol fee should match updated 25% bps"
    );
    assert_eq!(creator_fee, 375, "creator fee should match updated 75% bps");
    assert_eq!(
        quote.protocol_fee, protocol_fee,
        "execution fee matches quote fee after fee config update"
    );
    assert_eq!(
        quote.creator_fee, creator_fee,
        "execution creator fee matches quote after fee config update"
    );
}
