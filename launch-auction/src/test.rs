#![cfg(test)]
extern crate std;

use crate::{
    AuctionCreatedEvent, AuctionError, AuctionSettledEvent, BidPlacedEvent, LaunchAuctionContract,
    LaunchAuctionContractClient, AUCTION_CREATED_EVENT_NAME, AUCTION_SETTLED_EVENT_NAME,
    BID_PLACED_EVENT_NAME,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, Symbol,
};

fn test_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

fn register_launch_auction<'a>(env: &'a Env) -> (LaunchAuctionContractClient<'a>, Address) {
    let id = env.register(LaunchAuctionContract, ());
    let client = LaunchAuctionContractClient::new(env, &id);
    (client, id)
}

fn auction_created_events(env: &Env) -> soroban_sdk::Vec<AuctionCreatedEvent> {
    let mut matching = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            let sym: Symbol = first.into_val(env);
            if sym == AUCTION_CREATED_EVENT_NAME {
                let event: AuctionCreatedEvent = data.into_val(env);
                matching.push_back(event);
            }
        }
    }
    matching
}

fn bid_placed_events(env: &Env) -> soroban_sdk::Vec<BidPlacedEvent> {
    let mut matching = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            let sym: Symbol = first.into_val(env);
            if sym == BID_PLACED_EVENT_NAME {
                let event: BidPlacedEvent = data.into_val(env);
                matching.push_back(event);
            }
        }
    }
    matching
}

fn auction_settled_events(env: &Env) -> soroban_sdk::Vec<AuctionSettledEvent> {
    let mut matching = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            let sym: Symbol = first.into_val(env);
            if sym == AUCTION_SETTLED_EVENT_NAME {
                let event: AuctionSettledEvent = data.into_val(env);
                matching.push_back(event);
            }
        }
    }
    matching
}

#[test]
fn test_create_auction_succeeds_and_emits_event() {
    let env = test_env();
    let (client, _) = register_launch_auction(&env);
    let creator = Address::generate(&env);

    let auction_id = client.create_auction(&creator, &10u32, &3600u64, &500i128);
    assert_eq!(auction_id, 1);

    // Assert event immediately: env.events() exposes the last invocation's events
    let events = auction_created_events(&env);
    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.auction_id, 1);
    assert_eq!(event.key_id, creator);
    assert_eq!(event.supply, 10);
    assert_eq!(event.min_bid, 500);
    assert_eq!(event.duration_seconds, 3600);

    // Check stored state
    let auction = client
        .get_auction(&auction_id)
        .expect("auction should exist");
    assert_eq!(auction.id, 1);
    assert_eq!(auction.key_id, creator);
    assert_eq!(auction.supply, 10);
    assert_eq!(auction.min_bid, 500);
    assert!(!auction.settled);
}

#[test]
fn test_create_auction_validates_inputs() {
    let env = test_env();
    let (client, _) = register_launch_auction(&env);
    let creator = Address::generate(&env);

    // Zero supply
    assert_eq!(
        client.try_create_auction(&creator, &0u32, &3600u64, &500i128),
        Err(Ok(AuctionError::InvalidSupply))
    );

    // Zero duration
    assert_eq!(
        client.try_create_auction(&creator, &10u32, &0u64, &500i128),
        Err(Ok(AuctionError::InvalidDuration))
    );

    // Non-positive min_bid
    assert_eq!(
        client.try_create_auction(&creator, &10u32, &3600u64, &0i128),
        Err(Ok(AuctionError::InvalidMinBid))
    );
    assert_eq!(
        client.try_create_auction(&creator, &10u32, &3600u64, &-10i128),
        Err(Ok(AuctionError::InvalidMinBid))
    );
}

#[test]
fn test_bids_below_min_bid_rejected_with_below_minimum() {
    let env = test_env();
    let (client, _) = register_launch_auction(&env);
    let creator = Address::generate(&env);
    let bidder = Address::generate(&env);

    let auction_id = client.create_auction(&creator, &5u32, &3600u64, &1000i128);

    // Bid below min_bid
    assert_eq!(
        client.try_place_bid(&bidder, &auction_id, &999i128),
        Err(Ok(AuctionError::BelowMinimum))
    );
    assert_eq!(
        client.try_place_bid(&bidder, &auction_id, &100i128),
        Err(Ok(AuctionError::BelowMinimum))
    );

    // Equal to min_bid succeeds
    let bid_id = client.place_bid(&bidder, &auction_id, &1000i128);
    assert_eq!(bid_id, 1);
}

