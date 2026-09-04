//! Tests for `claim_stake_reward` and the staking rewards pool (#786 / #789 / #806).

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_ledger_sequence, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::{events, StakingError, STAKE_LOCK_LEDGERS};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

#[test]
fn test_claim_stake_reward_fails_with_no_stake() {
    let env = test_env_with_auths();
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
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    let stake_id = client.stake_keys_locked(&creator, &holder, &1u32, &STAKE_LOCK_LEDGERS);

    let result = client.try_claim_stake_reward(&creator, &holder, &stake_id);
    assert_eq!(result, Err(Ok(StakingError::PositionLocked)));

    // Still locked just one ledger before the unlock boundary.
    let position = client
        .get_staking_position(&creator, &holder, &stake_id)
        .unwrap();
    set_ledger_sequence(&env, position.unlock_ledger - 1);
    let result = client.try_claim_stake_reward(&creator, &holder, &stake_id);
    assert_eq!(result, Err(Ok(StakingError::PositionLocked)));
}

#[test]
fn test_claim_stake_reward_pays_out_and_unlocks_after_lock_period() {
    let env = test_env_with_auths();
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

    let stake_id = client.stake_keys_locked(&creator, &holder, &1u32, &STAKE_LOCK_LEDGERS);
    assert_eq!(client.get_total_staked(&creator), 1);

    let position = client
        .get_staking_position(&creator, &holder, &stake_id)
        .unwrap();
    set_ledger_sequence(&env, position.unlock_ledger);

    let claim = client.claim_stake_reward(&creator, &holder, &stake_id);
    assert_eq!(claim.reward, pool_after_buy);
    assert_eq!(claim.amount, 1);

    // Sole staker gets the entire pool; pool and total-staked bookkeeping are cleared.
    assert_eq!(client.get_staking_rewards_pool(&creator), 0);
    assert_eq!(client.get_total_staked(&creator), 0);
    assert_eq!(client.get_staked_balance(&creator, &holder), 0);
    assert_eq!(
        client.get_staking_position(&creator, &holder, &stake_id),
        None
    );

    // Liquid balance is untouched by staking/unstaking bookkeeping.
    assert_eq!(client.get_key_balance(&creator, &holder), 1);
}

#[test]
fn test_claim_stake_reward_splits_pool_pro_rata_across_stakers() {
    let env = test_env_with_auths();
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

    let stake_a = client.stake_keys_locked(&creator, &holder_a, &3u32, &STAKE_LOCK_LEDGERS);
    let stake_b = client.stake_keys_locked(&creator, &holder_b, &1u32, &STAKE_LOCK_LEDGERS);
    assert_eq!(client.get_total_staked(&creator), 4);

    let pool_total = client.get_staking_rewards_pool(&creator);
    let expected_a = pool_total * 3 / 4;
    let expected_b_before_a_claims = pool_total / 4;

    let position_a = client
        .get_staking_position(&creator, &holder_a, &stake_a)
        .unwrap();
    set_ledger_sequence(&env, position_a.unlock_ledger);

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
    let (client, contract_id) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    let pool = client.get_staking_rewards_pool(&creator);
    let stake_id = client.stake_keys_locked(&creator, &holder, &1u32, &STAKE_LOCK_LEDGERS);

    let position = client
        .get_staking_position(&creator, &holder, &stake_id)
        .unwrap();
    set_ledger_sequence(&env, position.unlock_ledger);
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
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    let stake_id = client.stake_keys_locked(&creator, &holder, &1u32, &STAKE_LOCK_LEDGERS);

    let position = client
        .get_staking_position(&creator, &holder, &stake_id)
        .unwrap();
    set_ledger_sequence(&env, position.unlock_ledger);

    client.pause(&admin);

    let result = client.try_claim_stake_reward(&creator, &holder, &stake_id);
    assert_eq!(result, Err(Ok(StakingError::ProtocolPaused)));
}
