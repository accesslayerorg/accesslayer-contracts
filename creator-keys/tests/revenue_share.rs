//! Integration tests for creator-funded key-holder revenue sharing (issue #1135).
//!
//! Tests coverage:
//! - Creator authorization enforcement
//! - Insufficient creator fee balance rejection
//! - Dust threshold rejection (`amount < supply`)
//! - No key holders rejection (`supply == 0`)
//! - Holder limit enforcement (`> MAX_SNAPSHOT_HOLDERS`)
//! - Filtering out zero-balance wallets from historical registry
//! - Pro-rata snapshot correctness across liquid and staked keys
//! - Isolation from post-snapshot trades / transfers
//! - Claiming credits claimant's pending dividend balance
//! - Double-claiming rejection (`AlreadyClaimed`)
//! - Nonexistent distribution ID rejection (`SnapshotNotFound`)
//! - `get_revenue_share` view returning accurate claimable/claimed amounts
//! - Remainder reconciliation (`total_allocated + remainder == requested_amount`)
//! - Emitted event topics and payloads for distribution and claim

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, setup_holders,
    test_env_with_auths, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use creator_keys::events::{
    revenue_distributed_topics, revenue_share_claimed_topics, RevenueDistributedEvent,
    RevenueShareClaimedEvent, REVENUE_DISTRIBUTED_EVENT_NAME, REVENUE_SHARE_CLAIMED_EVENT_NAME,
};
use creator_keys::{ContractError, CreatorKeysContractClient, RevenueShareView};
use soroban_sdk::testutils::{Address as _, Events, Ledger as _};
use soroban_sdk::{Address, Env, IntoVal, Symbol};

const KEY_PRICE: i128 = 1_000;

fn setup_test_context(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address, Address) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(
        env,
        &client,
        KEY_PRICE,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(env, &client, "alice");

    let holder_a = Address::generate(env);
    let holder_b = Address::generate(env);

    // Buy keys: holder_a gets 3 keys, holder_b gets 1 key
    let holders = [(holder_a.clone(), 3u32), (holder_b.clone(), 1u32)];
    setup_holders(env, &client, &creator, &holders);

    (client, creator, holder_a, holder_b)
}

#[test]
fn test_distribute_revenue_success_and_remainder_reconciliation() {
    let env = test_env_with_auths();
    let (client, creator, holder_a, holder_b) = setup_test_context(&env);

    // Initial creator fee balance after setup (4 buys)
    let fee_balance_before = client.get_creator_fee_balance(&creator);
    assert!(fee_balance_before > 0);

    // Distribute an amount with non-zero dust remainder
    // Supply = 4. Holder A = 3, Holder B = 1.
    // Choose amount = 1007:
    // Share A = (1007 * 3) / 4 = 3021 / 4 = 755
    // Share B = (1007 * 1) / 4 = 1007 / 4 = 251
    // Total allocated = 755 + 251 = 1006
    // Remainder = 1007 - 1006 = 1
    assert!(fee_balance_before >= 1007);

    let dist_id = client.distribute_revenue(&creator, &1007i128);
    let events = env.events().all();
    assert_eq!(dist_id, 1);

    // Only total_allocated (1006) must be debited from creator fee balance;
    // remainder (1) stays in the fee balance.
    let fee_balance_after = client.get_creator_fee_balance(&creator);
    assert_eq!(fee_balance_before - fee_balance_after, 1006);

    // Check get_revenue_share views before any claims
    let view_a = client.get_revenue_share(&dist_id, &holder_a);
    assert_eq!(
        view_a,
        RevenueShareView {
            claimable_amount: 755,
            claimed_amount: 0,
        }
    );

    let view_b = client.get_revenue_share(&dist_id, &holder_b);
    assert_eq!(
        view_b,
        RevenueShareView {
            claimable_amount: 251,
            claimed_amount: 0,
        }
    );

    // Verify event
    assert_eq!(
        revenue_distributed_topics(&creator, dist_id),
        (REVENUE_DISTRIBUTED_EVENT_NAME, creator.clone(), dist_id)
    );
    let (_, topics, data) = events
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == REVENUE_DISTRIBUTED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("RevenueDistributedEvent not found");

    let topic_key: Address = topics.get(1).unwrap().into_val(&env);
    let topic_dist_id: u32 = topics.get(2).unwrap().into_val(&env);
    assert_eq!(topic_key, creator);
    assert_eq!(topic_dist_id, dist_id);

    let event: RevenueDistributedEvent = data.into_val(&env);
    assert_eq!(event.key_id, creator);
    assert_eq!(event.distribution_id, dist_id);
    assert_eq!(event.amount, 1007);
    assert_eq!(event.allocated_amount, 1006);
    assert_eq!(event.remainder, 1);
    assert_eq!(event.total_eligible_keys, 4);
    assert_eq!(event.eligible_count, 2);
}

