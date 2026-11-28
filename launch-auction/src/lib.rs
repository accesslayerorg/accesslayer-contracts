#![no_std]
#[cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

#[cfg(test)]
mod test;

/// Event name for auction creation.
pub const AUCTION_CREATED_EVENT_NAME: Symbol = symbol_short!("auc_creat");

/// Event name for bid placement.
pub const BID_PLACED_EVENT_NAME: Symbol = symbol_short!("bid_place");

/// Event name for auction settlement.
pub const AUCTION_SETTLED_EVENT_NAME: Symbol = symbol_short!("auc_settl");

/// Default persistent storage TTL thresholds (~30 days).
pub const AUCTION_TTL_THRESHOLD: u32 = 100_000;
pub const AUCTION_TTL_EXTEND_TO: u32 = 518_400;
pub const MAX_AUCTION_BIDS: u32 = 32;
pub const SETTLEMENT_PAGE_SIZE: u32 = 8;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AuctionError {
    /// Bid amount is below the auction's minimum required bid.
    BelowMinimum = 1,
    /// Specified auction was not found.
    AuctionNotFound = 2,
    /// Auction has closed and no longer accepts bids.
    AuctionExpired = 3,
    /// Auction duration has not ended; cannot settle before close.
    AuctionNotClosed = 4,
    /// Auction has already been settled.
    AlreadySettled = 5,
    /// Caller is not authorized to perform this operation.
    Unauthorized = 6,
    /// Supply must be strictly greater than zero.
    InvalidSupply = 7,
    /// Auction duration must be strictly greater than zero.
    InvalidDuration = 8,
    /// Minimum bid must be strictly greater than zero.
    InvalidMinBid = 9,
    TooManyItems = 10,
    NoRefund = 11,
    Overflow = 12,
    InvalidCursor = 13,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Total count of auctions created.
    AuctionCount,
    /// Auction metadata record by auction ID.
    Auction(u32),
    /// List of bids submitted for an auction ID.
    AuctionBids(u32),
    /// Escrow balance locked per bidder for an auction ID.
    Escrow(u32, Address),
    /// Allocated key count won by a bidder.
    Allocation(u32, Address),
    /// Market clearing price established upon settlement.
    ClearingPrice(u32),
    /// Flag indicating whether the bonding curve has been opened.
    BondingCurveOpen(u32),
    Settlement(u32),
    SortedBids(u32),
}

/// Progress through sorting and allocation. The cursor counts both stages.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementPage {
    pub cursor: u32,
    pub clearing_price: i128,
    pub refunded_bids: u32,
    pub complete: bool,
}

/// Metadata and state for a key launch auction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Auction {
    pub id: u32,
    pub key_id: Address,
    pub creator: Address,
    pub supply: u32,
    pub start_time: u64,
    pub end_time: u64,
    pub min_bid: i128,
    pub settled: bool,
    pub clearing_price: i128,
    pub keys_allocated: u32,
    pub total_raised: i128,
}

/// Individual sealed bid placed by a participant.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bid {
    pub id: u32,
    pub auction_id: u32,
    pub bidder: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Event payload for `AuctionCreated`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionCreatedEvent {
    pub auction_id: u32,
    pub key_id: Address,
    pub creator: Address,
    pub supply: u32,
    pub duration_seconds: u64,
    pub min_bid: i128,
    pub start_time: u64,
    pub end_time: u64,
}

/// Event payload for `BidPlaced`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BidPlacedEvent {
    pub auction_id: u32,
    pub bid_id: u32,
    pub bidder: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Event payload for `AuctionSettled`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionSettledEvent {
    pub auction_id: u32,
    pub key_id: Address,
    pub clearing_price: i128,
    pub supply: u32,
    pub keys_allocated: u32,
    pub total_raised: i128,
    pub winning_bids_count: u32,
    pub refunded_bids_count: u32,
    pub settled_at: u64,
}

#[contract]
pub struct LaunchAuctionContract;

fn extend_ttl(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, AUCTION_TTL_THRESHOLD, AUCTION_TTL_EXTEND_TO);
}

