//! Tests for `claim_dividend` entrypoint.

mod contract_test_env;

use contract_test_env::{
    compute_expected_holder_dividend, distribute_test_dividend, register_creator_keys,
    register_test_creator, set_pricing_and_fees, test_env_with_auths, DEFAULT_CREATOR_BPS,
    DEFAULT_PROTOCOL_BPS,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_claim_dividend_no_claimable_fails() {
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

    // No distribution made yet
    let result = client.try_claim_dividend(&creator, &buyer);
    assert_eq!(result, Err(Ok(ContractError::NoDividendClaimable)));
}

#[test]
fn test_claim_dividend_happy_path_returns_correct_amount() {
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
    let claimed = client.claim_dividend(&creator, &buyer);
    assert_eq!(claimed, expected);
}

#[test]
fn test_claim_dividend_resets_claimable_to_zero() {
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

    assert_eq!(client.get_claimable_dividend(&creator, &buyer), 0);
}

#[test]
fn test_double_claim_fails_with_no_claimable() {
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

    let result = client.try_claim_dividend(&creator, &buyer);
    assert_eq!(result, Err(Ok(ContractError::NoDividendClaimable)));
}

#[test]
fn test_claim_dividend_while_paused_fails() {
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

    let result = client.try_claim_dividend(&creator, &buyer);
    assert_eq!(result, Err(Ok(ContractError::ProtocolPaused)));
}

#[test]
fn test_claim_dividend_after_sell_captures_pending() {
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

    // Seller exits; settlement should save dividends to pending.
    client.sell_key(&creator, &buyer, &None);

    // After selling all keys, holder can still claim previously earned dividends.
    let expected = compute_expected_holder_dividend(amount, 1, 1, DEFAULT_PROTOCOL_BPS);
    let claimed = client.claim_dividend(&creator, &buyer);
    assert_eq!(claimed, expected);
}

#[test]
fn test_claim_dividend_proportional_amounts_across_holders() {
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
    client.buy_key(&creator, &whale, &100, &None);
    client.buy_key(&creator, &whale, &100, &None);
    client.buy_key(&creator, &small, &100, &None);

    let distributor = Address::generate(&env);
    let amount = 10_000i128;
    distribute_test_dividend(&client, &creator, &distributor, amount);

    let whale_claimed = client.claim_dividend(&creator, &whale);
    let small_claimed = client.claim_dividend(&creator, &small);

    let expected_whale = compute_expected_holder_dividend(amount, 2, 3, DEFAULT_PROTOCOL_BPS);
    let expected_small = compute_expected_holder_dividend(amount, 1, 3, DEFAULT_PROTOCOL_BPS);
    assert_eq!(whale_claimed, expected_whale);
    assert_eq!(small_claimed, expected_small);
}