#[test]
fn test_distribute_insufficient_creator_fee_balance() {
    let env = test_env_with_auths();
    let (client, creator, _, _) = setup_test_context(&env);

    let fee_balance = client.get_creator_fee_balance(&creator);
    let excessive_amount = fee_balance + 100;

    let res = client.try_distribute_revenue(&creator, &excessive_amount);
    assert_eq!(res, Err(Ok(ContractError::InsufficientBalance)));
}

#[test]
fn test_distribute_dust_threshold_rejection() {
    let env = test_env_with_auths();
    let (client, creator, _, _) = setup_test_context(&env);

    // Supply is 4. Amount < supply must be rejected with ZeroDistributionAmount.
    let res = client.try_distribute_revenue(&creator, &3i128);
    assert_eq!(res, Err(Ok(ContractError::ZeroDistributionAmount)));
}

#[test]
fn test_distribute_no_key_holders() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "bob");

    // Creator has zero supply
    let res = client.try_distribute_revenue(&creator, &1000i128);
    assert_eq!(res, Err(Ok(ContractError::NoKeyHolders)));
}

#[test]
fn test_distribute_filters_zero_balance_wallets() {
    let env = test_env_with_auths();
    let (client, creator, _, _) = setup_test_context(&env);

    // Setup another holder who buys then sells all keys
    let seller = Address::generate(&env);
    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &seller, &quote.total_amount, &None);
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &seller, &None);

    // Verify seller is in holder registry but has 0 balance
    let registry = client.get_holder_registry(&creator);
    assert!(registry.contains(&seller));
    assert_eq!(client.get_key_balance(&creator, &seller), 0);

    // Supply is still 4 (holder_a = 3, holder_b = 1)
    let dist_id = client.distribute_revenue(&creator, &1000i128);
    assert_eq!(dist_id, 1);

    // Seller must have 0 claimable share
    let view_seller = client.get_revenue_share(&dist_id, &seller);
    assert_eq!(
        view_seller,
        RevenueShareView {
            claimable_amount: 0,
            claimed_amount: 0,
        }
    );

    // Seller cannot claim
    assert_eq!(
        client.try_claim_revenue_share(&dist_id, &seller),
        Err(Ok(ContractError::NoDividendClaimable))
    );
}

#[test]
fn test_snapshot_includes_staked_keys() {
    let env = test_env_with_auths();
    let (client, creator, holder_a, holder_b) = setup_test_context(&env);

    // Stake 2 out of 3 keys for holder_a
    client.stake_keys(&creator, &holder_a, &2u32);

    // Distribution should still account for all 3 keys of holder_a and 1 of holder_b (total = 4)
    let dist_id = client.distribute_revenue(&creator, &1000i128);

    let view_a = client.get_revenue_share(&dist_id, &holder_a);
    let view_b = client.get_revenue_share(&dist_id, &holder_b);

    assert_eq!(view_a.claimable_amount, 750);
    assert_eq!(view_b.claimable_amount, 250);
}

#[test]
fn test_post_snapshot_trades_do_not_affect_shares() {
    let env = test_env_with_auths();
    let (client, creator, _, holder_b) = setup_test_context(&env);

    let dist_id = client.distribute_revenue(&creator, &1000i128);

    // Holder B transfers or sells their key after the snapshot
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &holder_b, &None);
    assert_eq!(client.get_key_balance(&creator, &holder_b), 0);

    // Holder B's snapshotted share must remain unchanged
    let view_b = client.get_revenue_share(&dist_id, &holder_b);
    assert_eq!(view_b.claimable_amount, 250);

    // Holder B can still claim their share
    let claimed = client.claim_revenue_share(&dist_id, &holder_b);
    assert_eq!(claimed, 250);
}

