#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, TryFromVal,
};

fn setup_test() -> (Env, VestingContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init(&admin);
    (env, client, admin)
}

#[test]
fn test_init_and_admin_management() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    assert_eq!(client.get_admin(), None);

    client.init(&admin);
    assert_eq!(client.get_admin(), Some(admin.clone()));

    // Double init fails
    assert_eq!(
        client.try_init(&admin),
        Err(Ok(ContractError::AlreadyInitialized))
    );

    // Update admin
    let new_admin = Address::generate(&env);
    client.set_admin(&new_admin);
    assert_eq!(client.get_admin(), Some(new_admin));
}

#[test]
fn test_create_vest_validations() {
    let (env, client, _admin) = setup_test();
    let wallet = Address::generate(&env);
    let key_id = Address::generate(&env);

    // Zero or negative amount
    assert_eq!(
        client.try_create_vest(&wallet, &key_id, &0, &100, &1_000),
        Err(Ok(ContractError::NotPositiveAmount))
    );
    assert_eq!(
        client.try_create_vest(&wallet, &key_id, &-500, &100, &1_000),
        Err(Ok(ContractError::NotPositiveAmount))
    );

    // Zero duration
    assert_eq!(
        client.try_create_vest(&wallet, &key_id, &10_000, &100, &0),
        Err(Ok(ContractError::InvalidDuration))
    );

    // Cliff exceeds duration
    assert_eq!(
        client.try_create_vest(&wallet, &key_id, &10_000, &1_500, &1_000),
        Err(Ok(ContractError::InvalidCliff))
    );

    // Valid vest creation
    let vest_id = client.create_vest(&wallet, &key_id, &10_000, &200, &1_000);
    assert_eq!(vest_id, 1);

    let schedule = client.get_vest(&vest_id);
    assert_eq!(schedule.vest_id, 1);
    assert_eq!(schedule.wallet, wallet);
    assert_eq!(schedule.key_id, key_id);
    assert_eq!(schedule.amount, 10_000);
    assert_eq!(schedule.claimed_amount, 0);
    assert_eq!(schedule.cliff_seconds, 200);
    assert_eq!(schedule.duration_seconds, 1_000);
}

#[test]
fn test_claim_vested_blocked_before_cliff() {
    let (env, client, _admin) = setup_test();
    let wallet = Address::generate(&env);
    let key_id = Address::generate(&env);

    let cliff_secs = 500u64;
    let duration_secs = 2_000u64;
    let amount = 100_000i128;

    env.ledger().set_timestamp(1_000);
    let vest_id = client.create_vest(&wallet, &key_id, &amount, &cliff_secs, &duration_secs);

    // Right at start time
    assert_eq!(
        client.try_claim_vested(&vest_id),
        Err(Ok(ContractError::CliffNotReached))
    );

    // Just before cliff (start + cliff - 1 = 1_000 + 500 - 1 = 1_499)
    env.ledger().set_timestamp(1_499);
    assert_eq!(
        client.try_claim_vested(&vest_id),
        Err(Ok(ContractError::CliffNotReached))
    );

    // View function reports claimable 0 before cliff
    let info = client.get_vested_amount(&vest_id);
    assert_eq!(info.total, amount);
    assert_eq!(info.claimed, 0);
    assert_eq!(info.claimable, 0);
}