#[contractimpl]
impl LaunchAuctionContract {
    /// Creates a sealed-bid launch auction for a creator key's initial allocation.
    ///
    /// Creator-only at launch: requires authentication from `key_id`.
    pub fn create_auction(
        env: Env,
        key_id: Address,
        supply: u32,
        duration_seconds: u64,
        min_bid: i128,
    ) -> Result<u32, AuctionError> {
        key_id.require_auth();

        if supply == 0 {
            return Err(AuctionError::InvalidSupply);
        }
        if duration_seconds == 0 {
            return Err(AuctionError::InvalidDuration);
        }
        if min_bid <= 0 {
            return Err(AuctionError::InvalidMinBid);
        }

        let count_key = DataKey::AuctionCount;
        let next_id = env
            .storage()
            .persistent()
            .get::<DataKey, u32>(&count_key)
            .unwrap_or(0)
            + 1;
        env.storage().persistent().set(&count_key, &next_id);
        extend_ttl(&env, &count_key);

        let start_time = env.ledger().timestamp();
        let end_time = start_time + duration_seconds;

        let auction = Auction {
            id: next_id,
            key_id: key_id.clone(),
            creator: key_id.clone(),
            supply,
            start_time,
            end_time,
            min_bid,
            settled: false,
            clearing_price: 0,
            keys_allocated: 0,
            total_raised: 0,
        };

        let auction_key = DataKey::Auction(next_id);
        env.storage().persistent().set(&auction_key, &auction);
        extend_ttl(&env, &auction_key);

        let bids_key = DataKey::AuctionBids(next_id);
        let empty_bids: Vec<Bid> = Vec::new(&env);
        env.storage().persistent().set(&bids_key, &empty_bids);
        extend_ttl(&env, &bids_key);

        env.events().publish(
            (AUCTION_CREATED_EVENT_NAME, key_id.clone()),
            AuctionCreatedEvent {
                auction_id: next_id,
                key_id,
                creator: auction.creator,
                supply,
                duration_seconds,
                min_bid,
                start_time,
                end_time,
            },
        );

        Ok(next_id)
    }

    /// Places a sealed bid in an open launch auction, locking funds in contract escrow.
    ///
    /// Rejects bids below `min_bid` with [`AuctionError::BelowMinimum`].
    pub fn place_bid(
        env: Env,
        bidder: Address,
        auction_id: u32,
        amount: i128,
    ) -> Result<u32, AuctionError> {
        bidder.require_auth();

        let auction_key = DataKey::Auction(auction_id);
        let auction: Auction = env
            .storage()
            .persistent()
            .get(&auction_key)
            .ok_or(AuctionError::AuctionNotFound)?;

        if auction.settled
            || env
                .storage()
                .persistent()
                .has(&DataKey::Settlement(auction_id))
        {
            return Err(AuctionError::AlreadySettled);
        }

        let now = env.ledger().timestamp();
        if now > auction.end_time {
            return Err(AuctionError::AuctionExpired);
        }

        if amount < auction.min_bid {
            return Err(AuctionError::BelowMinimum);
        }

        let bids_key = DataKey::AuctionBids(auction_id);
        let mut bids: Vec<Bid> = env
            .storage()
            .persistent()
            .get(&bids_key)
            .unwrap_or_else(|| Vec::new(&env));

        if bids.len() >= MAX_AUCTION_BIDS {
            return Err(AuctionError::TooManyItems);
        }
        let bid_id = bids.len() + 1;
        let bid = Bid {
            id: bid_id,
            auction_id,
            bidder: bidder.clone(),
            amount,
            timestamp: now,
        };

        bids.push_back(bid);
        env.storage().persistent().set(&bids_key, &bids);
        extend_ttl(&env, &bids_key);

        // Record escrow balance locked for this bidder
        let escrow_key = DataKey::Escrow(auction_id, bidder.clone());
        let current_escrow = env
            .storage()
            .persistent()
            .get::<DataKey, i128>(&escrow_key)
            .unwrap_or(0);
        let new_escrow = current_escrow
            .checked_add(amount)
            .ok_or(AuctionError::Overflow)?;
        env.storage().persistent().set(&escrow_key, &new_escrow);
        extend_ttl(&env, &escrow_key);

        env.events().publish(
            (BID_PLACED_EVENT_NAME, bidder.clone()),
            BidPlacedEvent {
                auction_id,
                bid_id,
                bidder,
                amount,
                timestamp: now,
            },
        );

        Ok(bid_id)
    }

