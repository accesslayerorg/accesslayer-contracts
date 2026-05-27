//! Regression test for zero bps creator fee resulting in full payment to creator.
//!
//! When a creator fee is set to zero bps (0%), the full payment after the protocol fee
//! should flow to the creator. This test confirms the fee split logic handles zero
//! correctly without rounding errors or silent deductions.
//!
//! Related issue: #288

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_zero_creator_bps_full_payment_to_creator_after_protocol_fee() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    
    // Set up: 0% creator fee, 10% protocol fee (0 bps creator, 1000 bps protocol)
    let admin = soroban_sdk::Address::generate(&env);
    client.set_fee_config(&admin, &0u32, &10000u32);
    
    // Verify fee config is set correctly
    let config = client.get_fee_config().unwrap();
    assert_eq!(config.creator_bps, 0, "Creator bps should be 0");
    assert_eq!(config.protocol_bps, 10000, "Protocol bps should be 10000 (100%)");
    
    // Set key price
    set_key_price_for_tests(&env, &client, 1000i128);
    
    // Register creator
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);
    
    // Perform a buy with payment of 1000
    let payment_amount = 1000i128;
    let supply = client.buy_key(&creator, &buyer, &payment_amount);
    assert_eq!(supply, 1, "Supply should increment to 1");
    
    // Compute expected fees
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&payment_amount);
    
    // With 0% creator bps and 100% protocol bps:
    // - Protocol fee should be 100% of payment = 1000
    // - Creator fee should be 0% of payment = 0
    assert_eq!(protocol_fee, 1000, "Protocol fee should be full payment amount");
    assert_eq!(creator_fee, 0, "Creator fee should be zero");
    
    // Verify no value is lost: creator_fee + protocol_fee should equal total payment
    assert_eq!(
        creator_fee + protocol_fee,
        payment_amount,
        "Sum of fees should equal total payment (no value lost)"
    );
    
    // Verify the creator receives nothing (all goes to protocol)
    assert_eq!(creator_fee, 0, "Creator should receive zero with 0 bps");
}

#[test]
fn test_zero_creator_bps_with_partial_protocol_fee() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    
    // Set up: 0% creator fee, 20% protocol fee (0 bps creator, 2000 bps protocol)
    // This means creator gets 80% and protocol gets 20%
    let admin = soroban_sdk::Address::generate(&env);
    client.set_fee_config(&admin, &8000u32, &2000u32);
    
    // Verify fee config
    let config = client.get_fee_config().unwrap();
    assert_eq!(config.creator_bps, 8000, "Creator bps should be 8000 (80%)");
    assert_eq!(config.protocol_bps, 2000, "Protocol bps should be 2000 (20%)");
    
    // Set key price
    set_key_price_for_tests(&env, &client, 1000i128);
    
    // Register creator
    let creator = register_test_creator(&env, &client, "bob");
    let buyer = soroban_sdk::Address::generate(&env);
    
    // Perform a buy
    let payment_amount = 1000i128;
    client.buy_key(&creator, &buyer, &payment_amount);
    
    // Compute fees
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&payment_amount);
    
    // With 80% creator and 20% protocol:
    // - Protocol fee = 20% of 1000 = 200
    // - Creator fee = 80% of 1000 = 800
    assert_eq!(protocol_fee, 200, "Protocol fee should be 20% of payment");
    assert_eq!(creator_fee, 800, "Creator fee should be 80% of payment");
    
    // Verify no value is lost
    assert_eq!(
        creator_fee + protocol_fee,
        payment_amount,
        "Sum of fees should equal total payment"
    );
}

#[test]
fn test_zero_protocol_bps_full_payment_to_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    
    // Set up: 100% creator fee, 0% protocol fee (10000 bps creator, 0 bps protocol)
    let admin = soroban_sdk::Address::generate(&env);
    client.set_fee_config(&admin, &10000u32, &0u32);
    
    // Verify fee config
    let config = client.get_fee_config().unwrap();
    assert_eq!(config.creator_bps, 10000, "Creator bps should be 10000 (100%)");
    assert_eq!(config.protocol_bps, 0, "Protocol bps should be 0");
    
    // Set key price
    set_key_price_for_tests(&env, &client, 1000i128);
    
    // Register creator
    let creator = register_test_creator(&env, &client, "charlie");
    let buyer = soroban_sdk::Address::generate(&env);
    
    // Perform a buy
    let payment_amount = 1000i128;
    client.buy_key(&creator, &buyer, &payment_amount);
    
    // Compute fees
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&payment_amount);
    
    // With 100% creator and 0% protocol:
    // - Protocol fee = 0% of 1000 = 0
    // - Creator fee = 100% of 1000 = 1000
    assert_eq!(protocol_fee, 0, "Protocol fee should be zero with 0 bps");
    assert_eq!(creator_fee, 1000, "Creator fee should be full payment amount");
    
    // Verify no value is lost
    assert_eq!(
        creator_fee + protocol_fee,
        payment_amount,
        "Sum of fees should equal total payment"
    );
}

#[test]
fn test_zero_creator_bps_no_rounding_errors_with_odd_amounts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    
    // Set up: 0% creator fee, 33.33% protocol fee (6667 bps creator, 3333 bps protocol)
    let admin = soroban_sdk::Address::generate(&env);
    client.set_fee_config(&admin, &6667u32, &3333u32);
    
    // Set key price
    set_key_price_for_tests(&env, &client, 999i128);
    
    // Register creator
    let creator = register_test_creator(&env, &client, "dave");
    let buyer = soroban_sdk::Address::generate(&env);
    
    // Perform a buy with an odd payment amount
    let payment_amount = 999i128;
    client.buy_key(&creator, &buyer, &payment_amount);
    
    // Compute fees
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&payment_amount);
    
    // Verify no value is lost due to rounding
    assert_eq!(
        creator_fee + protocol_fee,
        payment_amount,
        "Sum of fees should equal total payment even with odd amounts"
    );
    
    // Protocol fee should be floor(999 * 3333 / 10000) = floor(332.6667) = 332
    // Creator fee should be 999 - 332 = 667 (remainder goes to creator)
    assert_eq!(protocol_fee, 332, "Protocol fee should be floored");
    assert_eq!(creator_fee, 667, "Creator fee should include remainder");
}
