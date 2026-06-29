//! Tests for `distribute_dividend` entrypoint.

mod contract_test_env;

use contract_test_env::{
    assert_claimable, compute_expected_holder_dividend, distribute_test_dividend,
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_pricing_and_fees,
    test_env_with_auths, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

// ---------------------------------------------------------------------------
// Error paths
// ---------------------------------------------------------------------------

#[test]
fn test_distribute_dividend_zero_amount_fails() {
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
    let distributor = Address::generate(&env);

    let result = client.try_distribute_dividend(&creator, &distributor, &0);
    assert_eq!(result, Err(Ok(ContractError::ZeroDistributionAmount)));
}

#[test]
fn test_distribute_dividend_negative_amount_fails() {
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
    let distributor = Address::generate(&env);

    let result = client.try_distribute_dividend(&creator, &distributor, &-1);
    assert_eq!(result, Err(Ok(ContractError::ZeroDistributionAmount)));
}

#[test]
fn test_distribute_dividend_not_registered_creator_fails() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let unknown = Address::generate(&env);
    let distributor = Address::generate(&env);

    let result = client.try_distribute_dividend(&unknown, &distributor, &1000);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_distribute_dividend_no_key_holders_fails() {
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
    let distributor = Address::generate(&env);

    // Creator registered but no one has bought a key yet (supply == 0).
    let result = client.try_distribute_dividend(&creator, &distributor, &1000);
    assert_eq!(result, Err(Ok(ContractError::NoKeyHolders)));
}

#[test]
fn test_distribute_dividend_no_fee_config_fails() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    // Buy without fee config set — buy_key allows it; now distribute requires config.
    // But buy_key itself requires fee config isn't set... actually buy_key works without it.
    // We need to seed at least one key holder first with no fee config.
    // buy_key skips fee split when config not set, so let's go:
    client.buy_key(&creator, &buyer, &100, &None);

    let distributor = Address::generate(&env);
    let result = client.try_distribute_dividend(&creator, &distributor, &1000);
    assert_eq!(result, Err(Ok(ContractError::FeeConfigNotSet)));
}

#[test]
fn test_distribute_dividend_while_paused_fails() {
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

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.pause(&admin);

    let distributor = Address::generate(&env);
    let result = client.try_distribute_dividend(&creator, &distributor, &1000);
    assert_eq!(result, Err(Ok(ContractError::ProtocolPaused)));
}

// ---------------------------------------------------------------------------
// Happy paths
// ---------------------------------------------------------------------------

#[test]
fn test_distribute_dividend_single_holder_receives_full_net() {
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
fn test_distribute_dividend_two_equal_holders_split_evenly() {
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
    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);
    client.buy_key(&creator, &buyer_a, &100, &None);
    client.buy_key(&creator, &buyer_b, &100, &None);

    let distributor = Address::generate(&env);
    let amount = 10_000i128;
    distribute_test_dividend(&client, &creator, &distributor, amount);

    let expected_each = compute_expected_holder_dividend(amount, 1, 2, DEFAULT_PROTOCOL_BPS);
    assert_claimable(&client, &creator, &buyer_a, expected_each);
    assert_claimable(&client, &creator, &buyer_b, expected_each);
}

#[test]
fn test_distribute_dividend_proportional_to_balance() {
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
    let whale = Address::generate(&env);
    let small = Address::generate(&env);
    // whale buys 3, small buys 1 → supply = 4
    client.buy_key(&creator, &whale, &100, &None);
    client.buy_key(&creator, &whale, &100, &None);
    client.buy_key(&creator, &whale, &100, &None);
    client.buy_key(&creator, &small, &100, &None);

    let distributor = Address::generate(&env);
    let amount = 10_000i128;
    distribute_test_dividend(&client, &creator, &distributor, amount);

    let expected_whale = compute_expected_holder_dividend(amount, 3, 4, DEFAULT_PROTOCOL_BPS);
    let expected_small = compute_expected_holder_dividend(amount, 1, 4, DEFAULT_PROTOCOL_BPS);
    assert_claimable(&client, &creator, &whale, expected_whale);
    assert_claimable(&client, &creator, &small, expected_small);
}

#[test]
fn test_distribute_dividend_deducts_protocol_fee() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    // Use 10% protocol fee
    set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100, &None);

    let before_protocol_balance = client.get_protocol_recipient_balance();
    let distributor = Address::generate(&env);
    let amount = 10_000i128;
    distribute_test_dividend(&client, &creator, &distributor, amount);

    // Protocol fee = 10% of 10_000 = 1_000
    let after_protocol_balance = client.get_protocol_recipient_balance();
    assert_eq!(after_protocol_balance - before_protocol_balance, 1_000);

    // Holder gets 90% of 10_000 = 9_000
    assert_claimable(&client, &creator, &buyer, 9_000);
}

#[test]
fn test_multiple_distributions_accumulate() {
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

    let expected = compute_expected_holder_dividend(10_000, 1, 1, DEFAULT_PROTOCOL_BPS) * 2;
    assert_claimable(&client, &creator, &buyer, expected);
}