#[test]
fn test_place_bid_locks_funds_in_escrow_and_emits_event() {
    let env = test_env();
    let (client, _) = register_launch_auction(&env);
    let creator = Address::generate(&env);
    let bidder = Address::generate(&env);

    let auction_id = client.create_auction(&creator, &5u32, &3600u64, &1000i128);

    assert_eq!(client.get_escrow_balance(&auction_id, &bidder), 0);

    let bid_id = client.place_bid(&bidder, &auction_id, &1500i128);
    assert_eq!(bid_id, 1);

    // Assert BidPlaced event immediately after place_bid invocation
    let events = bid_placed_events(&env);
    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.auction_id, auction_id);
    assert_eq!(event.bid_id, 1);
    assert_eq!(event.bidder, bidder);
    assert_eq!(event.amount, 1500);

    // Check updated escrow
    assert_eq!(client.get_escrow_balance(&auction_id, &bidder), 1500);
}

#[test]
fn test_settle_auction_allocates_from_highest_bid_downward_and_refunds_losing_bids() {
    let env = test_env();
    let (client, _) = register_launch_auction(&env);
    let creator = Address::generate(&env);

    let supply = 3u32;
    let auction_id = client.create_auction(&creator, &supply, &3600u64, &100i128);

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let b3 = Address::generate(&env);
    let b4 = Address::generate(&env);
    let b5 = Address::generate(&env);

    // Place 5 bids with varying amounts
    client.place_bid(&b1, &auction_id, &500i128);
    client.place_bid(&b2, &auction_id, &1000i128); // Highest
    client.place_bid(&b3, &auction_id, &750i128);
    client.place_bid(&b4, &auction_id, &250i128); // Losing
    client.place_bid(&b5, &auction_id, &150i128); // Losing

    // Advancing time past auction end
    env.ledger().set_timestamp(3601);

    // Settle auction
    let clearing_price = client.settle_auction(&auction_id);

    // Acceptance criteria 5: AuctionSettled event emitted with correct auction and bid data
    // Assert immediately after settle_auction before other contract calls
    let settled_events = auction_settled_events(&env);
    assert_eq!(settled_events.len(), 1);
    let event = settled_events.get(0).unwrap();
    assert_eq!(event.auction_id, auction_id);
    assert_eq!(event.key_id, creator);
    assert_eq!(event.clearing_price, 500);
    assert_eq!(event.supply, 3);
    assert_eq!(event.keys_allocated, 3);
    assert_eq!(event.total_raised, 1500); // 3 keys * 500 clearing price
    assert_eq!(event.winning_bids_count, 3);
    assert_eq!(event.refunded_bids_count, 4); // b4, b5 100% refunded + b2, b3 excess refunded

    // Top 3 bids are: b2 (1000), b3 (750), b1 (500).
    // The lowest winning bid is 500, which is the clearing price!
    assert_eq!(clearing_price, 500);

    // Acceptance criteria 2: allocates correctly from highest bid downward
    assert_eq!(client.get_allocation(&auction_id, &b2), 1);
    assert_eq!(client.get_allocation(&auction_id, &b3), 1);
    assert_eq!(client.get_allocation(&auction_id, &b1), 1);
    assert_eq!(client.get_allocation(&auction_id, &b4), 0);
    assert_eq!(client.get_allocation(&auction_id, &b5), 0);

    // Acceptance criteria 3: losing bids refunded automatically on settlement
    // Losing bidders' escrow is completely released/refunded (balance becomes 0)
    assert_eq!(client.get_escrow_balance(&auction_id, &b4), 0);
    assert_eq!(client.get_escrow_balance(&auction_id, &b5), 0);

    // Winning bidders excess refunded as well
    assert_eq!(client.get_escrow_balance(&auction_id, &b1), 0);
    assert_eq!(client.get_escrow_balance(&auction_id, &b2), 0);
    assert_eq!(client.get_escrow_balance(&auction_id, &b3), 0);

    // Acceptance criteria 4: bonding curve opens at clearing price after auction settles
    assert!(client.is_bonding_curve_open(&auction_id));
    assert_eq!(client.get_clearing_price(&auction_id), Some(500));
}

