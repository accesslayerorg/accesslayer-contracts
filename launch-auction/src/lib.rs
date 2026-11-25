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

        if auction.settled {
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
        let new_escrow = current_escrow + amount;
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

    /// Settles the auction after duration ends:
    /// 1. Clears bids from highest to lowest downward until supply is exhausted.
    /// 2. Automatically refunds losing bids (and excess over clearing price).
    /// 3. Establishes the initial clearing price for price discovery.
    /// 4. Opens the bonding curve at the discovered clearing price.
    /// 5. Emits `AuctionSettled` with complete auction outcome data.
    pub fn settle_auction(env: Env, auction_id: u32) -> Result<i128, AuctionError> {
        let auction_key = DataKey::Auction(auction_id);
        let mut auction: Auction = env
            .storage()
            .persistent()
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
        let bids: Vec<Bid> = env
            .storage()
            .persistent()
            .get(&bids_key)
            .unwrap_or_else(|| Vec::new(&env));

        let total_bids_count = bids.len();

        if total_bids_count == 0 {
            // No bids placed: default clearing price is min_bid, 0 keys allocated
            let clearing_price = auction.min_bid;
            auction.settled = true;
            auction.clearing_price = clearing_price;
            auction.keys_allocated = 0;
            auction.total_raised = 0;

            env.storage().persistent().set(&auction_key, &auction);
            extend_ttl(&env, &auction_key);

            let curve_key = DataKey::BondingCurveOpen(auction_id);
            env.storage().persistent().set(&curve_key, &true);
            extend_ttl(&env, &curve_key);

            let price_key = DataKey::ClearingPrice(auction_id);
            env.storage().persistent().set(&price_key, &clearing_price);
            extend_ttl(&env, &price_key);

            env.events().publish(
                (AUCTION_SETTLED_EVENT_NAME, auction.key_id.clone()),
                AuctionSettledEvent {
                    auction_id,
                    key_id: auction.key_id,
                    clearing_price,
                    supply: auction.supply,
                    keys_allocated: 0,
                    total_raised: 0,
                    winning_bids_count: 0,
                    refunded_bids_count: 0,
                    settled_at: now,
                },
            );

            return Ok(clearing_price);
        }

        // Sort bids descending by amount (stable insertion sort for Soroban Vec)
        let mut sorted_bids: Vec<Bid> = Vec::new(&env);
        for bid in bids.iter() {
            let mut inserted = false;
            let mut temp: Vec<Bid> = Vec::new(&env);
            for existing in sorted_bids.iter() {
                if !inserted && bid.amount > existing.amount {
                    temp.push_back(bid.clone());
                    inserted = true;
                }
                temp.push_back(existing);
            }
            if !inserted {
                temp.push_back(bid);
            }
            sorted_bids = temp;
        }

        let supply = auction.supply;
        let winning_count = core::cmp::min(supply, sorted_bids.len());
        let clearing_price = sorted_bids.get(winning_count - 1).unwrap().amount;

        let mut total_raised: i128 = 0;
        let mut refunded_count: u32 = 0;

        for i in 0..sorted_bids.len() {
            let bid = sorted_bids.get(i).unwrap();
            let escrow_key = DataKey::Escrow(auction_id, bid.bidder.clone());

            if i < winning_count {
                // Winning bid: allocate 1 key
                let alloc_key = DataKey::Allocation(auction_id, bid.bidder.clone());
                let prior_alloc = env
                    .storage()
                    .persistent()
                    .get::<DataKey, u32>(&alloc_key)
                    .unwrap_or(0);
                env.storage()
                    .persistent()
                    .set(&alloc_key, &(prior_alloc + 1));
                extend_ttl(&env, &alloc_key);

                total_raised += clearing_price;

                // Refund excess payment over uniform clearing price, if any
                let excess = bid.amount - clearing_price;
                let current_escrow = env
                    .storage()
                    .persistent()
                    .get::<DataKey, i128>(&escrow_key)
                    .unwrap_or(0);
                let remaining_escrow = if current_escrow >= bid.amount {
                    current_escrow - bid.amount
                } else {
                    0
                };
                env.storage()
                    .persistent()
                    .set(&escrow_key, &remaining_escrow);
                extend_ttl(&env, &escrow_key);

                if excess > 0 {
                    refunded_count += 1;
                }
            } else {
                // Losing bid: 100% refunded automatically on settlement
                let current_escrow = env
                    .storage()
                    .persistent()
                    .get::<DataKey, i128>(&escrow_key)
                    .unwrap_or(0);
                let remaining_escrow = if current_escrow >= bid.amount {
                    current_escrow - bid.amount
                } else {
                    0
                };
                env.storage()
                    .persistent()
                    .set(&escrow_key, &remaining_escrow);
                extend_ttl(&env, &escrow_key);

                refunded_count += 1;
            }
        }

        // Open bonding curve at the clearing price
        auction.settled = true;
        auction.clearing_price = clearing_price;
        auction.keys_allocated = winning_count;
        auction.total_raised = total_raised;

        env.storage().persistent().set(&auction_key, &auction);
        extend_ttl(&env, &auction_key);

        let curve_key = DataKey::BondingCurveOpen(auction_id);
        env.storage().persistent().set(&curve_key, &true);
        extend_ttl(&env, &curve_key);

        let price_key = DataKey::ClearingPrice(auction_id);
        env.storage().persistent().set(&price_key, &clearing_price);
        extend_ttl(&env, &price_key);

        env.events().publish(
            (AUCTION_SETTLED_EVENT_NAME, auction.key_id.clone()),
            AuctionSettledEvent {
                auction_id,
                key_id: auction.key_id,
                clearing_price,
                supply: auction.supply,
                keys_allocated: winning_count,
                total_raised,
                winning_bids_count: winning_count,
                refunded_bids_count: refunded_count,
                settled_at: now,
            },
        );

        Ok(clearing_price)
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
        env.storage()
            .persistent()
            .get(&DataKey::Allocation(auction_id, bidder))
            .unwrap_or(0)
    }

    /// Returns the remaining locked escrow balance for a bidder in an auction.
    pub fn get_escrow_balance(env: Env, auction_id: u32, bidder: Address) -> i128 {
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
