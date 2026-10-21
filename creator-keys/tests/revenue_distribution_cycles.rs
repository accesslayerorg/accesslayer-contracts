//! Tests for cycle-based protocol revenue distribution (`accumulate_fees`,
//! `distribute_cycle`, `claim_cycle`).

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_ledger_sequence,
    test_env_with_auths,
};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

const KEY_PRICE: i128 = 100;

/// Admin, creator, and two stakers holding 1 and 2 staked keys respectively.
fn setup(
    env: &Env,
) -> (
    CreatorKeysContractClient<'_>,
    Address,
    Address,
    Address,
    Address,
) {
    let (client, _) = register_creator_keys(env);
    let admin = set_key_price_for_tests(env, &client, KEY_PRICE);
    client.set_protocol_admin(&admin, &admin);
    let creator = register_test_creator(env, &client, "alice");
    let staker_a = Address::generate(env);
    let staker_b = Address::generate(env);
    set_ledger_sequence(env, 100);
    client.buy_keys(&creator, &staker_a, &1u32, &10_000i128, &None);
    client.buy_keys(&creator, &staker_b, &2u32, &10_000i128, &None);
    client.stake_keys(&creator, &staker_a, &1u32);
    client.stake_keys(&creator, &staker_b, &2u32);
    (client, admin, creator, staker_a, staker_b)
}

fn holders(env: &Env, a: &Address, b: &Address) -> Vec<Address> {
    let mut list = Vec::new(env);
    list.push_back(a.clone());
    list.push_back(b.clone());
    list
}

#[test]
fn test_full_distribute_and_claim_cycle() {
    let env = test_env_with_auths();
    let (client, admin, creator, a, b) = setup(&env);

    client.accumulate_fees(&admin, &creator, &1_000i128);
    client.accumulate_fees(&admin, &creator, &1_000i128);
    assert_eq!(client.get_revenue_pool(&creator), 2_000);

    let cycle = client.distribute_cycle(&admin, &creator, &holders(&env, &a, &b));
    assert_eq!(cycle, 1);
    // 2_000 split 1:2 -> 666 / 1_333, dust of 1 stays in the pool.
    assert_eq!(client.get_revenue_pool(&creator), 1);

    assert_eq!(client.claim_cycle(&creator, &a, &cycle), 666);
    assert_eq!(client.claim_cycle(&creator, &b, &cycle), 1_333);
}

#[test]
fn test_double_claim_in_same_cycle_rejected() {
    let env = test_env_with_auths();
    let (client, admin, creator, a, b) = setup(&env);
    client.accumulate_fees(&admin, &creator, &900i128);
    let cycle = client.distribute_cycle(&admin, &creator, &holders(&env, &a, &b));

    client.claim_cycle(&creator, &a, &cycle);
    assert_eq!(
        client.try_claim_cycle(&creator, &a, &cycle),
        Err(Ok(ContractError::AlreadyClaimed))
    );
}

#[test]
fn test_cycle_length_gates_next_distribution() {
    let env = test_env_with_auths();
    let (client, admin, creator, a, b) = setup(&env);
    client.set_distribution_cycle_length(&admin, &50u32);

    client.accumulate_fees(&admin, &creator, &900i128);
    client.distribute_cycle(&admin, &creator, &holders(&env, &a, &b));

    client.accumulate_fees(&admin, &creator, &900i128);
    assert_eq!(
        client.try_distribute_cycle(&admin, &creator, &holders(&env, &a, &b)),
        Err(Ok(ContractError::CooldownActive))
    );

    set_ledger_sequence(&env, 150);
    assert_eq!(
        client.distribute_cycle(&admin, &creator, &holders(&env, &a, &b)),
        2
    );
}

#[test]
fn test_non_admin_and_invalid_inputs_rejected() {
    let env = test_env_with_auths();
    let (client, admin, creator, a, b) = setup(&env);
    let stranger = Address::generate(&env);

    assert_eq!(
        client.try_accumulate_fees(&stranger, &creator, &100i128),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        client.try_accumulate_fees(&admin, &creator, &0i128),
        Err(Ok(ContractError::ZeroDistributionAmount))
    );
    assert_eq!(
        client.try_set_distribution_cycle_length(&admin, &0u32),
        Err(Ok(ContractError::NotPositiveAmount))
    );
    assert_eq!(
        client.try_distribute_cycle(&admin, &creator, &holders(&env, &a, &b)),
        Err(Ok(ContractError::ZeroDistributionAmount))
    );
    assert_eq!(
        client.try_claim_cycle(&creator, &a, &1u32),
        Err(Ok(ContractError::NoDividendClaimable))
    );
}
