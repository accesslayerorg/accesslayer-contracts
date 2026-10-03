#![cfg(test)]

use crate::{
    lp_reward::{
        LPAdded, LPRewardClaimed, LpRewardError, LP_ADDED_EVENT_NAME, LP_CLAIMED_EVENT_NAME,
    },
    CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol, TryFromVal, Val, Vec,
};

fn setup_test() -> (Env, CreatorKeysContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let creator = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice_lp"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    (env, client, admin, creator)
}

fn get_events<T>(env: &Env, topic_symbol: Symbol) -> Vec<T>
where
    T: IntoVal<Env, Val> + TryFromVal<Env, Val>,
{
    let mut matching = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            if Symbol::try_from_val(env, &first) == Ok(topic_symbol.clone()) {
                if let Ok(event) = T::try_from_val(env, &data) {
                    matching.push_back(event);
                }
            }
        }
    }
    matching
}

#[test]
fn test_creator_keys_add_liquidity_and_proportional_share() {
    let (env, client, _admin, creator) = setup_test();
    let provider1 = Address::generate(&env);
    let provider2 = Address::generate(&env);

    let lp1 = client.add_liquidity(&creator, &provider1, &2_000);
    assert_eq!(lp1, 1);
    assert_eq!(client.get_lp_total_liquidity(&creator), 2_000);

    let p1 = client.get_lp_position(&lp1);
    assert_eq!(p1.contribution, 2_000);
    assert_eq!(p1.share, 10_000); // 100%

    // Second provider adds 6,000 (total = 8,000)
    let lp2 = client.add_liquidity_for(&provider2, &creator, &6_000);
    assert_eq!(lp2, 2);

    // Check LPAdded event immediately
    let add_events: Vec<LPAdded> = get_events(&env, LP_ADDED_EVENT_NAME);
    assert_eq!(add_events.len(), 1);
    let ev = add_events.get(0).unwrap();
    assert_eq!(ev.lp_id, lp2);
    assert_eq!(ev.key_id, creator);
    assert_eq!(ev.provider, provider2);
    assert_eq!(ev.amount, 6_000);

    assert_eq!(client.get_lp_total_liquidity(&creator), 8_000);

    // Shares: 25% and 75%
    assert_eq!(client.get_lp_position(&lp1).share, 2_500);
    assert_eq!(client.get_lp_position(&lp2).share, 7_500);
}

#[test]
fn test_creator_keys_accrue_trading_fees_and_claim_rewards() {
    let (env, client, _admin, creator) = setup_test();
    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);

    let lp1 = client.add_liquidity(&creator, &p1, &1_000);
    let lp2 = client.add_liquidity(&creator, &p2, &3_000);

    // Accrue 400 fee from trading volume
    client.accrue_lp_trading_fee(&creator, &400);

    // 25% to p1 (100), 75% to p2 (300)
    assert_eq!(client.get_lp_position(&lp1).pending_rewards, 100);
    assert_eq!(client.get_lp_position(&lp2).pending_rewards, 300);

    // P1 claims rewards
    let claimed = client.claim_lp_rewards(&lp1);
    assert_eq!(claimed, 100);

    // Verify LPRewardClaimed event emitted immediately after claim
    let claim_events: Vec<LPRewardClaimed> = get_events(&env, LP_CLAIMED_EVENT_NAME);
    assert_eq!(claim_events.len(), 1);
    let ev = claim_events.get(0).unwrap();
    assert_eq!(ev.lp_id, lp1);
    assert_eq!(ev.amount, 100);
    assert_eq!(ev.provider, p1);

    // After claim, pending is 0
    assert_eq!(client.get_lp_position(&lp1).pending_rewards, 0);
    assert_eq!(client.get_lp_position(&lp1).contribution, 1_000);
}

#[test]
fn test_creator_keys_remove_liquidity_returns_principal_plus_rewards() {
    let (env, client, _admin, creator) = setup_test();
    let provider = Address::generate(&env);

    let lp = client.add_liquidity(&creator, &provider, &10_000);
    client.accrue_lp_trading_fee(&creator, &1_500);

    // Total return = 10,000 + 1,500 = 11,500
    let total = client.remove_liquidity(&lp);
    assert_eq!(total, 11_500);
    assert_eq!(client.get_lp_total_liquidity(&creator), 0);

    let pos = client.get_lp_position(&lp);
    assert_eq!(pos.contribution, 0);
    assert_eq!(pos.share, 0);
    assert_eq!(pos.pending_rewards, 0);

    let re_remove = client.try_remove_liquidity(&lp);
    assert_eq!(re_remove, Err(Ok(LpRewardError::PositionAlreadyClosed)));
}

#[test]
fn test_creator_keys_validation_errors() {
    let (env, client, _admin, creator) = setup_test();
    let provider = Address::generate(&env);

    // Non-positive amount
    assert_eq!(
        client.try_add_liquidity(&creator, &provider, &0),
        Err(Ok(LpRewardError::NotPositiveAmount))
    );
    assert_eq!(
        client.try_add_liquidity(&creator, &provider, &-100),
        Err(Ok(LpRewardError::NotPositiveAmount))
    );

    // Unknown position
    assert_eq!(
        client.try_get_lp_position(&999),
        Err(Ok(LpRewardError::PositionNotFound))
    );
    assert_eq!(
        client.try_claim_lp_rewards(&999),
        Err(Ok(LpRewardError::PositionNotFound))
    );
    assert_eq!(
        client.try_remove_liquidity(&999),
        Err(Ok(LpRewardError::PositionNotFound))
    );
}
