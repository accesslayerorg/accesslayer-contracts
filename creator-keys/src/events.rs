//! Centralized event names and helpers for consistent event emission.
//!
//! This module provides a single source of truth for event names used throughout
//! the contract, reducing string duplication and ensuring consistency across
//! event emission paths.
//!
//! ### Event Schema Stability
//!
//! Downstream indexers rely on the stable ordering of fields in event payloads.
//! When modifying event structures:
//! - **Do not reorder** existing fields.
//! - **Add new fields** only at the end of the structure to maintain compatibility.
//! - **Avoid removing fields**; if a field is deprecated, keep it with a default value.
//!
//! This approach ensures that indexers can reliably parse event data across
//! different contract versions.
//!
//! ### Quote-Related Event Field Semantics
//!
//! - `supply`: Number of keys in circulation after the trade (for buy/sell events)
//! - `payment`: Total amount paid by the buyer (for buy events, ≥ key price)

use soroban_sdk::{Address, String, Symbol, contracttype, symbol_short}; 

/// Event name for protocol pause.
pub const PAUSE_EVENT_NAME: Symbol = symbol_short!("pause");

/// Event name for protocol unpause.
pub const UNPAUSE_EVENT_NAME: Symbol = symbol_short!("unpause");

/// Event name for creator registration.
pub const REGISTER_EVENT_NAME: Symbol = symbol_short!("register");

/// Event name for key purchase.
pub const BUY_EVENT_NAME: Symbol = symbol_short!("buy");

/// Event name for key sale.
pub const SELL_EVENT_NAME: Symbol = symbol_short!("sell");

/// Common topic indexes for event tuple topics.
pub const TOPIC_EVENT_NAME_INDEX: u32 = 0;
pub const TOPIC_CREATOR_INDEX: u32 = 1;
pub const TOPIC_BUYER_INDEX: u32 = 2;

/// Stable field order for registration event payloads.
pub const REGISTER_EVENT_DATA_FIELDS: [&str; 6] = [
    "creator",
    "handle",
    "supply",
    "holder_count",
    "creator_bps",
    "protocol_bps",
];

/// Number of fields in the registration event data payload.
pub const REGISTER_EVENT_FIELD_COUNT: usize = REGISTER_EVENT_DATA_FIELDS.len();

/// Stable field order for buy event tuple payloads.
pub const BUY_EVENT_DATA_FIELDS: [&str; 2] = ["supply", "payment"];

/// Number of fields in the buy event data payload.
pub const BUY_EVENT_FIELD_COUNT: usize = BUY_EVENT_DATA_FIELDS.len();

/// Stable field order for sell event tuple payloads.
pub const SELL_EVENT_DATA_FIELDS: [&str; 1] = ["supply"];

/// Number of fields in the sell event data payload.
pub const SELL_EVENT_FIELD_COUNT: usize = SELL_EVENT_DATA_FIELDS.len();

/// Event name for dividend distribution.
pub const DIVIDEND_DISTRIBUTED: Symbol = symbol_short!("divdist");

/// Event name for dividend claim.
pub const DIVIDEND_CLAIMED: Symbol = symbol_short!("divclm");

/// Stable registration event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(REGISTER_EVENT_NAME, creator)`
/// - data: `CreatorRegisteredEvent`
///
/// This keeps the creator address indexed in event topics while preserving
/// a predictable payload for off-chain consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CreatorRegisteredEvent {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
    pub holder_count: u32,
    pub creator_bps: u32,
    pub protocol_bps: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendDistributedEvent {
    pub creator_id: Address,
    pub total_amount: i128,
    pub snapshot_supply: i128,
    pub ledger: u32,
    pub protocol_fee: i128,
    pub distributed_amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendClaimedEvent {
    pub creator_id: Address,
    pub claimant: Address,
    pub amount: i128,
}

/// Shared registration event topics tuple.
pub fn register_event_topics(creator: &Address) -> (Symbol, Address) {
    (REGISTER_EVENT_NAME, creator.clone())
}

/// Shared buy event topics tuple.
pub fn buy_event_topics(creator: &Address, buyer: &Address) -> (Symbol, Address, Address) {
    (BUY_EVENT_NAME, creator.clone(), buyer.clone())
}

pub fn dividend_distributed_topics(creator: &Address) -> (Symbol, Address) {
    (DIVIDEND_DISTRIBUTED, creator.clone())
}

pub fn dividend_claimed_topics(
    creator: &Address,
    claimant: &Address,
) -> (Symbol, Address, Address) {
    (DIVIDEND_CLAIMED, creator.clone(), claimant.clone())
}
