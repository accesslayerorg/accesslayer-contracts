#![cfg(test)]

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::Address;

#[test]
fn test_vesting_lifecycle_success() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let beneficiary = Address::generate(&env);

    let total_keys = 100u32;
    let vesting_period = 100u32;

    // 1. Create vesting schedule
    client.create_vesting(&creator, &beneficiary, &total_keys, &vesting_period);

    let schedule = client
        .get_vesting_schedule(&creator, &beneficiary)
        .expect("Schedule should exist");
    assert_eq!(schedule.beneficiary, beneficiary);
    assert_eq!(schedule.total_keys, 100);
    assert_eq!(schedule.vesting_period_ledgers, 100);
    assert_eq!(schedule.claimed_keys, 0);

    // 2. Advance ledger by 50 (50% vested)
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = schedule.start_ledger + 50;
    env.ledger().set(ledger_info);

    let claimed_50 = client.claim_vested(&creator, &beneficiary);
    assert_eq!(claimed_50, 50);
    assert_eq!(client.get_balance(&creator, &beneficiary), 50);

    let schedule_after_50 = client
        .get_vesting_schedule(&creator, &beneficiary)
        .expect("Schedule should exist");
    assert_eq!(schedule_after_50.claimed_keys, 50);

    // 3. Advance ledger to full completion (>= 100)
    let mut ledger_info_full = env.ledger().get();
    ledger_info_full.sequence_number = schedule.start_ledger + 100;
    env.ledger().set(ledger_info_full);

    let claimed_rest = client.claim_vested(&creator, &beneficiary);
    assert_eq!(claimed_rest, 50);
    assert_eq!(client.get_balance(&creator, &beneficiary), 100);

    let final_schedule = client
        .get_vesting_schedule(&creator, &beneficiary)
        .expect("Schedule should exist");
    assert_eq!(final_schedule.claimed_keys, 100);
}

#[test]
fn test_vesting_already_claimed_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let beneficiary = Address::generate(&env);

    client.create_vesting(&creator, &beneficiary, &100, &100);
    let schedule = client
        .get_vesting_schedule(&creator, &beneficiary)
        .expect("Schedule should exist");

    // Advance 50 ledgers
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = schedule.start_ledger + 50;
    env.ledger().set(ledger_info);

    let _ = client.claim_vested(&creator, &beneficiary);

    // Attempt claiming again without advancing ledger
    let res = client.try_claim_vested(&creator, &beneficiary);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::AlreadyClaimed);
}

#[test]
fn test_vesting_duplicate_schedule_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let beneficiary = Address::generate(&env);

    client.create_vesting(&creator, &beneficiary, &100, &100);

    // Trying to create a duplicate schedule for same creator & beneficiary
    let res = client.try_create_vesting(&creator, &beneficiary, &50, &50);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::AlreadyRegistered);
}

#[test]
fn test_vesting_invalid_parameters() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let beneficiary = Address::generate(&env);

    // 0 keys
    let res1 = client.try_create_vesting(&creator, &beneficiary, &0, &100);
    assert_eq!(res1.unwrap_err().unwrap(), ContractError::NotPositiveAmount);

    // 0 period
    let res2 = client.try_create_vesting(&creator, &beneficiary, &100, &0);
    assert_eq!(res2.unwrap_err().unwrap(), ContractError::NotPositiveAmount);
}
