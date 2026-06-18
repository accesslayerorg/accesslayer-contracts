//! Regression test: sell quote net proceeds must match execution-path proceeds.
//!
//! `get_sell_quote` and `sell_key` are separate code paths. This test calls the
//! quote first, executes a sell with the same `(creator, holder)` inputs, and
//! asserts the quoted seller net (`total_amount`) equals proceeds derived from
//! the execution fee path (`compute_fees_for_payment` on the key price).

mod contract_test_env;

use contract_test_env::{
    compute_expected_buy_price, register_creator_keys, register_test_creator, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::CreatorKeysContractClient;
use soroban_sdk::{testutils::Address as _, Address, Env};

/// Seller net proceeds implied by the same fee math used at execution time.
fn actual_sell_proceeds(client: &CreatorKeysContractClient<'_>, price: i128) -> i128 {
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&price);
    price - creator_fee - protocol_fee
}

/// Quote then sell at the current supply; assert quoted and actual proceeds match.
fn assert_sell_quote_matches_execution(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    holder: &Address,
    supply_before_sell: u32,
) {
    assert_eq!(
        client.get_total_key_supply(creator),
        supply_before_sell,
        "precondition: expected supply before sell"
    );
    assert!(
        client.get_key_balance(creator, holder) > 0,
        "precondition: holder must have keys to sell"
    );

    let quote = client.get_sell_quote(creator, holder);
    let actual_proceeds = actual_sell_proceeds(client, quote.price);

    assert_eq!(
        quote.creator_fee + quote.protocol_fee + quote.total_amount,
        quote.price,
        "quote fee split must conserve price"
    );
    assert_eq!(
        quote.total_amount, actual_proceeds,
        "quoted net proceeds must match execution-path proceeds before sell"
    );
    assert_eq!(
        quote.total_amount - actual_proceeds,
        0,
        "difference between quoted and actual proceeds must be zero before sell"
    );

    let supply_after = client.sell_key(creator, holder);
    assert_eq!(
        supply_after,
        supply_before_sell - 1,
        "supply should decrement by one after sell"
    );

    let post_sell_proceeds = actual_sell_proceeds(client, quote.price);
    assert_eq!(
        quote.total_amount, post_sell_proceeds,
        "quoted net proceeds must still match execution-path proceeds after sell"
    );
    assert_eq!(
        quote.total_amount - post_sell_proceeds,
        0,
        "difference between quoted and actual proceeds must be zero after sell"
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
        client.buy_key(creator, &holder, &buy_quote.total_amount);
    }
    holder
}

#[test]
fn test_sell_quote_proceeds_match_execution_at_supply_one() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1000, 9000, 1000);

    let creator = register_test_creator(&env, &client, "alice");
    let holder = setup_holder_with_supply(&env, &client, &creator, 1);

    assert_sell_quote_matches_execution(&client, &creator, &holder, 1);
}

#[test]
fn test_sell_quote_proceeds_match_execution_at_supply_five() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1000, 9000, 1000);

    let creator = register_test_creator(&env, &client, "bob");
    let holder = setup_holder_with_supply(&env, &client, &creator, 5);

    assert_sell_quote_matches_execution(&client, &creator, &holder, 5);
}

#[test]
fn test_buy_then_sell_has_symmetric_price_impact_after_fees() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 1_000_i128;
    set_pricing_and_fees(&env, &client, key_price, 9000, 1000);

    let creator = register_test_creator(&env, &client, "carol");
    let trader = Address::generate(&env);
    let starting_supply = client.get_total_key_supply(&creator);
    let initial_buy_quote = client.get_buy_quote(&creator);

    client.buy_key(&creator, &trader, &initial_buy_quote.total_amount);
    let sell_quote = client.get_sell_quote(&creator, &trader);
    let supply_after_buy = client.get_total_key_supply(&creator);
    assert_eq!(supply_after_buy, starting_supply + 1);

    // A buy followed by selling the same key should unwind the supply-side price impact;
    // the seller receives the original curve price minus the configured sell fees.
    let expected_sell_price = compute_expected_buy_price(supply_after_buy, key_price);
    let expected_sell_proceeds =
        expected_sell_price - sell_quote.creator_fee - sell_quote.protocol_fee;
    assert_eq!(sell_quote.price, expected_sell_price);
    assert_eq!(sell_quote.total_amount, expected_sell_proceeds);

    client.sell_key(&creator, &trader);
    let final_buy_quote = client.get_buy_quote(&creator);
    assert_eq!(client.get_total_key_supply(&creator), starting_supply);
    assert_eq!(final_buy_quote.price, initial_buy_quote.price);
    assert_eq!(final_buy_quote.total_amount, initial_buy_quote.total_amount);
}
