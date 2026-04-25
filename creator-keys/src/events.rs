//! Centralized event names and helpers for consistent event emission.
//!
//! This module provides a single source of truth for event names used throughout
//! the contract, reducing string duplication and ensuring consistency across
//! event emission paths.

use soroban_sdk::{contracttype, symbol_short, Address, String, Symbol};

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
pub const REGISTER_EVENT_DATA_FIELDS: [&str; 4] = ["creator", "handle", "supply", "holder_count"];

/// Stable field order for buy event tuple payloads.
pub const BUY_EVENT_DATA_FIELDS: [&str; 2] = ["supply", "payment"];

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