    /// Settles an auction containing up to MAX_AUCTION_BIDS bids in one call.
    pub fn settle_auction(env: Env, auction_id: u32) -> Result<i128, AuctionError> {
        Ok(settle_page(&env, auction_id, 0, MAX_AUCTION_BIDS, true)?.clearing_price)
    }

    /// Starts at zero, then uses the cursor returned by each successful page.
    pub fn settle_auction_page(
        env: Env,
        auction_id: u32,
        cursor: u32,
    ) -> Result<SettlementPage, AuctionError> {
        settle_page(&env, auction_id, cursor, SETTLEMENT_PAGE_SIZE, false)
    }

    /// Claims losing bids and excess winning payments from the internal escrow ledger.
    pub fn claim_refund(env: Env, auction_id: u32, bidder: Address) -> Result<i128, AuctionError> {
        bidder.require_auth();
        let auction =
            Self::get_auction(env.clone(), auction_id).ok_or(AuctionError::AuctionNotFound)?;
        if !auction.settled {
            return Err(AuctionError::AuctionNotClosed);
        }
        let paid = auction
            .clearing_price
            .checked_mul(i128::from(Self::get_allocation(
                env.clone(),
                auction_id,
                bidder.clone(),
            )))
            .ok_or(AuctionError::Overflow)?;
        let key = DataKey::Escrow(auction_id, bidder.clone());
        let escrow = env.storage().persistent().get(&key).unwrap_or(0i128);
        let refund = escrow.checked_sub(paid).ok_or(AuctionError::Overflow)?;
        if refund <= 0 {
            return Err(AuctionError::NoRefund);
        }
        env.storage().persistent().remove(&key);
        env.events()
            .publish((symbol_short!("auc_refnd"), auction_id, bidder), refund);
        Ok(refund)
    }

    /// Returns auction details by ID.
    pub fn get_auction(env: Env, auction_id: u32) -> Option<Auction> {
        env.storage()
            .persistent()
            .get(&DataKey::Auction(auction_id))
    }

    /// Returns all bids submitted for an auction.
    pub fn get_bids(env: Env, auction_id: u32) -> Vec<Bid> {
        env.storage()
            .persistent()
            .get(&DataKey::AuctionBids(auction_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns the number of keys allocated to a bidder for a settled auction.
    pub fn get_allocation(env: Env, auction_id: u32, bidder: Address) -> u32 {
        if Self::get_auction(env.clone(), auction_id).is_some_and(|auction| !auction.settled) {
            return 0;
        }
        env.storage()
            .persistent()
            .get(&DataKey::Allocation(auction_id, bidder))
            .unwrap_or(0)
    }

    /// Returns the remaining locked escrow balance for a bidder in an auction.
    pub fn get_escrow_balance(env: Env, auction_id: u32, bidder: Address) -> i128 {
        if Self::get_auction(env.clone(), auction_id).is_some_and(|auction| auction.settled) {
            return 0;
        }
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(auction_id, bidder))
            .unwrap_or(0)
    }

    /// Returns whether the bonding curve has opened for an auction's key.
    pub fn is_bonding_curve_open(env: Env, auction_id: u32) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::BondingCurveOpen(auction_id))
            .unwrap_or(false)
    }

    /// Returns the initial clearing price discovered for an auction.
    pub fn get_clearing_price(env: Env, auction_id: u32) -> Option<i128> {
        env.storage()
            .persistent()
            .get(&DataKey::ClearingPrice(auction_id))
    }
}