#[test]
fn test_linear_release_and_partial_claims() {
    let (env, client, _admin) = setup_test();
    let wallet = Address::generate(&env);
    let key_id = Address::generate(&env);

    let amount = 10_000i128;
    let cliff_secs = 100u64;
    let duration_secs = 1_000u64;

    env.ledger().set_timestamp(0);
    let vest_id = client.create_vest(&wallet, &key_id, &amount, &cliff_secs, &duration_secs);

    // Exactly at cliff (t = 100): 100 / 1000 = 10% = 1,000 unlocked
    env.ledger().set_timestamp(100);
    let info = client.get_vested_amount(&vest_id);
    assert_eq!(info.total, 10_000);
    assert_eq!(info.claimed, 0);
    assert_eq!(info.claimable, 1_000);

    let claimed_1 = client.claim_vested(&vest_id);
    assert_eq!(claimed_1, 1_000);

    // Immediately claiming again fails with NoVestedTokens
    assert_eq!(
        client.try_claim_vested(&vest_id),
        Err(Ok(ContractError::NoVestedTokens))
    );

    // Halfway (t = 500): 500 / 1000 = 50% = 5,000 total vested.
    // Since 1,000 already claimed, claimable is 4,000.
    env.ledger().set_timestamp(500);
    let info_mid = client.get_vested_amount(&vest_id);
    assert_eq!(info_mid.total, 10_000);
    assert_eq!(info_mid.claimed, 1_000);
    assert_eq!(info_mid.claimable, 4_000);

    let claimed_2 = client.claim_vested(&vest_id);
    assert_eq!(claimed_2, 4_000);

    let info_after_mid = client.get_vested_amount(&vest_id);
    assert_eq!(info_after_mid.claimed, 5_000);
    assert_eq!(info_after_mid.claimable, 0);

    // 75% point (t = 750): 7,500 total vested. 5,000 claimed -> 2,500 claimable.
    env.ledger().set_timestamp(750);
    let claimed_3 = client.claim_vested(&vest_id);
    assert_eq!(claimed_3, 2_500);

    // Beyond full duration (t = 1500 > 1000): 10,000 total vested.
    // 7,500 claimed -> remaining 2,500 claimable.
    env.ledger().set_timestamp(1_500);
    let info_end = client.get_vested_amount(&vest_id);
    assert_eq!(info_end.claimed, 7_500);
    assert_eq!(info_end.claimable, 2_500);

    let claimed_4 = client.claim_vested(&vest_id);
    assert_eq!(claimed_4, 2_500);

    // Fully claimed: total = 10,000, claimed = 10,000, claimable = 0
    let info_final = client.get_vested_amount(&vest_id);
    assert_eq!(info_final.total, 10_000);
    assert_eq!(info_final.claimed, 10_000);
    assert_eq!(info_final.claimable, 0);

    // Additional claims rejected
    assert_eq!(
        client.try_claim_vested(&vest_id),
        Err(Ok(ContractError::NoVestedTokens))
    );
}

#[test]
fn test_vest_claimed_event_emission() {
    let (env, client, _admin) = setup_test();
    let wallet = Address::generate(&env);
    let key_id = Address::generate(&env);

    let amount = 20_000i128;
    let cliff_secs = 200u64;
    let duration_secs = 1_000u64;

    env.ledger().set_timestamp(100);
    let vest_id = client.create_vest(&wallet, &key_id, &amount, &cliff_secs, &duration_secs);

    // Advance to 600 (elapsed = 500): 500 / 1000 = 50% = 10,000 claimable
    env.ledger().set_timestamp(600);
    client.claim_vested(&vest_id);

    // Verify VestClaimed event was emitted
    let mut matching_events = soroban_sdk::Vec::new(&env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            if Symbol::try_from_val(&env, &first) == Ok(VEST_CLAIMED_EVENT_NAME) {
                if let Ok(event) = VestClaimed::try_from_val(&env, &data) {
                    matching_events.push_back(event);
                }
            }
        }
    }

    assert_eq!(matching_events.len(), 1);
    let event = matching_events.get(0).unwrap();
    assert_eq!(event.vest_id, vest_id);
    assert_eq!(event.wallet, wallet);
    assert_eq!(event.key_id, key_id);
    assert_eq!(event.amount, 10_000);
    assert_eq!(event.remaining_locked, 10_000);

    // Second claim at t = 1100 (elapsed = 1000): completes vesting
    env.ledger().set_timestamp(1_100);
    client.claim_vested(&vest_id);

    let mut second_events = soroban_sdk::Vec::new(&env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            if Symbol::try_from_val(&env, &first) == Ok(VEST_CLAIMED_EVENT_NAME) {
                if let Ok(event) = VestClaimed::try_from_val(&env, &data) {
                    second_events.push_back(event);
                }
            }
        }
    }

    assert_eq!(second_events.len(), 1);
    let second_event = second_events.get(0).unwrap();
    assert_eq!(second_event.vest_id, vest_id);
    assert_eq!(second_event.amount, 10_000);
    assert_eq!(second_event.remaining_locked, 0);
}

