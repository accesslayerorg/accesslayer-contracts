//! Event definitions for the holder-milestones contract.
//!
//! ### Event Schema Stability
//!
//! Downstream indexers rely on stable field ordering in event payloads.
//! - Do not reorder existing fields.
//! - Add new fields only at the end.
//! - Never remove fields; keep deprecated ones with a default value.

use soroban_sdk::{contracttype, symbol_short, Address, Symbol};

/// Emitted when a holder successfully claims a milestone reward.
pub const MILESTONE_REWARD_CLAIMED_NAME: Symbol = symbol_short!("ms_claim");

/// Stable payload for the `MilestoneRewardClaimed` event.
///
/// Topics: `(MILESTONE_REWARD_CLAIMED_NAME, wallet, key_id)`
/// Data:   `MilestoneRewardClaimedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MilestoneRewardClaimedEvent {
    pub wallet: Address,
    pub key_id: Address,
    /// Zero-based index into the milestone schedule (0 = 30d, 1 = 90d, …).
    pub milestone_index: u32,
    /// Reward amount distributed to the holder.
    pub amount: i128,
}

/// Build topics for `MilestoneRewardClaimed`.
pub fn milestone_reward_claimed_topics(
    wallet: &Address,
    key_id: &Address,
) -> (Symbol, Address, Address) {
    (MILESTONE_REWARD_CLAIMED_NAME, wallet.clone(), key_id.clone())
}
