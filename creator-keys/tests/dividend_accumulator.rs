//! Tests for the per-key dividend accumulator model.
//!
//! Verifies accumulator growth, checkpoint semantics, and interaction with buy/sell.

mod contract_test_env;

use contract_test_env::{
    assert_claimable, compute_expected_holder_dividend, distribute_test_dividend,
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
    DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_new_buyer_after_distribution_earns_no_retroactive_dividends() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");

    let early_buyer = Address::generate(&env);
    client.buy_key(&creator, &early_buyer, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    // Late buyer joins after distribution; checkpoint is set to current accumulator.
    let late_buyer = Address::generate(&env);
    client.buy_key(&creator, &late_buyer, &100, &None);

    assert_claimable(&client, &creator, &late_buyer, 0);
}

#[test]
fn test_existing_holder_earns_from_all_distributions_via_checkpoint() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");

    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    // Holder was present for both distributions; supply was 1 for both.
    let expected = compute_expected_holder_dividend(10_000, 1, 1, DEFAULT_PROTOCOL_BPS) * 2;
    assert_claimable(&client, &creator, &holder, expected);
}

#[test]
fn test_sell_all_then_rebuy_starts_fresh_on_pending() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");

    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    // Sell all keys — settlement captures earned into pending.
    client.sell_key(&creator, &holder, &None);
    let pending_after_sell = compute_expected_holder_dividend(10_000, 1, 1, DEFAULT_PROTOCOL_BPS);

    // Buy again — checkpoint is updated but pending remains from the sell settlement.
    client.buy_key(&creator, &holder, &100, &None);

    // Another distribution after re-buy; supply is now 1 again, holder_balance is 1.
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    let per_dist = compute_expected_holder_dividend(10_000, 1, 1, DEFAULT_PROTOCOL_BPS);
    // Total = pending_from_before_sell + earned_since_rebuy
    let expected_total = pending_after_sell + per_dist;
    assert_claimable(&client, &creator, &holder, expected_total);
}

#[test]
fn test_accumulator_grows_correctly_across_sequential_distributions() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");

    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &100, &None);

    let distributor = Address::generate(&env);
    let amounts = [5_000i128, 3_000, 7_000];

    let mut total_expected = 0i128;
    for &amount in &amounts {
        distribute_test_dividend(&client, &creator, &distributor, amount);
        total_expected += compute_expected_holder_dividend(amount, 1, 1, DEFAULT_PROTOCOL_BPS);
    }

    assert_claimable(&client, &creator, &holder, total_expected);
}

#[test]
fn test_buy_more_keys_mid_stream_does_not_earn_retroactively() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");

    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &100, &None); // balance = 1, supply = 1

    let distributor = Address::generate(&env);
    // First distribution: supply = 1, holder earns full net.
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    // Holder buys a second key — settlement runs, checkpointing earnings so far.
    client.buy_key(&creator, &holder, &100, &None); // balance = 2, supply = 2

    // Second distribution: supply = 2, holder (balance=2) earns 2/2 of net.
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    let first_dist = compute_expected_holder_dividend(10_000, 1, 1, DEFAULT_PROTOCOL_BPS);
    let second_dist = compute_expected_holder_dividend(10_000, 2, 2, DEFAULT_PROTOCOL_BPS);
    let expected = first_dist + second_dist;

    assert_claimable(&client, &creator, &holder, expected);
}
