//! Tests for `claim_stake_reward` and the staking rewards pool (#786 / #789).

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_ledger_sequence, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::{events, FeatureError, STAKE_LOCK_LEDGERS};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

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

    let result = client.try_claim_stake_reward(&creator, &holder);
    assert_eq!(result, Err(Ok(FeatureError::NoStakeFound)));
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
    client.stake_keys(&creator, &holder, &1u32);

    let result = client.try_claim_stake_reward(&creator, &holder);
    assert_eq!(result, Err(Ok(FeatureError::StakeLockActive)));

    // Still locked just one ledger before the unlock boundary.
    let unlock_ledger = client.get_stake_unlock_ledger(&creator, &holder).unwrap();
    set_ledger_sequence(&env, unlock_ledger - 1);
    let result = client.try_claim_stake_reward(&creator, &holder);
    assert_eq!(result, Err(Ok(FeatureError::StakeLockActive)));
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

    client.stake_keys(&creator, &holder, &1u32);
    assert_eq!(client.get_total_staked(&creator), 1);

    let start_sequence = env.ledger().sequence();
    set_ledger_sequence(&env, start_sequence + STAKE_LOCK_LEDGERS);

    let reward = client.claim_stake_reward(&creator, &holder);
    assert_eq!(reward, pool_after_buy);

    // Sole staker gets the entire pool; pool and total-staked bookkeeping are cleared.
    assert_eq!(client.get_staking_rewards_pool(&creator), 0);
    assert_eq!(client.get_total_staked(&creator), 0);
    assert_eq!(client.get_staked_balance(&creator, &holder), 0);
    assert_eq!(client.get_stake_unlock_ledger(&creator, &holder), None);

    // Liquid balance is untouched by staking/unstaking bookkeeping.
    assert_eq!(client.get_key_balance(&creator, &holder), 1);
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

    client.stake_keys(&creator, &holder_a, &3u32);
    client.stake_keys(&creator, &holder_b, &1u32);
    assert_eq!(client.get_total_staked(&creator), 4);

    let pool_total = client.get_staking_rewards_pool(&creator);
    let expected_a = pool_total * 3 / 4;
    let expected_b_before_a_claims = pool_total * 1 / 4;

    let start_sequence = env.ledger().sequence();
    set_ledger_sequence(&env, start_sequence + STAKE_LOCK_LEDGERS);

    let reward_a = client.claim_stake_reward(&creator, &holder_a);
    assert_eq!(reward_a, expected_a);
    assert_eq!(client.get_total_staked(&creator), 1);

    let reward_b = client.claim_stake_reward(&creator, &holder_b);
    // holder_b is the sole remaining staker and claims what's left in the pool,
    // which (absent further rounding loss) matches their pro-rata share.
    assert_eq!(reward_b, expected_b_before_a_claims);
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
    client.stake_keys(&creator, &holder, &1u32);

    let start_sequence = env.ledger().sequence();
    set_ledger_sequence(&env, start_sequence + STAKE_LOCK_LEDGERS);
    client.claim_stake_reward(&creator, &holder);

    let mut found = false;
    for (contract, topics, data) in env.events().all().iter() {
        if contract != contract_id {
            continue;
        }
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        if event_name == events::STAKE_REWARD_CLAIMED_EVENT_NAME {
            let payload: events::StakeRewardClaimedEvent = data.clone().into_val(&env);
            assert_eq!(payload.wallet, holder);
            assert_eq!(payload.key_id, creator);
            assert_eq!(payload.quantity_unlocked, 1);
            assert_eq!(payload.reward_amount, pool);
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
    client.stake_keys(&creator, &holder, &1u32);

    let start_sequence = env.ledger().sequence();
    set_ledger_sequence(&env, start_sequence + STAKE_LOCK_LEDGERS);

    client.pause(&admin);

    let result = client.try_claim_stake_reward(&creator, &holder);
    assert_eq!(result, Err(Ok(FeatureError::ProtocolPaused)));
}
