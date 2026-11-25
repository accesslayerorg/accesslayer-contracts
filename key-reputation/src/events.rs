//! Centralized event names and helpers for consistent event emission.
//!
//! ### Event Schema Stability
//!
//! Downstream indexers rely on the stable ordering of fields in event payloads.
//! When modifying event structures:
//! - **Do not reorder** existing fields.
//! - **Add new fields** only at the end of the structure to maintain compatibility.
//! - **Avoid removing fields**; if a field is deprecated, keep it with a default value.

use soroban_sdk::{contracttype, symbol_short, Address, Symbol};

/// Event name emitted whenever a composite score is recomputed.
pub const KEY_SCORE_UPDATED_NAME: Symbol = symbol_short!("ks_update");

/// Topic index constants for `KeyScoreUpdated`.
pub const TOPIC_EVENT_NAME_INDEX: u32 = 0;
pub const TOPIC_KEY_ID_INDEX: u32 = 1;
pub const TOPIC_METRIC_INDEX: u32 = 2;

/// Stable field order for `KeyScoreUpdated` payload.
pub const KEY_SCORE_UPDATED_FIELDS: [&str; 3] = ["key_id", "metric", "composite_score"];

/// Emitted payload for a score update.
///
/// Event shape:
/// - topics: `(KEY_SCORE_UPDATED_NAME, key_id, metric_u32)`
/// - data: `KeyScoreUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyScoreUpdatedEvent {
    /// The key whose score changed.
    pub key_id: Address,
    /// Numeric discriminant of the metric that triggered the update.
    pub metric: u32,
    /// New composite score in the 0–1000 range.
    pub composite_score: u32,
}

/// Build the topics tuple for a `KeyScoreUpdated` event.
pub fn key_score_updated_topics(key_id: &Address, metric: u32) -> (Symbol, Address, u32) {
    (KEY_SCORE_UPDATED_NAME, key_id.clone(), metric)
}