fn settle_page(
    env: &Env,
    auction_id: u32,
    cursor: u32,
    limit: u32,
    atomic: bool,
) -> Result<SettlementPage, AuctionError> {
    let storage = env.storage().persistent();
    let auction_key = DataKey::Auction(auction_id);
    let mut auction: Auction = storage
        .get(&auction_key)
        .ok_or(AuctionError::AuctionNotFound)?;
    if auction.settled {
        return Err(AuctionError::AlreadySettled);
    }
    let now = env.ledger().timestamp();
    if now < auction.end_time {
        return Err(AuctionError::AuctionNotClosed);
    }

    let bids_key = DataKey::AuctionBids(auction_id);
    let bids: Vec<Bid> = storage.get(&bids_key).unwrap_or_else(|| Vec::new(env));
    let count = bids.len();
    if atomic && count > limit {
        return Err(AuctionError::TooManyItems);
    }
    let final_cursor = count.checked_mul(2).ok_or(AuctionError::Overflow)?;
    let progress_key = DataKey::Settlement(auction_id);
    let sorted_key = DataKey::SortedBids(auction_id);
    let mut progress = storage
        .get::<_, SettlementPage>(&progress_key)
        .unwrap_or(SettlementPage {
            cursor: 0,
            clearing_price: auction.min_bid,
            refunded_bids: 0,
            complete: false,
        });
    if cursor != progress.cursor {
        return Err(AuctionError::InvalidCursor);
    }
    let mut sorted: Vec<Bid> = storage.get(&sorted_key).unwrap_or_else(|| Vec::new(env));
    let end = if atomic {
        final_cursor
    } else {
        cursor.saturating_add(limit).min(final_cursor)
    };
    for index in cursor..end.min(count) {
        let bid = bids.get(index).ok_or(AuctionError::InvalidCursor)?;
        insert_ranked_bid(&mut sorted, bid);
    }

    let winning_count = auction.supply.min(count);
    if end >= count && winning_count > 0 {
        progress.clearing_price = sorted
            .get(winning_count - 1)
            .ok_or(AuctionError::InvalidCursor)?
            .amount;
    }
    for index in cursor.max(count)..end {
        let rank = index - count;
        let bid = sorted.get(rank).ok_or(AuctionError::InvalidCursor)?;
        if rank < winning_count {
            let key = DataKey::Allocation(auction_id, bid.bidder);
            let allocated = storage.get(&key).unwrap_or(0u32);
            storage.set(&key, &(allocated + 1));
            extend_ttl(env, &key);
        }
        if rank >= winning_count || bid.amount > progress.clearing_price {
            progress.refunded_bids += 1;
        }
    }

    progress.cursor = end;
    progress.complete = end == final_cursor;
    if !progress.complete {
        storage.set(&progress_key, &progress);
        storage.set(&sorted_key, &sorted);
        extend_ttl(env, &progress_key);
        extend_ttl(env, &sorted_key);
        extend_ttl(env, &auction_key);
        extend_ttl(env, &bids_key);
        return Ok(progress);
    }

    auction.settled = true;
    auction.clearing_price = progress.clearing_price;
    auction.keys_allocated = winning_count;
    auction.total_raised = progress
        .clearing_price
        .checked_mul(i128::from(winning_count))
        .ok_or(AuctionError::Overflow)?;
    storage.set(&auction_key, &auction);
    extend_ttl(env, &auction_key);
    let curve_key = DataKey::BondingCurveOpen(auction_id);
    storage.set(&curve_key, &true);
    extend_ttl(env, &curve_key);
    let price_key = DataKey::ClearingPrice(auction_id);
    storage.set(&price_key, &progress.clearing_price);
    extend_ttl(env, &price_key);
    storage.remove(&progress_key);
    storage.remove(&sorted_key);

    env.events().publish(
        (AUCTION_SETTLED_EVENT_NAME, auction.key_id.clone()),
        AuctionSettledEvent {
            auction_id,
            key_id: auction.key_id,
            clearing_price: auction.clearing_price,
            supply: auction.supply,
            keys_allocated: winning_count,
            total_raised: auction.total_raised,
            winning_bids_count: winning_count,
            refunded_bids_count: progress.refunded_bids,
            settled_at: now,
        },
    );
    Ok(progress)
}

fn insert_ranked_bid(sorted: &mut Vec<Bid>, bid: Bid) {
    let mut low = 0;
    let mut high = sorted.len();
    // Insert after equal bids so that placement order breaks ties.
    while low < high {
        let mid = low + (high - low) / 2;
        if sorted.get(mid).unwrap().amount >= bid.amount {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    sorted.insert(low, bid);
}