#[test]
fn test_claim_credits_pending_balance_and_prevents_double_claim() {
    let env = test_env_with_auths();
    let (client, creator, holder_a, _) = setup_test_context(&env);

    let dist_id = client.distribute_revenue(&creator, &1000i128);

    // Pending dividend balance before claim
    let pending_before = client.get_claimable_dividend(&creator, &holder_a);

    // Claim share
    let claimed = client.claim_revenue_share(&dist_id, &holder_a);
    let events = env.events().all();
    assert_eq!(claimed, 750);

    // Pending dividend balance should increase by claimed amount
    let pending_after = client.get_claimable_dividend(&creator, &holder_a);
    assert_eq!(pending_after - pending_before, 750);

    // View should now reflect 0 claimable and 750 claimed
    let view_a = client.get_revenue_share(&dist_id, &holder_a);
    assert_eq!(
        view_a,
        RevenueShareView {
            claimable_amount: 0,
            claimed_amount: 750,
        }
    );

    // Second claim must fail with AlreadyClaimed
    let res = client.try_claim_revenue_share(&dist_id, &holder_a);
    assert_eq!(res, Err(Ok(ContractError::AlreadyClaimed)));

    // Verify claim event
    assert_eq!(
        revenue_share_claimed_topics(dist_id, &holder_a),
        (REVENUE_SHARE_CLAIMED_EVENT_NAME, dist_id, holder_a.clone())
    );
    let (_, topics, data) = events
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == REVENUE_SHARE_CLAIMED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("RevenueShareClaimedEvent not found");

    let topic_dist_id: u32 = topics.get(1).unwrap().into_val(&env);
    let topic_claimant: Address = topics.get(2).unwrap().into_val(&env);
    assert_eq!(topic_dist_id, dist_id);
    assert_eq!(topic_claimant, holder_a);

    let event: RevenueShareClaimedEvent = data.into_val(&env);
    assert_eq!(event.distribution_id, dist_id);
    assert_eq!(event.key_id, creator);
    assert_eq!(event.claimant, holder_a);
    assert_eq!(event.amount, 750);
}

#[test]
fn test_claim_nonexistent_distribution() {
    let env = test_env_with_auths();
    let (client, _, holder_a, _) = setup_test_context(&env);

    let res = client.try_claim_revenue_share(&999u32, &holder_a);
    assert_eq!(res, Err(Ok(ContractError::SnapshotNotFound)));
}

#[test]
fn test_distribute_revenue_unauthorized_caller_rejected() {
    let env = test_env_with_auths();
    let (client, creator, _, _) = setup_test_context(&env);

    // Disable mocked authorizations to verify creator.require_auth() is enforced
    env.mock_auths(&[]);

    let res = client.try_distribute_revenue(&creator, &1000i128);
    assert!(res.is_err());
}

#[test]
fn test_distribute_holder_limit_exceeded() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        KEY_PRICE,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "limitcreator");

    // MAX_SNAPSHOT_HOLDERS is 100. Create 101 unique holders, each holding 1 key.
    for i in 0..101 {
        let holder = contract_test_env::test_wallet_address_from_index(&env, i);
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &holder, &quote.total_amount, &None);
    }

    let fee_balance = client.get_creator_fee_balance(&creator);
    assert!(fee_balance >= 101);

    let res = client.try_distribute_revenue(&creator, &fee_balance);
    assert_eq!(res, Err(Ok(ContractError::SnapshotHolderLimitExceeded)));
}

#[test]
fn test_get_revenue_share_nonexistent_distribution_returns_zeros() {
    let env = test_env_with_auths();
    let (client, _, holder_a, _) = setup_test_context(&env);

    // Querying an unrecorded/nonexistent distribution_id
    let view = client.get_revenue_share(&9999u32, &holder_a);
    assert_eq!(
        view,
        RevenueShareView {
            claimable_amount: 0,
            claimed_amount: 0,
        }
    );
}

#[test]
fn test_get_revenue_share_unallocated_wallet_returns_zeros() {
    let env = test_env_with_auths();
    let (client, creator, _, _) = setup_test_context(&env);

    let dist_id = client.distribute_revenue(&creator, &1000i128);

    // Stranger wallet never held keys in this distribution
    let stranger = Address::generate(&env);
    let view = client.get_revenue_share(&dist_id, &stranger);
    assert_eq!(
        view,
        RevenueShareView {
            claimable_amount: 0,
            claimed_amount: 0,
        }
    );
}
