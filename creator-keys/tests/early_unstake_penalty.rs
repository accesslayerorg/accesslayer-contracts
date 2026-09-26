//! Tests for `early_unstake` and `set_early_exit_penalty` with forfeited penalty.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::events::{self, EarlyUnstakePenaltyEvent};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

#[test]
fn test_early_unstake_default_penalty_and_rewards_pool() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    // Buy 10 keys and stake all 10 keys.
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    assert_eq!(client.get_liquid_balance(&creator, &holder), 10);
    client.stake_keys(&creator, &holder, &10u32);
    assert_eq!(client.get_staked_balance(&creator, &holder), 10);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 0);

    // Early unstake: default penalty is 2000 bps (20%).
    // 10 * 20% = 2 penalty keys, 8 returned keys.
    let pool_before = client.get_staking_rewards_pool(&creator);
    client.early_unstake(&creator, &holder);
    let events = env.events().all();

    // Staked balance is now 0, liquid balance is 8 (staked 10 minus 2 penalty)
    assert_eq!(client.get_staked_balance(&creator, &holder), 0);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 8);

    // Penalty keys (2) added to the staking rewards pool
    assert_eq!(client.get_staking_rewards_pool(&creator), pool_before + 2);

    // Verify emitted early_unstake event
    let mut found_event = false;
    for (contract, topics, data) in events.iter() {
        if contract != contract_id {
            continue;
        }
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        if event_name == events::EARLY_UNSTAKE_PENALTY_EVENT_NAME {
            let payload: EarlyUnstakePenaltyEvent = data.clone().into_val(&env);
            assert_eq!(payload.wallet, holder);
            assert_eq!(payload.key_id, creator);
            assert_eq!(payload.returned_quantity, 8);
            assert_eq!(payload.penalty_quantity, 2);
            found_event = true;
        }
    }
    assert!(
        found_event,
        "expected EarlyUnstakePenaltyEvent to be emitted"
    );
}

#[test]
fn test_early_unstake_custom_penalty() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    // Set penalty to 4000 bps (40%)
    client.set_early_exit_penalty(&creator, &4000u32);
    assert_eq!(client.get_early_exit_penalty_bps(&creator), 4000);

    // Buy 10 keys and stake 10
    for _ in 0..10 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }
    client.stake_keys(&creator, &holder, &10u32);

    // Early unstake with 40% penalty -> 4 penalty, 6 returned
    let pool_before = client.get_staking_rewards_pool(&creator);
    client.early_unstake(&creator, &holder);

    assert_eq!(client.get_staked_balance(&creator, &holder), 0);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 6);
    assert_eq!(client.get_staking_rewards_pool(&creator), pool_before + 4);
}

#[test]
#[should_panic(expected = "PenaltyTooHigh")]
fn test_set_early_exit_penalty_above_5000_panics() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");

    // Setting penalty BPS above 5000 (e.g. 5001) must panic with PenaltyTooHigh
    client.set_early_exit_penalty(&creator, &5001u32);
}

#[test]
#[should_panic(expected = "NoStakeFound")]
fn test_early_unstake_no_active_stake_panics() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    // Holder has no active stake for creator key -> must panic with NoStakeFound
    client.early_unstake(&creator, &holder);
}
