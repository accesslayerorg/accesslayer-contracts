#![cfg(test)]

use crate::{ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup_test() -> (Env, CreatorKeysContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &100i128);
    client.set_curve_slope(&admin, &100i128);
    client.set_fee_config(&admin, &9000u32, &1000u32);

    (env, client, admin, treasury)
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient, creator: &Address) {
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

#[test]
fn test_circuit_breaker_threshold_configuration_and_trigger() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    // Default threshold is 30%.
    // Buy 1: supply 0 -> 1. Price moves from base_price (100) to 200 (100% increase > 30%).
    let buyer = Address::generate(&env);
    let result = client.try_buy_key(&creator, &buyer, &1000i128, &None);
    assert_eq!(result, Err(Ok(ContractError::CircuitBreakerTriggered)));

    // Admin sets threshold to 200% (200)
    client.set_circuit_breaker_threshold(&admin, &200u32);

    // Now buy succeeds because price delta (100%) < 200% threshold
    let supply = client.buy_key(&creator, &buyer, &1000i128, &None);
    assert_eq!(supply, 1);
}

#[test]
fn test_referral_system_fee_split_and_validation() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    // Set high threshold so buy doesn't trigger circuit breaker
    client.set_circuit_breaker_threshold(&admin, &500u32);

    let buyer = Address::generate(&env);
    let referrer = Address::generate(&env);

    // Buyer or creator as referrer panics with InvalidReferrer
    let res_buyer_ref =
        client.try_buy_key_with_referrer(&creator, &buyer, &1000i128, &None, &Some(buyer.clone()));
    assert_eq!(res_buyer_ref, Err(Ok(ContractError::InvalidReferrer)));

    let res_creator_ref = client.try_buy_key_with_referrer(
        &creator,
        &buyer,
        &1000i128,
        &None,
        &Some(creator.clone()),
    );
    assert_eq!(res_creator_ref, Err(Ok(ContractError::InvalidReferrer)));

    // Valid referral buy
    // Price at supply 0 is 100. Protocol fee at 10% (1000 bps) is 10.
    // Treasury gets 50% (5), referrer gets 50% (5).
    let treasury_bal_before = client.get_treasury_balance();
    client.buy_key_with_referrer(&creator, &buyer, &1000i128, &None, &Some(referrer.clone()));

    let treasury_bal_after = client.get_treasury_balance();
    assert_eq!(treasury_bal_after - treasury_bal_before, 5);

    let ref_earnings = client.get_referral_earnings(&referrer);
    assert_eq!(ref_earnings, 5);

    // Buy without referrer sends full protocol fee (20) to treasury (price at supply 1 is 200, 10% = 20)
    let buyer2 = Address::generate(&env);
    let treasury_bal_before2 = client.get_treasury_balance();
    client.buy_key(&creator, &buyer2, &1000i128, &None);
    let treasury_bal_after2 = client.get_treasury_balance();
    assert_eq!(treasury_bal_after2 - treasury_bal_before2, 20);
}

#[test]
fn test_whitelist_mode_and_permissions() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    client.set_circuit_breaker_threshold(&admin, &500u32);

    let wallet = Address::generate(&env);
    let attacker = Address::generate(&env);

    // Non-creator caller panics with NotRegistered on whitelist functions
    assert_eq!(
        client.try_enable_whitelist(&attacker),
        Err(Ok(ContractError::NotRegistered))
    );
    assert_eq!(
        client.try_disable_whitelist(&attacker),
        Err(Ok(ContractError::NotRegistered))
    );
    assert_eq!(
        client.try_add_to_whitelist(&attacker, &wallet),
        Err(Ok(ContractError::NotRegistered))
    );
    assert_eq!(
        client.try_remove_from_whitelist(&attacker, &wallet),
        Err(Ok(ContractError::NotRegistered))
    );

    // Enable whitelist
    client.enable_whitelist(&creator);

    // Buy by non-whitelisted wallet fails with NotWhitelisted
    assert_eq!(
        client.try_buy_key(&creator, &wallet, &1000i128, &None),
        Err(Ok(ContractError::NotWhitelisted))
    );

    // Add to whitelist
    client.add_to_whitelist(&creator, &wallet);

    // Buy by whitelisted wallet succeeds
    let supply = client.buy_key(&creator, &wallet, &1000i128, &None);
    assert_eq!(supply, 1);

    // Remove from whitelist
    client.remove_from_whitelist(&creator, &wallet);
    let buyer2 = Address::generate(&env);
    assert_eq!(
        client.try_buy_key(&creator, &buyer2, &1000i128, &None),
        Err(Ok(ContractError::NotWhitelisted))
    );

    // Disable whitelist mode
    client.disable_whitelist(&creator);
    // Any wallet can buy now
    let supply2 = client.buy_key(&creator, &buyer2, &1000i128, &None);
    assert_eq!(supply2, 2);
}

#[test]
fn test_key_burn_reduces_supply_and_balance() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    client.set_circuit_breaker_threshold(&admin, &500u32);

    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &1000i128, &None);

    let balance_before = client.get_key_balance(&creator, &holder);
    let supply_before = client.get_creator_supply(&creator);
    assert_eq!(balance_before, 1);
    assert_eq!(supply_before, 1);

    // Burn with quantity > balance panics with InsufficientBalance
    assert_eq!(
        client.try_burn(&holder, &creator, &2u32),
        Err(Ok(ContractError::InsufficientBalance))
    );

    // Burn 1 key
    let new_supply = client.burn(&holder, &creator, &1u32);
    assert_eq!(new_supply, 0);

    let balance_after = client.get_key_balance(&creator, &holder);
    let supply_after = client.get_creator_supply(&creator);
    assert_eq!(balance_after, 0);
    assert_eq!(supply_after, 0);
}
