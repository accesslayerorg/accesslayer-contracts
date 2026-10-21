//! Event definitions for the holder-rewards contract.
//!
//! ### Event Schema Stability
//!
//! Downstream indexers rely on stable field ordering in event payloads.
//! - Do not reorder existing fields.
//! - Add new fields only at the end.
//! - Never remove fields; keep deprecated ones with a default value.

use soroban_sdk::{contracttype, Address, Env, Symbol};

/// Emitted when an airdrop is created and its holder snapshot is taken.
pub const AIRDROP_CREATED_EVENT_NAME: &str = "AirdropCreated";

/// Emitted when a holder claims their pro-rata share of an airdrop.
pub const AIRDROP_CLAIMED_EVENT_NAME: &str = "AirdropClaimed";

/// Stable payload for the `AirdropCreated` event.
///
/// Topics: `(Symbol("AirdropCreated"), key_id)`
/// Data:   `AirdropCreatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AirdropCreatedEvent {
    pub airdrop_id: u64,
    pub key_id: Address,
    pub funder: Address,
    pub reward_token: Address,
    pub total_amount: i128,
    /// Sum of snapshot key balances across all holders with a non-zero
    /// balance at the creation ledger.
    pub total_snapshot_supply: u32,
    /// Ledger sequence number at which holder balances were snapshotted.
    pub snapshot_ledger: u32,
}

/// Stable payload for the `AirdropClaimed` event.
///
/// Topics: `(Symbol("AirdropClaimed"), key_id, wallet)`
/// Data:   `AirdropClaimedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AirdropClaimedEvent {
    pub airdrop_id: u64,
    pub key_id: Address,
    pub wallet: Address,
    /// Reward amount transferred to the claiming holder.
    pub amount: i128,
}

/// Build topics for `AirdropCreated`.
pub fn airdrop_created_topics(env: &Env, key_id: &Address) -> (Symbol, Address) {
    (Symbol::new(env, AIRDROP_CREATED_EVENT_NAME), key_id.clone())
}

/// Build topics for `AirdropClaimed`.
pub fn airdrop_claimed_topics(
    env: &Env,
    key_id: &Address,
    wallet: &Address,
) -> (Symbol, Address, Address) {
    (
        Symbol::new(env, AIRDROP_CLAIMED_EVENT_NAME),
        key_id.clone(),
        wallet.clone(),
    )
}
