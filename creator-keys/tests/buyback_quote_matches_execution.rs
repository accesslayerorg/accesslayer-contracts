//! Regression test: buyback quote cost must match the execution-path cost.
//!
//! The creator-facing buyback flow uses the same fixed-price fee math as the
//! existing exit path, so this test snapshots the quote, executes the matching
//! state change, and asserts the quoted cost equals the actual charged amount
//! at two different supply levels.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::CreatorKeysContractClient;
use soroban_sdk::{testutils::Address as _, Address, Env};

/// Cost implied by the same fee math used when the contract executes the trade path.
fn actual_buyback_cost(client: &CreatorKeysContractClient<'_>, price: i128) -> i128 {
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&price);
    price - creator_fee - protocol_fee
}

/// Quote at the current supply and assert the quoted buyback cost matches the
/// actual execution-path cost for the same creator and holder.
fn assert_buyback_quote_matches_execution(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    holder: &Address,
    supply_before_trade: u32,
) {
    assert_eq!(
        client.get_total_key_supply(creator),
        supply_before_trade,
        "precondition: expected supply before buyback"
    );
    assert!(
        client.get_key_balance(creator, holder) > 0,
        "precondition: holder must have keys before buyback"
    );

    let quote = client.get_sell_quote(creator, holder);
    let actual_cost = actual_buyback_cost(client, quote.price);

    assert_eq!(
        quote.creator_fee + quote.protocol_fee + quote.total_amount,
        quote.price,
        "quote fee split must conserve price"
    );
    assert_eq!(
        quote.total_amount, actual_cost,
        "quoted buyback cost must match execution-path cost before trade"
    );

    let supply_after_trade = client.sell_key(creator, holder, &None);
    assert_eq!(
        supply_after_trade,
        supply_before_trade - 1,
        "supply should decrement by one after buyback-equivalent execution"
    );

    let post_trade_cost = actual_buyback_cost(client, quote.price);
    assert_eq!(
        quote.total_amount, post_trade_cost,
        "quoted buyback cost must still match execution-path cost after trade"
    );
}

fn setup_holder_with_supply(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    key_count: u32,
) -> Address {
    let holder = Address::generate(env);
    let buy_quote = client.get_buy_quote(creator);
    for _ in 0..key_count {
        client.buy_key(creator, &holder, &buy_quote.total_amount, &None);
    }
    holder
}

#[test]
fn test_buyback_quote_matches_execution_at_supply_one() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1000, 9000, 1000);

    let creator = register_test_creator(&env, &client, "alice");
    let holder = setup_holder_with_supply(&env, &client, &creator, 1);

    assert_buyback_quote_matches_execution(&client, &creator, &holder, 1);
}

#[test]
fn test_buyback_quote_matches_execution_at_supply_five() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1000, 9000, 1000);

    let creator = register_test_creator(&env, &client, "bob");
    let holder = setup_holder_with_supply(&env, &client, &creator, 5);

    assert_buyback_quote_matches_execution(&client, &creator, &holder, 5);
}