#[test]
fn test_cannot_settle_before_close_or_double_settle() {
    let env = test_env();
    let (client, _) = register_launch_auction(&env);
    let creator = Address::generate(&env);

    let auction_id = client.create_auction(&creator, &5u32, &3600u64, &100i128);

    // Early settlement attempt reverts with AuctionNotClosed
    assert_eq!(
        client.try_settle_auction(&auction_id),
        Err(Ok(AuctionError::AuctionNotClosed))
    );

    // Advance ledger timestamp to closed
    env.ledger().set_timestamp(3601);

    // First settlement succeeds
    let price = client.settle_auction(&auction_id);
    assert_eq!(price, 100);

    // Second settlement attempt reverts with AlreadySettled
    assert_eq!(
        client.try_settle_auction(&auction_id),
        Err(Ok(AuctionError::AlreadySettled))
    );
}

#[test]
fn test_cannot_bid_after_expiration() {
    let env = test_env();
    let (client, _) = register_launch_auction(&env);
    let creator = Address::generate(&env);
    let bidder = Address::generate(&env);

    let auction_id = client.create_auction(&creator, &5u32, &1000u64, &100i128);

    // Time passes past duration
    env.ledger().set_timestamp(1001);

    assert_eq!(
        client.try_place_bid(&bidder, &auction_id, &200i128),
        Err(Ok(AuctionError::AuctionExpired))
    );
}

#[test]
fn test_undersubscribed_auction_allocates_to_all_bidders() {
    let env = test_env();
    let (client, _) = register_launch_auction(&env);
    let creator = Address::generate(&env);

    // Supply is 10, but only 2 bids placed
    let auction_id = client.create_auction(&creator, &10u32, &3600u64, &200i128);

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    client.place_bid(&b1, &auction_id, &300i128);
    client.place_bid(&b2, &auction_id, &500i128);

    env.ledger().set_timestamp(3601);

    let clearing_price = client.settle_auction(&auction_id);

    // Lowest winning bid is 300
    assert_eq!(clearing_price, 300);
    assert_eq!(client.get_allocation(&auction_id, &b1), 1);
    assert_eq!(client.get_allocation(&auction_id, &b2), 1);

    let auction = client.get_auction(&auction_id).unwrap();
    assert_eq!(auction.keys_allocated, 2);
    assert_eq!(auction.total_raised, 600); // 2 keys * 300
    assert!(client.is_bonding_curve_open(&auction_id));
    assert_eq!(client.get_clearing_price(&auction_id), Some(300));
}

#[test]
fn test_zero_bids_auction_settles_at_min_bid_and_opens_curve() {
    let env = test_env();
    let (client, _) = register_launch_auction(&env);
    let creator = Address::generate(&env);

    let auction_id = client.create_auction(&creator, &5u32, &3600u64, &250i128);

    env.ledger().set_timestamp(3601);

    let clearing_price = client.settle_auction(&auction_id);
    assert_eq!(clearing_price, 250);

    let auction = client.get_auction(&auction_id).unwrap();
    assert!(auction.settled);
    assert_eq!(auction.keys_allocated, 0);
    assert_eq!(auction.total_raised, 0);
    assert!(client.is_bonding_curve_open(&auction_id));
    assert_eq!(client.get_clearing_price(&auction_id), Some(250));
}

#[test]
fn test_get_bids_returns_all_placed_bids() {
    let env = test_env();
    let (client, _) = register_launch_auction(&env);
    let creator = Address::generate(&env);

    let auction_id = client.create_auction(&creator, &5u32, &3600u64, &100i128);

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    client.place_bid(&b1, &auction_id, &200i128);
    client.place_bid(&b2, &auction_id, &300i128);

    let bids = client.get_bids(&auction_id);
    assert_eq!(bids.len(), 2);
    assert_eq!(bids.get(0).unwrap().bidder, b1);
    assert_eq!(bids.get(0).unwrap().amount, 200);
    assert_eq!(bids.get(1).unwrap().bidder, b2);
    assert_eq!(bids.get(1).unwrap().amount, 300);
}
