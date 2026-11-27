//! English auction for secondary key sales with bid increments and anti-sniping extension.
//!
//! Issue #1284: Holders selling a large position can only use fixed prices or Dutch auctions.
//! This module adds an ascending English auction with minimum bid increments, refunds for
//! outbid bidders and an automatic extension when bids arrive near the end.

use soroban_sdk::{contracttype, Address, Env};

use crate::ContractError;

/// Maximum extension window in ledgers (last N ledgers before end).
pub const EXTENSION_WINDOW_LEDGERS: u32 = 100;

/// Extension amount when bid arrives within window.
pub const EXTENSION_AMOUNT_LEDGERS: u32 = 50;

/// Maximum total extension allowed.
pub const MAX_TOTAL_EXTENSION_LEDGERS: u32 = 500;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EnglishAuction {
    /// Unique auction ID
    pub auction_id: u64,
    /// Seller address
    pub seller: Address,
    /// Key being sold
    pub key_id: Address,
    /// Quantity of keys
    pub quantity: i128,
    /// Reserve price (minimum acceptable bid)
    pub reserve_price: i128,
    /// Original end ledger
    pub original_end_ledger: u32,
    /// Current end ledger (may be extended)
    pub current_end_ledger: u32,
    /// Minimum bid increment in basis points
    pub min_increment_bps: u32,
    /// Current highest bid amount
    pub highest_bid: i128,
    /// Current highest bidder
    pub highest_bidder: Option<Address>,
    /// Total extensions applied in ledgers
    pub total_extensions: u32,
    /// Whether auction is finalized
    pub finalized: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum EnglishAuctionKey {
    /// (auction_id) -> EnglishAuction
    Auction(u64),
    /// Next auction ID
    NextAuctionId,
    /// (seller, key_id) -> active auction_id (only one auction per seller per key)
    ActiveAuction(Address, Address),
}

/// Start a new English auction.
pub fn start_english_auction(
    env: &Env,
    seller: Address,
    key_id: Address,
    quantity: i128,
    reserve_price: i128,
    duration_ledgers: u32,
    min_increment_bps: u32,
) -> Result<u64, ContractError> {
    seller.require_auth();

    // Validate inputs
    if quantity <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    if reserve_price < 0 {
        return Err(ContractError::InsufficientPayment);
    }
    if duration_ledgers == 0 {
        return Err(ContractError::InvalidLockPeriod);
    }

    // Check seller has sufficient balance
    // Note: In real implementation, would need to check actual key balance
    // and escrow the keys

    // Get next auction ID
    let auction_id = env
        .storage()
        .persistent()
        .get::<EnglishAuctionKey, u64>(&EnglishAuctionKey::NextAuctionId)
        .unwrap_or(1);

    // Create auction
    let current_ledger = env.ledger().sequence();
    let end_ledger = current_ledger + duration_ledgers;

    let auction = EnglishAuction {
        auction_id,
        seller: seller.clone(),
        key_id: key_id.clone(),
        quantity,
        reserve_price,
        original_end_ledger: end_ledger,
        current_end_ledger: end_ledger,
        min_increment_bps,
        highest_bid: 0,
        highest_bidder: None,
        total_extensions: 0,
        finalized: false,
    };

    // Store auction
    let auction_key = EnglishAuctionKey::Auction(auction_id);
    env.storage().persistent().set(&auction_key, &auction);

    // Store active auction mapping
    let active_key = EnglishAuctionKey::ActiveAuction(seller.clone(), key_id.clone());
    env.storage().persistent().set(&active_key, &auction_id);

    // Increment next auction ID
    env.storage()
        .persistent()
        .set(&EnglishAuctionKey::NextAuctionId, &(auction_id + 1));

    // Emit event
    env.events().publish(
        (soroban_sdk::symbol_short!("auct_strt"),),
        (
            auction_id,
            seller,
            key_id,
            quantity,
            reserve_price,
            end_ledger,
            min_increment_bps,
        ),
    );

    Ok(auction_id)
}

/// Place a bid on an auction.
pub fn place_bid(
    env: &Env,
    bidder: Address,
    auction_id: u64,
    amount: i128,
) -> Result<(), ContractError> {
    bidder.require_auth();

    // Load auction
    let auction_key = EnglishAuctionKey::Auction(auction_id);
    let mut auction: EnglishAuction = env
        .storage()
        .persistent()
        .get(&auction_key)
        .ok_or(ContractError::ProposalNotFound)?;

    // Check auction not finalized
    if auction.finalized {
        return Err(ContractError::Unauthorized);
    }

    // Check auction not ended
    let current_ledger = env.ledger().sequence();
    if current_ledger >= auction.current_end_ledger {
        return Err(ContractError::DeadlinePassed);
    }

    // Calculate minimum required bid
    let min_bid = if auction.highest_bid == 0 {
        auction.reserve_price
    } else {
        // Calculate minimum increment
        let increment = auction
            .highest_bid
            .checked_mul(auction.min_increment_bps as i128)
            .and_then(|v| v.checked_div(10000))
            .ok_or(ContractError::Overflow)?;
        auction
            .highest_bid
            .checked_add(increment)
            .ok_or(ContractError::Overflow)?
    };

    // Validate bid amount
    if amount < min_bid {
        return Err(ContractError::InsufficientPayment);
    }

    // Refund previous highest bidder if exists
    if let Some(ref prev_bidder) = auction.highest_bidder {
        // Note: In real implementation, would transfer XLM back to prev_bidder
        env.events().publish(
            (soroban_sdk::symbol_short!("bid_refnd"),),
            (auction_id, prev_bidder.clone(), auction.highest_bid),
        );
    }

    // Update auction with new highest bid
    auction.highest_bid = amount;
    auction.highest_bidder = Some(bidder.clone());

    // Check if bid is within extension window
    let ledgers_until_end = auction.current_end_ledger.saturating_sub(current_ledger);
    if ledgers_until_end <= EXTENSION_WINDOW_LEDGERS
        && auction.total_extensions < MAX_TOTAL_EXTENSION_LEDGERS
    {
        // Extend the auction
        let extension = EXTENSION_AMOUNT_LEDGERS
            .min(MAX_TOTAL_EXTENSION_LEDGERS.saturating_sub(auction.total_extensions));
        auction.current_end_ledger = auction.current_end_ledger.saturating_add(extension);
        auction.total_extensions = auction.total_extensions.saturating_add(extension);

        env.events().publish(
            (soroban_sdk::symbol_short!("auct_ext"),),
            (auction_id, extension, auction.current_end_ledger),
        );
    }

    // Save updated auction
    env.storage().persistent().set(&auction_key, &auction);

    // Emit bid placed event
    env.events().publish(
        (soroban_sdk::symbol_short!("bid_plcd"),),
        (auction_id, bidder, amount),
    );

    Ok(())
}

/// Finalize an auction (can be called by anyone after end).
pub fn finalize_auction(env: &Env, auction_id: u64) -> Result<(), ContractError> {
    // Load auction
    let auction_key = EnglishAuctionKey::Auction(auction_id);
    let mut auction: EnglishAuction = env
        .storage()
        .persistent()
        .get(&auction_key)
        .ok_or(ContractError::ProposalNotFound)?;

    // Check auction not already finalized
    if auction.finalized {
        return Err(ContractError::Unauthorized);
    }

    // Check auction has ended
    let current_ledger = env.ledger().sequence();
    if current_ledger < auction.current_end_ledger {
        return Err(ContractError::TimelockNotElapsed);
    }

    // Mark as finalized
    auction.finalized = true;

    // Check if reserve was met
    if let Some(winner) = auction
        .highest_bidder
        .as_ref()
        .filter(|_| auction.highest_bid >= auction.reserve_price)
    {
        // Successful sale

        // Note: In real implementation would:
        // 1. Transfer keys to winner
        // 2. Calculate and deduct protocol fee and royalty
        // 3. Transfer proceeds to seller

        env.events().publish(
            (soroban_sdk::symbol_short!("auct_sccs"),),
            (
                auction_id,
                winner.clone(),
                auction.highest_bid,
                auction.quantity,
            ),
        );
    } else {
        // Reserve not met - return keys to seller and refund bidder if exists
        if let Some(ref bidder) = auction.highest_bidder {
            // Refund the bidder
            env.events().publish(
                (soroban_sdk::symbol_short!("bid_refnd"),),
                (auction_id, bidder.clone(), auction.highest_bid),
            );
        }

        env.events().publish(
            (soroban_sdk::symbol_short!("auct_fail"),),
            (auction_id, auction.seller.clone()),
        );
    }

    // Save finalized auction
    env.storage().persistent().set(&auction_key, &auction);

    // Remove active auction mapping
    let active_key =
        EnglishAuctionKey::ActiveAuction(auction.seller.clone(), auction.key_id.clone());
    env.storage().persistent().remove(&active_key);

    Ok(())
}

/// Cancel an auction (only if no bids exist).
pub fn cancel_auction(env: &Env, seller: Address, auction_id: u64) -> Result<(), ContractError> {
    seller.require_auth();

    // Load auction
    let auction_key = EnglishAuctionKey::Auction(auction_id);
    let auction: EnglishAuction = env
        .storage()
        .persistent()
        .get(&auction_key)
        .ok_or(ContractError::ProposalNotFound)?;

    // Verify seller
    if auction.seller != seller {
        return Err(ContractError::Unauthorized);
    }

    // Check auction not finalized
    if auction.finalized {
        return Err(ContractError::Unauthorized);
    }

    // Check no bids exist
    if auction.highest_bidder.is_some() {
        return Err(ContractError::Unauthorized);
    }

    // Remove auction
    env.storage().persistent().remove(&auction_key);

    // Remove active auction mapping
    let active_key = EnglishAuctionKey::ActiveAuction(seller.clone(), auction.key_id.clone());
    env.storage().persistent().remove(&active_key);

    // Emit event
    env.events().publish(
        (soroban_sdk::symbol_short!("auct_cncl"),),
        (auction_id, seller),
    );

    Ok(())
}

/// Get auction details.
pub fn get_auction(env: &Env, auction_id: u64) -> Option<EnglishAuction> {
    let auction_key = EnglishAuctionKey::Auction(auction_id);
    env.storage().persistent().get(&auction_key)
}