#[test]
fn test_zero_cliff_linear_vesting() {
    let (env, client, _admin) = setup_test();
    let wallet = Address::generate(&env);
    let key_id = Address::generate(&env);

    let amount = 50_000i128;
    let cliff_secs = 0u64;
    let duration_secs = 500u64;

    env.ledger().set_timestamp(1_000);
    let vest_id = client.create_vest(&wallet, &key_id, &amount, &cliff_secs, &duration_secs);

    // At t = 1_100 (100s elapsed): 100 / 500 = 20% = 10,000 unlocked immediately
    env.ledger().set_timestamp(1_100);
    let info = client.get_vested_amount(&vest_id);
    assert_eq!(info.claimable, 10_000);

    let claimed = client.claim_vested(&vest_id);
    assert_eq!(claimed, 10_000);
}

#[test]
fn test_cliff_equals_duration_all_or_nothing() {
    let (env, client, _admin) = setup_test();
    let wallet = Address::generate(&env);
    let key_id = Address::generate(&env);

    let amount = 25_000i128;
    let cliff_secs = 1_000u64;
    let duration_secs = 1_000u64;

    env.ledger().set_timestamp(0);
    let vest_id = client.create_vest(&wallet, &key_id, &amount, &cliff_secs, &duration_secs);

    // At 999s: 0 claimable, claim blocked
    env.ledger().set_timestamp(999);
    assert_eq!(client.get_vested_amount(&vest_id).claimable, 0);
    assert_eq!(
        client.try_claim_vested(&vest_id),
        Err(Ok(ContractError::CliffNotReached))
    );

    // At 1000s: 100% claimable
    env.ledger().set_timestamp(1_000);
    assert_eq!(client.get_vested_amount(&vest_id).claimable, 25_000);

    let claimed = client.claim_vested(&vest_id);
    assert_eq!(claimed, 25_000);
    assert_eq!(client.get_vested_amount(&vest_id).claimed, 25_000);
}

#[test]
fn test_multiple_independent_vests() {
    let (env, client, _admin) = setup_test();
    let wallet_a = Address::generate(&env);
    let wallet_b = Address::generate(&env);
    let key_1 = Address::generate(&env);
    let key_2 = Address::generate(&env);

    env.ledger().set_timestamp(100);
    let id_a = client.create_vest(&wallet_a, &key_1, &10_000, &100, &1_000);
    let id_b = client.create_vest(&wallet_b, &key_2, &20_000, &200, &2_000);

    assert_eq!(id_a, 1);
    assert_eq!(id_b, 2);

    // At t = 250 (150s elapsed):
    // id_a (cliff 100): past cliff -> 150/1000 = 1,500 claimable
    // id_b (cliff 200): before cliff -> 0 claimable
    env.ledger().set_timestamp(250);
    assert_eq!(client.get_vested_amount(&id_a).claimable, 1_500);
    assert_eq!(client.get_vested_amount(&id_b).claimable, 0);

    let claimed_a = client.claim_vested(&id_a);
    assert_eq!(claimed_a, 1_500);
    assert_eq!(
        client.try_claim_vested(&id_b),
        Err(Ok(ContractError::CliffNotReached))
    );
}

#[test]
fn test_non_existent_vest_fails() {
    let (_env, client, _admin) = setup_test();

    assert_eq!(
        client.try_get_vest(&999),
        Err(Ok(ContractError::VestNotFound))
    );
    assert_eq!(
        client.try_get_vested_amount(&999),
        Err(Ok(ContractError::VestNotFound))
    );
    assert_eq!(
        client.try_claim_vested(&999),
        Err(Ok(ContractError::VestNotFound))
    );
}
