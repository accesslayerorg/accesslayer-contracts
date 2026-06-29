//! Tests for `get_claimable_dividend` read-only view.

mod contract_test_env;

use contract_test_env::{
    assert_claimable, capture_snapshot, compute_expected_holder_dividend, distribute_test_dividend,
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
    DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_get_claimable_dividend_zero_before_any_distribution() {
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
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100, &None);

    assert_claimable(&client, &creator, &buyer, 0);
}

#[test]
fn test_get_claimable_dividend_is_read_only() {
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
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    let snapshot_before = capture_snapshot(&client, &creator, &buyer);
    let _ = client.get_claimable_dividend(&creator, &buyer);
    let snapshot_after = capture_snapshot(&client, &creator, &buyer);

    snapshot_before.assert_unchanged(&snapshot_after);
}

#[test]
fn test_get_claimable_dividend_correct_after_distribution() {
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
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100, &None);

    let distributor = Address::generate(&env);
    let amount = 10_000i128;
    distribute_test_dividend(&client, &creator, &distributor, amount);

    let expected = compute_expected_holder_dividend(amount, 1, 1, DEFAULT_PROTOCOL_BPS);
    assert_claimable(&client, &creator, &buyer, expected);
}

#[test]
fn test_get_claimable_dividend_zero_after_claim() {
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
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);
    client.claim_dividend(&creator, &buyer);

    assert_claimable(&client, &creator, &buyer, 0);
}

#[test]
fn test_get_claimable_dividend_accumulates_across_distributions() {
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
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    let expected = compute_expected_holder_dividend(10_000, 1, 1, DEFAULT_PROTOCOL_BPS) * 3;
    assert_claimable(&client, &creator, &buyer, expected);
}

#[test]
fn test_get_claimable_dividend_works_while_paused() {
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
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.pause(&admin);

    // Read-only view must work even when protocol is paused.
    let expected = compute_expected_holder_dividend(10_000, 1, 1, DEFAULT_PROTOCOL_BPS);
    assert_claimable(&client, &creator, &buyer, expected);
}
