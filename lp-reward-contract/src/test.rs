#![cfg(test)]

use crate::{
    LPAdded, LPRewardClaimed, LpRewardContract, LpRewardContractClient, LpRewardError,
    LP_ADDED_EVENT_NAME, LP_CLAIMED_EVENT_NAME,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol, TryFromVal, Val, Vec,
};

fn setup_env() -> (Env, LpRewardContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LpRewardContract, ());
    let client = LpRewardContractClient::new(&env, &contract_id);
    let key_id = Address::generate(&env);
    (env, client, key_id)
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
fn test_lp_share_computed_proportionally_from_total_liquidity_pool() {
    let (env, client, key_id) = setup_env();
    let provider1 = Address::generate(&env);
    let provider2 = Address::generate(&env);
    let provider3 = Address::generate(&env);

    // Initial pool liquidity is zero
    assert_eq!(client.get_total_liquidity(&key_id), 0);

    // Provider 1 adds 1,000 tokens (100% of pool)
    let lp1 = client.add_liquidity(&key_id, &provider1, &1_000);
    assert_eq!(lp1, 1);
    assert_eq!(client.get_total_liquidity(&key_id), 1_000);

    let pos1 = client.get_lp_position(&lp1);
    assert_eq!(pos1.contribution, 1_000);
    assert_eq!(pos1.share, 10_000); // 100% = 10,000 bps

    // Provider 2 adds 3,000 tokens (Total = 4,000 tokens)
    let lp2 = client.add_liquidity(&key_id, &provider2, &3_000);
    assert_eq!(lp2, 2);
    assert_eq!(client.get_total_liquidity(&key_id), 4_000);

    // Dynamic proportional shares
    let pos1_updated = client.get_lp_position(&lp1);
    let pos2 = client.get_lp_position(&lp2);
    assert_eq!(pos1_updated.share, 2_500); // 1,000 / 4,000 = 25% = 2,500 bps
    assert_eq!(pos2.share, 7_500); // 3,000 / 4,000 = 75% = 7,500 bps

    // Provider 3 adds 6,000 tokens (Total = 10,000 tokens)
    let lp3 = client.add_liquidity_for(&provider3, &key_id, &6_000);
    assert_eq!(lp3, 3);
    assert_eq!(client.get_total_liquidity(&key_id), 10_000);

    assert_eq!(client.get_lp_position(&lp1).share, 1_000); // 10%
    assert_eq!(client.get_lp_position(&lp2).share, 3_000); // 30%
    assert_eq!(client.get_lp_position(&lp3).share, 6_000); // 60%
}

#[test]
fn test_lp_added_event_emitted() {
    let (env, client, key_id) = setup_env();
    let provider = Address::generate(&env);

    let lp_id = client.add_liquidity(&key_id, &provider, &5_000);
    let events: Vec<LPAdded> = get_events(&env, LP_ADDED_EVENT_NAME);

    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.lp_id, lp_id);
    assert_eq!(event.key_id, key_id);
    assert_eq!(event.provider, provider);
    assert_eq!(event.amount, 5_000);
    assert_eq!(event.share, 10_000);
}

#[test]
fn test_rewards_accrue_correctly_proportional_to_share_and_trading_volume() {
    let (env, client, key_id) = setup_env();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // Alice provides 200, Bob provides 600 (Total = 800: Alice 25%, Bob 75%)
    let lp_alice = client.add_liquidity(&key_id, &alice, &200);
    let lp_bob = client.add_liquidity(&key_id, &bob, &600);

    // Trading volume fee 1: 400 tokens
    client.accrue_trading_fee(&key_id, &400);

    // Alice should accrue 25% of 400 = 100
    // Bob should accrue 75% of 400 = 300
    let pos_alice = client.get_lp_position(&lp_alice);
    let pos_bob = client.get_lp_position(&lp_bob);
    assert_eq!(pos_alice.pending_rewards, 100);
    assert_eq!(pos_bob.pending_rewards, 300);

    // Additional trading volume fee 2: 800 tokens (Total fees = 1,200)
    client.accrue_trading_fee(&key_id, &800);

    // Alice total pending: 25% of 1,200 = 300
    // Bob total pending: 75% of 1,200 = 900
    assert_eq!(client.get_lp_position(&lp_alice).pending_rewards, 300);
    assert_eq!(client.get_lp_position(&lp_bob).pending_rewards, 900);
}

#[test]
fn test_claim_lp_rewards_transfers_correct_pending_amount() {
    let (env, client, key_id) = setup_env();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let lp_alice = client.add_liquidity(&key_id, &alice, &500);
    let lp_bob = client.add_liquidity(&key_id, &bob, &500);

    client.accrue_trading_fee(&key_id, &200);

    // Alice claims rewards
    let claimed_alice = client.claim_lp_rewards(&lp_alice);
    assert_eq!(claimed_alice, 100);

    // Verify LPRewardClaimed event emitted immediately after claim
    let claim_events: Vec<LPRewardClaimed> = get_events(&env, LP_CLAIMED_EVENT_NAME);
    assert_eq!(claim_events.len(), 1);
    let ev = claim_events.get(0).unwrap();
    assert_eq!(ev.lp_id, lp_alice);
    assert_eq!(ev.amount, 100);
    assert_eq!(ev.provider, alice);

    // Alice's pending rewards are reset to 0, contribution still intact
    let pos_alice = client.get_lp_position(&lp_alice);
    assert_eq!(pos_alice.pending_rewards, 0);
    assert_eq!(pos_alice.contribution, 500);

    // Bob has not claimed, still has 100 pending
    let pos_bob = client.get_lp_position(&lp_bob);
    assert_eq!(pos_bob.pending_rewards, 100);

    // Calling claim again when zero pending returns 0
    let claimed_again = client.claim_lp_rewards(&lp_alice);
    assert_eq!(claimed_again, 0);

    // New fee accrues: both get 50 each
    client.accrue_trading_fee(&key_id, &100);
    assert_eq!(client.get_lp_position(&lp_alice).pending_rewards, 50);
    assert_eq!(client.get_lp_position(&lp_bob).pending_rewards, 150);
}

