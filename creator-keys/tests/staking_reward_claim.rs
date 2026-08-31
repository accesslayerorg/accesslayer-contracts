//! Tests for the key staking rewards feature: `stake_keys` locks a holder's
//! keys for `STAKE_LOCK_LEDGERS` (30 days), a share of every protocol fee
//! accrues into the creator's `staking_rewards` pool, and `claim_stake_reward`
//! pays out a pro-rata reward and unlocks the keys once the lock expires.
//!
//! Acceptance criteria covered:
//! - Staked keys are removed from the transferable/liquid balance.
//! - Selling staked keys panics with `KeysStaked`.
//! - `claim_stake_reward` before the lock period expires panics with
//!   `StakeLockActive`.
//! - The reward is proportional to the staked quantity relative to the total
//!   staked for the creator.
//! - Keys are returned to the holder's balance on claim.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_ledger_sequence, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::{events, ContractError, StakingError, STAKE_LOCK_LEDGERS};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

/// Configure TTL windows wide enough for the 30-day lock period and advance
/// the ledger past `STAKE_LOCK_LEDGERS`.
fn advance_past_lock(env: &soroban_sdk::Env) {
    let start_sequence = env.ledger().sequence();
    set_ledger_sequence(env, start_sequence + STAKE_LOCK_LEDGERS);
}

#[test]
fn test_claim_stake_reward_fails_with_no_stake() {
    let env = test_env_with_auths();
    env.ledger().set_max_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    env.ledger()
        .set_min_persistent_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    let result = client.try_claim_stake_reward(&creator, &holder, &0u32);
    assert_eq!(result, Err(Ok(StakingError::PositionNotFound)));
}

#[test]
fn test_claim_stake_reward_fails_while_lock_is_active() {
    let env = test_env_with_auths();
    env.ledger().set_max_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    env.ledger()
        .set_min_persistent_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    let stake_id = client.stake_keys(&creator, &holder, &1u32);

    // Claim while the lock is still active reverts with StakeLockActive.
    let result = client.try_claim_stake_reward(&creator, &holder, &stake_id);
    assert_eq!(result, Err(Ok(StakingError::StakeLockActive)));

    // Still locked just one ledger before the unlock boundary.
    let position = client
        .get_staking_position(&creator, &holder, &stake_id)
        .unwrap();
    set_ledger_sequence(&env, position.unlock_ledger - 1);
    let result = client.try_claim_stake_reward(&creator, &holder, &stake_id);
    assert_eq!(result, Err(Ok(StakingError::StakeLockActive)));
}

#[test]
fn test_claim_stake_reward_pays_out_and_unlocks_after_lock_period() {
    let env = test_env_with_auths();
    env.ledger().set_max_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    env.ledger()
        .set_min_persistent_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    let pool_after_buy = client.get_staking_rewards_pool(&creator);
    assert!(
        pool_after_buy > 0,
        "buying with a fee config configured should seed the staking rewards pool"
    );

    // Staked keys are removed from the liquid/transferable balance.
    let stake_id = client.stake_keys(&creator, &holder, &1u32);
    assert_eq!(client.get_total_staked(&creator), 1);
    assert_eq!(client.get_staked_balance(&creator, &holder), 1);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 0);

    advance_past_lock(&env);

    let claim = client.claim_stake_reward(&creator, &holder, &stake_id);
    assert_eq!(claim.amount, 1);
    // Sole staker gets the entire pool.
    assert_eq!(claim.reward, pool_after_buy);

    // Pool and total-staked bookkeeping are cleared.
    assert_eq!(client.get_staking_rewards_pool(&creator), 0);
    assert_eq!(client.get_total_staked(&creator), 0);
    assert_eq!(client.get_staked_balance(&creator, &holder), 0);
    assert!(client
        .get_staking_position(&creator, &holder, &stake_id)
        .is_none());

    // Keys are returned to the holder's balance on claim.
    assert_eq!(client.get_key_balance(&creator, &holder), 1);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 1);
}

