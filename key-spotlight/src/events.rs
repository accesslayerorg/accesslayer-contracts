//! Event definitions for the key-spotlight contract.
//!
//! ### Event Schema Stability
//!
//! - Do not reorder existing fields.
//! - Add new fields only at the end.
//! - Never remove fields; keep deprecated ones with a default value.

use soroban_sdk::{contracttype, symbol_short, Address, Symbol};

/// Emitted when a creator places or raises a bid on a time slot.
pub const SPOTLIGHT_BID_PLACED_NAME: Symbol = symbol_short!("sp_bid");

/// Emitted when a slot's winning bid is finalised (slot start time reached).
pub const SPOTLIGHT_AWARDED_NAME: Symbol = symbol_short!("sp_award");

// ── SpotlightBidPlaced ────────────────────────────────────────────────────────

/// Stable payload for `SpotlightBidPlaced`.
///
/// Topics: `(SPOTLIGHT_BID_PLACED_NAME, key_id, slot_id)`
/// Data:   `SpotlightBidPlacedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SpotlightBidPlacedEvent {
    pub key_id: Address,
    /// Numeric slot identifier (unix timestamp of slot start, rounded to day boundary).
    pub slot_id: u64,
    pub bid_amount: i128,
    /// Address of the bidder (creator authorising the call).
    pub bidder: Address,
}

pub fn spotlight_bid_placed_topics(key_id: &Address, slot_id: u64) -> (Symbol, Address, u64) {
    (SPOTLIGHT_BID_PLACED_NAME, key_id.clone(), slot_id)
}

// ── SpotlightAwarded ──────────────────────────────────────────────────────────

/// Stable payload for `SpotlightAwarded`.
///
/// Topics: `(SPOTLIGHT_AWARDED_NAME, key_id, slot_id)`
/// Data:   `SpotlightAwardedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SpotlightAwardedEvent {
    pub key_id: Address,
    pub slot_id: u64,
    pub winning_amount: i128,
    pub duration_days: u32,
}

pub fn spotlight_awarded_topics(key_id: &Address, slot_id: u64) -> (Symbol, Address, u64) {
    (SPOTLIGHT_AWARDED_NAME, key_id.clone(), slot_id)
}