#[test]
fn test_remove_liquidity_returns_correct_principal_plus_rewards() {
    let (env, client, key_id) = setup_env();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let lp_alice = client.add_liquidity(&key_id, &alice, &1_000);
    let lp_bob = client.add_liquidity(&key_id, &bob, &3_000);

    client.accrue_trading_fee(&key_id, &800);

    // Alice accrued 25% of 800 = 200 rewards.
    // Principal = 1,000.
    // Total returned should be 1,000 + 200 = 1,200.
    let returned = client.remove_liquidity(&lp_alice);
    assert_eq!(returned, 1_200);

    // Total liquidity in pool decreases by 1,000 to 3,000
    assert_eq!(client.get_total_liquidity(&key_id), 3_000);

    // Alice's position is closed
    let pos_alice = client.get_lp_position(&lp_alice);
    assert_eq!(pos_alice.contribution, 0);
    assert_eq!(pos_alice.share, 0);
    assert_eq!(pos_alice.pending_rewards, 0);

    // Removing again fails with PositionAlreadyClosed
    let err = client.try_remove_liquidity(&lp_alice);
    assert_eq!(err, Err(Ok(LpRewardError::PositionAlreadyClosed)));

    // Bob now owns 100% of remaining pool
    let pos_bob = client.get_lp_position(&lp_bob);
    assert_eq!(pos_bob.share, 10_000);
    assert_eq!(pos_bob.pending_rewards, 600);

    // Bob removes liquidity: 3,000 + 600 = 3,600
    let bob_returned = client.remove_liquidity(&lp_bob);
    assert_eq!(bob_returned, 3_600);
    assert_eq!(client.get_total_liquidity(&key_id), 0);
}

#[test]
fn test_get_lp_position_returns_accurate_values_at_any_point() {
    let (env, client, key_id) = setup_env();
    let provider = Address::generate(&env);

    let lp = client.add_liquidity(&key_id, &provider, &2_000);

    // Point 1: Immediately after add
    let p1 = client.get_lp_position(&lp);
    assert_eq!(p1.lp_id, lp);
    assert_eq!(p1.provider, provider);
    assert_eq!(p1.key_id, key_id);
    assert_eq!(p1.contribution, 2_000);
    assert_eq!(p1.share, 10_000);
    assert_eq!(p1.pending_rewards, 0);

    // Point 2: After trading fee accrues
    client.accrue_trading_fee(&key_id, &500);
    let p2 = client.get_lp_position(&lp);
    assert_eq!(p2.contribution, 2_000);
    assert_eq!(p2.pending_rewards, 500);

    // Point 3: After second provider joins
    let provider2 = Address::generate(&env);
    let _lp2 = client.add_liquidity(&key_id, &provider2, &2_000);
    let p3 = client.get_lp_position(&lp);
    assert_eq!(p3.share, 5_000); // Now 50%
    assert_eq!(p3.pending_rewards, 500); // Existing rewards preserved

    // Point 4: After claiming rewards
    client.claim_lp_rewards(&lp);
    let p4 = client.get_lp_position(&lp);
    assert_eq!(p4.contribution, 2_000);
    assert_eq!(p4.pending_rewards, 0);

    // Point 5: After removal
    client.remove_liquidity(&lp);
    let p5 = client.get_lp_position(&lp);
    assert_eq!(p5.contribution, 0);
    assert_eq!(p5.share, 0);
    assert_eq!(p5.pending_rewards, 0);
}

#[test]
fn test_validation_errors() {
    let (env, client, key_id) = setup_env();
    let provider = Address::generate(&env);

    // Zero / negative amount rejected
    let err_zero = client.try_add_liquidity(&key_id, &provider, &0);
    assert_eq!(err_zero, Err(Ok(LpRewardError::NotPositiveAmount)));

    let err_neg = client.try_add_liquidity(&key_id, &provider, &-50);
    assert_eq!(err_neg, Err(Ok(LpRewardError::NotPositiveAmount)));

    // Unknown position
    let err_nf = client.try_get_lp_position(&999);
    assert_eq!(err_nf, Err(Ok(LpRewardError::PositionNotFound)));

    let err_rem_nf = client.try_remove_liquidity(&999);
    assert_eq!(err_rem_nf, Err(Ok(LpRewardError::PositionNotFound)));

    let err_claim_nf = client.try_claim_lp_rewards(&999);
    assert_eq!(err_claim_nf, Err(Ok(LpRewardError::PositionNotFound)));

    // Accruing fee on empty pool
    let err_fee_empty = client.try_accrue_trading_fee(&key_id, &100);
    assert_eq!(err_fee_empty, Err(Ok(LpRewardError::ZeroLiquidityPool)));

    // Negative fee
    let err_fee_neg = client.try_accrue_trading_fee(&key_id, &-10);
    assert_eq!(err_fee_neg, Err(Ok(LpRewardError::NotPositiveAmount)));
}
