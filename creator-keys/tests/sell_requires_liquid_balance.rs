//! Tests that verify staked keys cannot be sold and only liquid balance is available for selling.
//!
//! ## Invariants Tested:
//! 1. Liquid balance = Total balance - Staked balance
//! 2. Staked balance ≤ Total balance  
//! 3. Sell operations only consume liquid balance
//! 4. Stake operations only consume liquid balance
//! 5. Total balance = Liquid balance + Staked balance
//! 6. Staking/unstaking does not affect total balance
//! 7. Staked balance is isolated per (creator, holder) pair

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, 100_i128);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

// ── Core Protection Tests ──────────────────────────────────────────────────

#[test]
fn test_sell_reverts_when_attempting_to_use_staked_keys() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..10 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }
    assert_eq!(client.get_key_balance(&creator, &holder), 10);

    client.stake_keys(&creator, &holder, &6);
    assert_eq!(client.get_staked_balance(&creator, &holder), 6);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 4);

    let result = client.try_sell_key(&creator, &holder, &None);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}

#[test]
fn test_sell_succeeds_within_liquid_balance_limit() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..10 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }
    client.stake_keys(&creator, &holder, &6);

    for _ in 0..4 {
        client.sell_key(&creator, &holder, &None);
    }

    assert_eq!(client.get_liquid_balance(&creator, &holder), 0);
    assert_eq!(client.get_staked_balance(&creator, &holder), 6);
    assert_eq!(client.get_key_balance(&creator, &holder), 6);
}

#[test]
fn test_staked_balance_unchanged_after_sell_attempts() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..10 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }
    client.stake_keys(&creator, &holder, &6);

    let _ = client.try_sell_key(&creator, &holder, &None);
    assert_eq!(client.get_staked_balance(&creator, &holder), 6);

    for _ in 0..4 {
        client.sell_key(&creator, &holder, &None);
    }
    assert_eq!(client.get_staked_balance(&creator, &holder), 6);
}

// ── Invariant Tests ────────────────────────────────────────────────────────

#[test]
fn invariant_liquid_equals_total_minus_staked() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..15 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }

    for stake_amount in [0, 5, 10, 15] {
        if stake_amount > 0 {
            client.stake_keys(&creator, &holder, &stake_amount);
        }

        let total = client.get_key_balance(&creator, &holder);
        let staked = client.get_staked_balance(&creator, &holder);
        let liquid = client.get_liquid_balance(&creator, &holder);

        assert_eq!(liquid, total - staked);

        if stake_amount > 0 {
            client.unstake_keys(&creator, &holder, &stake_amount);
        }
    }
}

#[test]
fn invariant_staked_never_exceeds_total() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..10 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }
    client.stake_keys(&creator, &holder, &7);

    let total = client.get_key_balance(&creator, &holder);
    let staked = client.get_staked_balance(&creator, &holder);
    assert!(staked <= total);

    let result = client.try_stake_keys(&creator, &holder, &4);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}

#[test]
fn invariant_total_equals_liquid_plus_staked() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..20 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }

    client.stake_keys(&creator, &holder, &8);
    for _ in 0..5 {
        client.sell_key(&creator, &holder, &None);
    }

    let total = client.get_key_balance(&creator, &holder);
    let staked = client.get_staked_balance(&creator, &holder);
    let liquid = client.get_liquid_balance(&creator, &holder);

    assert_eq!(total, liquid + staked);
}

#[test]
fn invariant_staking_preserves_total_balance() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..12 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }
    let initial_total = client.get_key_balance(&creator, &holder);

    client.stake_keys(&creator, &holder, &5);
    assert_eq!(client.get_key_balance(&creator, &holder), initial_total);

    client.unstake_keys(&creator, &holder, &3);
    assert_eq!(client.get_key_balance(&creator, &holder), initial_total);
}

#[test]
fn invariant_sell_only_reduces_liquid_not_staked() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..20 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }
    client.stake_keys(&creator, &holder, &12);

    let staked_before = client.get_staked_balance(&creator, &holder);

    for _ in 0..8 {
        client.sell_key(&creator, &holder, &None);
    }

    assert_eq!(client.get_staked_balance(&creator, &holder), staked_before);
}

#[test]
fn invariant_staked_isolated_per_creator_holder_pair() {
    let env = test_env_with_auths();
    let (client, creator1) = setup(&env);
    let creator2 = register_test_creator(&env, &client, "bob");
    let holder1 = Address::generate(&env);
    let holder2 = Address::generate(&env);

    for _ in 0..10 {
        client.buy_key(&creator1, &holder1, &100_i128, &None);
    }
    client.stake_keys(&creator1, &holder1, &6);

    for _ in 0..8 {
        client.buy_key(&creator1, &holder2, &100_i128, &None);
    }
    client.stake_keys(&creator1, &holder2, &3);

    for _ in 0..5 {
        client.buy_key(&creator2, &holder1, &100_i128, &None);
    }
    client.stake_keys(&creator2, &holder1, &2);

    assert_eq!(client.get_staked_balance(&creator1, &holder1), 6);
    assert_eq!(client.get_staked_balance(&creator1, &holder2), 3);
    assert_eq!(client.get_staked_balance(&creator2, &holder1), 2);
    assert_eq!(client.get_staked_balance(&creator2, &holder2), 0);
}

// ── Edge Case Tests ────────────────────────────────────────────────────────

#[test]
fn test_stake_zero_amount_fails() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..5 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }

    let result = client.try_stake_keys(&creator, &holder, &0);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn test_unstake_more_than_staked_fails() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..10 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }
    client.stake_keys(&creator, &holder, &5);

    let result = client.try_unstake_keys(&creator, &holder, &6);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}

#[test]
fn test_stake_all_then_unstake_all() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..7 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }

    client.stake_keys(&creator, &holder, &7);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 0);

    let result = client.try_sell_key(&creator, &holder, &None);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));

    client.unstake_keys(&creator, &holder, &7);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 7);

    client.sell_key(&creator, &holder, &None);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 6);
}

#[test]
fn test_buy_after_staking_increases_liquid_only() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..5 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }
    client.stake_keys(&creator, &holder, &3);

    let staked_before = client.get_staked_balance(&creator, &holder);
    let liquid_before = client.get_liquid_balance(&creator, &holder);

    for _ in 0..4 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }

    assert_eq!(client.get_staked_balance(&creator, &holder), staked_before);
    assert_eq!(
        client.get_liquid_balance(&creator, &holder),
        liquid_before + 4
    );
}

#[test]
fn test_partial_unstake_then_sell() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    for _ in 0..10 {
        client.buy_key(&creator, &holder, &100_i128, &None);
    }

    client.stake_keys(&creator, &holder, &10);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 0);

    client.unstake_keys(&creator, &holder, &4);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 4);

    for _ in 0..3 {
        client.sell_key(&creator, &holder, &None);
    }

    assert_eq!(client.get_liquid_balance(&creator, &holder), 1);
    assert_eq!(client.get_staked_balance(&creator, &holder), 6);
}
