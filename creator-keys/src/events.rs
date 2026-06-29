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

use soroban_sdk::{contracttype, symbol_short, Address, String, Symbol};

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

/// Event name for peer-to-peer key transfer.
pub const TRANSFER_EVENT_NAME: Symbol = symbol_short!("transfer");

/// Event name for creator buyback.
pub const BUYBACK_EVENT_NAME: Symbol = symbol_short!("buyback");

/// Common topic indexes for event tuple topics.
pub const TOPIC_EVENT_NAME_INDEX: u32 = 0;
pub const TOPIC_CREATOR_INDEX: u32 = 1;
pub const TOPIC_BUYER_INDEX: u32 = 2;

/// Stable field order for registration event payloads.
pub const REGISTER_EVENT_DATA_FIELDS: [&str; 7] = [
    "creator",
    "handle",
    "supply",
    "holder_count",
    "creator_bps",
    "protocol_bps",
    "curve_preset", // NEW
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

/// Stable field order for buyback event payloads.
pub const BUYBACK_EVENT_DATA_FIELDS: [&str; 5] =
    ["creator", "amount", "price_paid", "new_supply", "ledger"];

/// Number of fields in the buyback event data payload.
pub const BUYBACK_EVENT_FIELD_COUNT: usize = BUYBACK_EVENT_DATA_FIELDS.len();

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
    pub curve_preset: crate::bonding_curve::CurvePreset,
}

/// Shared registration event topics tuple.
pub fn register_event_topics(creator: &Address) -> (Symbol, Address) {
    (REGISTER_EVENT_NAME, creator.clone())
}

/// Stable buyback event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(BUYBACK_EVENT_NAME, creator)`
/// - data: `KeysBoughtBackEvent`
///
/// # Creator Fee Waiver
/// On buybacks, the creator fee is explicitly waived because the creator cannot pay
/// themselves a fee. The protocol fee still applies.
///
/// # Indexer Note
/// This event represents a creator burning keys from their own held balance,
/// which is distinct from a regular buy event. Indexers should process this
/// event separately from `BUY_EVENT_NAME` events to correctly track supply
/// changes and fee accounting.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysBoughtBackEvent {
    /// Address of the creator performing the buyback.
    pub creator: Address,
    /// Number of keys being bought back and burned.
    pub amount: u32,
    /// Total amount paid by the creator, including protocol fee (but not creator fee).
    pub price_paid: i128,
    /// New total supply of keys for the creator after the buyback.
    pub new_supply: u32,
    /// Ledger sequence number at the time of the buyback.
    pub ledger: u32,
}

/// Shared buy event topics tuple.
pub fn buy_event_topics(creator: &Address, buyer: &Address) -> (Symbol, Address, Address) {
    (BUY_EVENT_NAME, creator.clone(), buyer.clone())
}

/// Shared peer-to-peer transfer event topics tuple.
pub fn transfer_event_topics(creator: &Address, from: &Address) -> (Symbol, Address, Address) {
    (TRANSFER_EVENT_NAME, creator.clone(), from.clone())
}

/// Shared buyback event topics tuple.
pub fn buyback_event_topics(creator: &Address) -> (Symbol, Address) {
    (BUYBACK_EVENT_NAME, creator.clone())
}

/// Event name for dividend distribution.
pub const DIVIDEND_DISTRIBUTED_EVENT_NAME: Symbol = symbol_short!("div_dist");

/// Event name for dividend claim.
pub const DIVIDEND_CLAIMED_EVENT_NAME: Symbol = symbol_short!("div_claim");

/// Event name for allocation locked.
pub const ALLOCATION_LOCKED_EVENT_NAME: Symbol = symbol_short!("alloc_lck");

/// Event name for allocation claimed.
pub const ALLOCATION_CLAIMED_EVENT_NAME: Symbol = symbol_short!("alloc_clm");

/// Event name for protocol fee recipient updated.
pub const PROTOCOL_FEE_RECIPIENT_UPDATED_EVENT_NAME: Symbol = symbol_short!("p_fee_upd");

/// Event name for creator fee recipient updated.
pub const CREATOR_FEE_RECIPIENT_UPDATED_EVENT_NAME: Symbol = symbol_short!("c_fee_upd");

/// Stable field order for dividend distributed event payloads.
pub const DIVIDEND_DISTRIBUTED_DATA_FIELDS: [&str; 4] =
    ["creator", "total_amount", "snapshot_supply", "ledger"];

/// Stable field order for dividend claimed event payloads.
pub const DIVIDEND_CLAIMED_DATA_FIELDS: [&str; 3] = ["creator", "claimant", "amount"];

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendDistributedEvent {
    pub creator: Address,
    pub total_amount: i128,
    pub snapshot_supply: u32,
    pub ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendClaimedEvent {
    pub creator: Address,
    pub claimant: Address,
    pub amount: i128,
}

pub fn dividend_distributed_topics(creator: &Address) -> (Symbol, Address) {
    (DIVIDEND_DISTRIBUTED_EVENT_NAME, creator.clone())
}

pub fn dividend_claimed_topics(
    creator: &Address,
    claimant: &Address,
) -> (Symbol, Address, Address) {
    (
        DIVIDEND_CLAIMED_EVENT_NAME,
        creator.clone(),
        claimant.clone(),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AllocationLockedEvent {
    pub creator_id: Address,
    pub amount: u32,
    pub unlock_ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AllocationClaimedEvent {
    pub creator_id: Address,
    pub amount: u32,
    pub ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ProtocolFeeRecipientUpdatedEvent {
    pub old_recipient: Address,
    pub new_recipient: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CreatorFeeRecipientUpdatedEvent {
    pub creator_id: Address,
    pub old_recipient: Address,
    pub new_recipient: Address,
}

/// Event name for key transfer.
pub const KEYS_TRANSFERRED_EVENT_NAME: Symbol = symbol_short!("xfer");

/// Stable field order for key transfer event payloads.
pub const KEYS_TRANSFERRED_DATA_FIELDS: [&str; 5] =
    ["creator_id", "from", "to", "amount", "ledger"];

/// Stable key transfer event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(KEYS_TRANSFERRED_EVENT_NAME, creator_id, from)`
/// - data: `KeysTransferredEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysTransferredEvent {
    pub creator_id: Address,
    pub from: Address,
    pub to: Address,
    pub amount: u32,
    pub ledger: u32,
}
