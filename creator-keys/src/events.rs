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

use soroban_sdk::{contracttype, symbol_short, Address, String, Symbol};

/// Event name for creator registration.
pub const REGISTER_EVENT_NAME: Symbol = symbol_short!("register");

/// Event name for key purchase.
pub const BUY_EVENT_NAME: Symbol = symbol_short!("buy");

/// Common topic indexes for event tuple topics.
pub const TOPIC_EVENT_NAME_INDEX: u32 = 0;
pub const TOPIC_CREATOR_INDEX: u32 = 1;
pub const TOPIC_BUYER_INDEX: u32 = 2;

/// Stable field order for registration event payloads.
pub const REGISTER_EVENT_DATA_FIELDS: [&str; 4] = ["creator", "handle", "supply", "holder_count"];

/// Stable field order for buy event tuple payloads.
pub const BUY_EVENT_DATA_FIELDS: [&str; 2] = ["supply", "payment"];

/// Quote-related field semantics for event payload interpretation.
///
/// These fields are derived from quote calculations and may appear in event
/// payloads or be referenced by indexers correlating events with quote data.
///
/// ### Field Types and Semantics
///
/// | Field          | Type  | Description                                              |
/// |----------------|-------|----------------------------------------------------------|
/// | `price`        | i128  | Base key price before fees (always positive)            |
/// | `creator_fee`  | i128  | Fee allocated to creator treasury (basis points applied)|
/// | `protocol_fee` | i128  | Fee allocated to protocol treasury (basis points applied)|
/// | `total_amount` | i128  | Net amount: `price + fees` for buys, `price - fees` for sells |
///
/// ### Compatibility Notes for Indexers
///
/// - All monetary values are in the smallest unit (e.g., stroops for XLM).
/// - Fee values are computed using basis points (1 BPS = 0.01%).
/// - `total_amount` direction depends on operation type:
///   - **Buy**: `total_amount = price + creator_fee + protocol_fee`
///   - **Sell**: `total_amount = price - creator_fee - protocol_fee`
/// - Indexers should handle potential overflow/underflow scenarios where
///   `total_amount` may differ from simple arithmetic due to checked math.
/// - Quote responses are not emitted directly in events; indexers should
///   call `get_buy_quote` or `get_sell_quote` for fee breakdown details.
pub const QUOTE_FIELD_SEMANTICS: [&str; 4] = ["price", "creator_fee", "protocol_fee", "total_amount"];

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
}

/// Shared registration event topics tuple.
pub fn register_event_topics(creator: &Address) -> (Symbol, Address) {
    (REGISTER_EVENT_NAME, creator.clone())
}

/// Shared buy event topics tuple.
pub fn buy_event_topics(creator: &Address, buyer: &Address) -> (Symbol, Address, Address) {
    (BUY_EVENT_NAME, creator.clone(), buyer.clone())
}
