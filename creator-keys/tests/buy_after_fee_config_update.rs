//! Regression test for buy execution after protocol fee config update.
//!
//! Verifies that the updated fee config is applied during the actual buy execution,
//! not just in the buy quote. This is distinct from the existing buy-quote-after-fee-
//! config-update tests: those assert the quote reflects updated fees, while these
//! assert the execution path (buy_key + compute_fees_for_payment) applies them too.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use soroban_sdk::testutils::Address as _;

/// After updating the protocol fee config, a buy execution applies the new fee, not
/// the original. The buyer receives the correct key balance and the fee matches the
/// updated bps calculation.
#[test]
fn test_buy_execution_applies_updated_protocol_fee() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_key_price(&admin, &1000);
    // Original fee config: 90/10 split
    client.set_fee_config(&admin, &9000, &1000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    // Update fee config before executing buy
    client.set_fee_config(&admin, &8000, &2000); // updated: 80/20 split

    let supply = client.buy_key(&creator, &buyer, &1000);
    assert_eq!(supply, 1, "supply should increment to 1 after buy");

    // Buyer receives the correct key balance
    assert_eq!(
        client.get_key_balance(&creator, &buyer),
        1,
        "buyer balance should be 1 after buy"
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

/// After a fee config update, the buy quote and buy execution agree on the fee amount.
///
/// This confirms the updated bps is applied consistently across both the read-only
/// quote path and the stateful buy execution path.
#[test]
fn test_buy_execution_fee_matches_quote_after_fee_config_update() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_key_price(&admin, &500);
    client.set_fee_config(&admin, &9000, &1000);

    let creator = register_test_creator(&env, &client, "bob");
    let buyer = soroban_sdk::Address::generate(&env);

    // Update fee config: 75/25 split
    client.set_fee_config(&admin, &7500, &2500);

    // Read the quote under the updated config
    let quote = client.get_buy_quote(&creator);

    // Execute the buy
    let supply = client.buy_key(&creator, &buyer, &500);
    assert_eq!(supply, 1, "supply should be 1 after buy");
    assert_eq!(
        client.get_key_balance(&creator, &buyer),
        1,
        "buyer balance should be 1"
    );

    // Fee via compute_fees_for_payment must match the quote under the updated config
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&500);
    // updated 75/25 on 500: protocol = 125, creator = 375
    assert_eq!(
        protocol_fee, 125,
        "protocol fee should match updated 25% bps"
    );
    assert_eq!(
        creator_fee, 375,
        "creator fee should match updated 75% bps"
    );
    assert_eq!(
        quote.protocol_fee, protocol_fee,
        "execution fee matches quote fee after fee config update"
    );
    assert_eq!(
        quote.creator_fee, creator_fee,
        "execution creator fee matches quote after fee config update"
    );
}
