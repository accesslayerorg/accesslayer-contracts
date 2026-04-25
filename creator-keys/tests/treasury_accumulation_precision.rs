//! Precision tests for repeated treasury-fee accumulation.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, set_protocol_fee_bps, set_test_timestamp, test_env_with_auths,
    DEFAULT_TEST_TIMESTAMP,
};
use creator_keys::CreatorKeysContractClient;
use soroban_sdk::Env;

fn setup_with_fee_config<'a>(
    env: &'a Env,
    creator_bps: u32,
    protocol_bps: u32,
) -> CreatorKeysContractClient<'a> {
    let (client, _) = register_creator_keys(env);
    set_protocol_fee_bps(env, &client, creator_bps, protocol_bps);
    client
}

#[test]
fn test_treasury_accumulation_precision_repeated_trades_has_no_drift() {
    let env = test_env_with_auths();
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP);
    let client = setup_with_fee_config(&env, 9000, 1000);

    let trade_amount = 123_i128;
    let iterations = 10_000usize;
    let mut protocol_total = 0_i128;
    let mut creator_total = 0_i128;

    for _ in 0..iterations {
        let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&trade_amount);
        creator_total = creator_total.checked_add(creator_fee).unwrap();
        protocol_total = protocol_total.checked_add(protocol_fee).unwrap();
    }

    let expected_gross = trade_amount * iterations as i128;
    assert_eq!(
        creator_total + protocol_total,
        expected_gross,
        "accumulated fee buckets must conserve total volume exactly"
    );
}

#[test]
fn test_treasury_accumulation_precision_small_value_trade_loop() {
    let env = test_env_with_auths();
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP);
    let client = setup_with_fee_config(&env, 9000, 1000);

    let trade_amount = 1_i128;
    let iterations = 5_000usize;
    let mut protocol_total = 0_i128;
    let mut creator_total = 0_i128;

    for _ in 0..iterations {
        let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&trade_amount);
        creator_total = creator_total.checked_add(creator_fee).unwrap();
        protocol_total = protocol_total.checked_add(protocol_fee).unwrap();
    }

    assert_eq!(
        protocol_total, 0,
        "protocol fee should floor to zero for dust"
    );
    assert_eq!(
        creator_total, iterations as i128,
        "creator receives full dust amount when protocol fee floors to zero"
    );
}
