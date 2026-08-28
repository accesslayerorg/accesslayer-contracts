//! Acceptance tests for early unstaking and penalty accounting.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::{events, ContractError};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

fn setup(
    env: &Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    Address,
    Address,
) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, 100);
    let creator = register_test_creator(env, &client, "alice");
    let holder = Address::generate(env);
    (client, creator, holder)
}

#[test]
fn early_unstake_returns_remainder_and_credits_default_penalty() {
    let env = test_env_with_auths();
    let (client, creator, holder) = setup(&env);

    for _ in 0..10 {
        client.buy_key(&creator, &holder, &100, &None);
    }
    client.stake_keys(&creator, &holder, &10);

    assert_eq!(client.early_unstake(&creator, &holder), 8);
    let event = env
        .events()
        .all()
        .iter()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .map(|value| {
                    let name: Symbol = value.into_val(&env);
                    name == events::EARLY_UNSTAKE_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .unwrap();
    assert_eq!(client.get_key_balance(&creator, &holder), 8);
    assert_eq!(client.get_staked_balance(&creator, &holder), 0);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 8);
    assert_eq!(client.get_staking_rewards(&creator), 2);

    let payload: events::EarlyUnstakedEvent = event.2.into_val(&env);
    assert_eq!(payload.wallet, holder);
    assert_eq!(payload.key_id, creator);
    assert_eq!(payload.returned_quantity, 8);
    assert_eq!(payload.penalty_quantity, 2);
}

#[test]
fn custom_penalty_and_required_errors_work() {
    let env = test_env_with_auths();
    let (client, creator, holder) = setup(&env);

    assert_eq!(
        client.try_set_early_exit_penalty(&creator, &5001),
        Err(Ok(ContractError::PenaltyTooHigh))
    );
    client.set_early_exit_penalty(&creator, &5000);

    for _ in 0..10 {
        client.buy_key(&creator, &holder, &100, &None);
    }
    client.stake_keys(&creator, &holder, &10);
    assert_eq!(client.early_unstake(&creator, &holder), 5);
    assert_eq!(client.get_staking_rewards(&creator), 5);

    assert_eq!(
        client.try_early_unstake(&creator, &holder),
        Err(Ok(ContractError::NoStakeFound))
    );
}

#[test]
fn zero_penalty_returns_all_staked_keys() {
    let env = test_env_with_auths();
    let (client, creator, holder) = setup(&env);
    client.set_early_exit_penalty(&creator, &0);

    for _ in 0..4 {
        client.buy_key(&creator, &holder, &100, &None);
    }
    client.stake_keys(&creator, &holder, &4);

    assert_eq!(client.early_unstake(&creator, &holder), 4);
    assert_eq!(client.get_key_balance(&creator, &holder), 4);
    assert_eq!(client.get_staking_rewards(&creator), 0);
}