#[test]
fn test_claim_stake_reward_splits_pool_pro_rata_across_stakers() {
    let env = test_env_with_auths();
    env.ledger().set_max_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    env.ledger()
        .set_min_persistent_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);

    // holder_a buys and stakes 3 keys, holder_b buys and stakes 1.
    for _ in 0..3 {
        client.buy_key(&creator, &holder_a, &KEY_PRICE, &None);
    }
    client.buy_key(&creator, &holder_b, &KEY_PRICE, &None);

    let stake_a = client.stake_keys(&creator, &holder_a, &3u32);
    let stake_b = client.stake_keys(&creator, &holder_b, &1u32);
    assert_eq!(client.get_total_staked(&creator), 4);

    let pool_total = client.get_staking_rewards_pool(&creator);
    let expected_a = pool_total * 3 / 4;
    let expected_b_before_a_claims = pool_total / 4;

    advance_past_lock(&env);

    let claim_a = client.claim_stake_reward(&creator, &holder_a, &stake_a);
    assert_eq!(claim_a.reward, expected_a);
    assert_eq!(client.get_total_staked(&creator), 1);

    let claim_b = client.claim_stake_reward(&creator, &holder_b, &stake_b);
    // holder_b is the sole remaining staker and claims what's left in the pool,
    // which (absent further rounding loss) matches their pro-rata share.
    assert_eq!(claim_b.reward, expected_b_before_a_claims);
    assert_eq!(client.get_total_staked(&creator), 0);
    assert_eq!(client.get_staking_rewards_pool(&creator), 0);
}

#[test]
fn test_claim_stake_reward_emits_event_with_expected_payload() {
    let env = test_env_with_auths();
    env.ledger().set_max_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    env.ledger()
        .set_min_persistent_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    let (client, contract_id) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    let pool = client.get_staking_rewards_pool(&creator);
    let stake_id = client.stake_keys(&creator, &holder, &1u32);

    advance_past_lock(&env);
    client.claim_stake_reward(&creator, &holder, &stake_id);

    let mut found = false;
    for (contract, topics, data) in env.events().all().iter() {
        if contract != contract_id {
            continue;
        }
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        if event_name == events::STAKE_REWARD_CLAIMED_EVENT_NAME {
            let payload: events::StakeRewardClaimedEvent = data.clone().into_val(&env);
            assert_eq!(payload.holder, holder);
            assert_eq!(payload.creator_id, creator);
            assert_eq!(payload.stake_id, stake_id);
            assert_eq!(payload.amount, 1);
            assert_eq!(payload.reward, pool);
            found = true;
        }
    }
    assert!(found, "expected a StakeRewardClaimed event");
}

#[test]
fn test_claim_stake_reward_fails_while_protocol_paused() {
    let env = test_env_with_auths();
    env.ledger().set_max_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    env.ledger()
        .set_min_persistent_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    let stake_id = client.stake_keys(&creator, &holder, &1u32);

    advance_past_lock(&env);

    client.pause(&admin);

    let result = client.try_claim_stake_reward(&creator, &holder, &stake_id);
    assert_eq!(result, Err(Ok(StakingError::ProtocolPaused)));
}

#[test]
fn test_sell_of_staked_keys_panics_with_keys_staked() {
    let env = test_env_with_auths();
    env.ledger().set_max_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    env.ledger()
        .set_min_persistent_entry_ttl(STAKE_LOCK_LEDGERS + 100_000);
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    // Stake the only key: no liquid balance remains, so a sell attempt must
    // panic with KeysStaked rather than succeed on the staked key.
    client.stake_keys(&creator, &holder, &1u32);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 0);

    let result = client.try_sell_key(&creator, &holder, &None);
    assert_eq!(result, Err(Ok(ContractError::KeysStaked)));
}
