#![cfg(test)]

use crate::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn register_creator(env: &Env, client: &CreatorKeysContractClient, creator: &Address) {
    client.register_creator(
        &crate::RegisterCreatorParams {
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
fn test_graduated_curve_applies_correct_exponent_by_supply_tier() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_key_price(&admin, &100i128);
    client.set_curve_slope(&admin, &100i128);

    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let milestones = soroban_sdk::Vec::from_array(&env, [(10u32, 2u32), (25u32, 3u32)]);
    client.set_graduated_curve(&creator, &milestones);

    let price_5 = env.as_contract(&contract_id, || {
        crate::compute_bonding_curve_price(&env, &creator, 100, 5)
    });
    let price_10 = env.as_contract(&contract_id, || {
        crate::compute_bonding_curve_price(&env, &creator, 100, 10)
    });
    let price_30 = env.as_contract(&contract_id, || {
        crate::compute_bonding_curve_price(&env, &creator, 100, 30)
    });
    assert_eq!(price_5, Ok(600));
    assert_eq!(price_10, Ok(10100));
    assert_eq!(price_30, Ok(2700100));
}

#[test]
fn test_graduated_curve_rejects_configuration_after_sales_begin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_key_price(&admin, &100i128);
    client.set_curve_slope(&admin, &100i128);
    // Disable the circuit breaker so the buy below settles normally and
    // establishes positive supply before configuring the curve.
    client.set_circuit_breaker_threshold(&admin, &10_000u32);

    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000i128, &None);

    let milestones = soroban_sdk::Vec::from_array(&env, [(10u32, 2u32)]);
    let result = client.try_set_graduated_curve(&creator, &milestones);
    assert!(result.is_err());
}
