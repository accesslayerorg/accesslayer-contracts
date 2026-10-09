mod contract_test_env;
use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::Address;

#[test]
fn claim_before_cliff_reverts_with_no_claimable() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "vestcreator");
    let beneficiary = Address::generate(&env);
    client.create_vesting_cliff(
        &creator,
        &creator,
        &beneficiary,
        &10_000i128,
        &1_000u64,
        &0u64,
        &10_000u64,
    );
    let result = client.try_claim_vested_cliff(&creator, &beneficiary);
    assert!(result.is_err());
}

#[test]
fn claim_mid_vesting_returns_linear_amount() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "vestcreator2");
    let beneficiary = Address::generate(&env);
    client.create_vesting_cliff(
        &creator,
        &creator,
        &beneficiary,
        &10_000i128,
        &1_000u64,
        &0u64,
        &10_000u64,
    );
    let mut l = env.ledger().get();
    l.timestamp = 5_000;
    env.ledger().set(l);
    let claimed = client.claim_vested_cliff(&creator, &beneficiary);
    assert_eq!(claimed, 5_000);
    let info = client.get_vesting_info(&creator, &beneficiary);
    assert_eq!(info.claimed_amount, 5_000);
    assert_eq!(info.claimable_amount, 0);
}

#[test]
fn claim_after_full_duration_returns_full_allocation() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "vestcreator3");
    let beneficiary = Address::generate(&env);
    client.create_vesting_cliff(
        &creator,
        &creator,
        &beneficiary,
        &10_000i128,
        &1_000u64,
        &0u64,
        &10_000u64,
    );
    let mut l = env.ledger().get();
    l.timestamp = 20_000;
    env.ledger().set(l);
    let claimed = client.claim_vested_cliff(&creator, &beneficiary);
    assert_eq!(claimed, 10_000);
}

#[test]
fn duplicate_schedule_is_rejected() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "vestcreator5");
    let beneficiary = Address::generate(&env);
    client.create_vesting_cliff(
        &creator,
        &creator,
        &beneficiary,
        &10_000i128,
        &1_000u64,
        &0u64,
        &10_000u64,
    );
    let result = client.try_create_vesting_cliff(
        &creator,
        &creator,
        &beneficiary,
        &5_000i128,
        &0u64,
        &0u64,
        &5_000u64,
    );
    assert!(result.is_err());
}
