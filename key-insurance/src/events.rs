//! Event definitions for the key-insurance contract.
//!
//! ### Event Schema Stability
//!
//! Downstream indexers rely on stable field ordering in event payloads.
//! - Do not reorder existing fields.
//! - Add new fields only at the end.
//! - Never remove fields; keep deprecated ones with a default value.

use soroban_sdk::{contracttype, symbol_short, Address, Symbol};

/// Emitted when a holder successfully purchases a policy.
pub const INSURANCE_PURCHASED_NAME: Symbol = symbol_short!("ins_buy");

/// Emitted when a payout is triggered for a holder.
pub const INSURANCE_PAYOUT_NAME: Symbol = symbol_short!("ins_pay");

// ── InsurancePurchased ────────────────────────────────────────────────────────

/// Stable payload for the `InsurancePurchased` event.
///
/// Topics: `(INSURANCE_PURCHASED_NAME, wallet, key_id)`
/// Data:   `InsurancePurchasedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct InsurancePurchasedEvent {
    pub wallet: Address,
    pub key_id: Address,
    /// Coverage amount locked by the policy (in contract token units).
    pub coverage_amount: i128,
    /// Trigger threshold expressed in basis points of the purchase price.
    pub threshold_bps: u32,
    /// Premium paid by the holder.
    pub premium: i128,
    /// Ledger timestamp at which the policy expires.
    pub expiry: u64,
}

/// Build topics for `InsurancePurchased`.
pub fn insurance_purchased_topics(wallet: &Address, key_id: &Address) -> (Symbol, Address, Address) {
    (INSURANCE_PURCHASED_NAME, wallet.clone(), key_id.clone())
}

// ── InsurancePayoutTriggered ──────────────────────────────────────────────────

/// Stable payload for the `InsurancePayoutTriggered` event.
///
/// Topics: `(INSURANCE_PAYOUT_NAME, wallet, key_id)`
/// Data:   `InsurancePayoutTriggeredEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct InsurancePayoutTriggeredEvent {
    pub wallet: Address,
    pub key_id: Address,
    /// Actual amount paid out (≤ coverage_amount).
    pub payout_amount: i128,
}

/// Build topics for `InsurancePayoutTriggered`.
pub fn insurance_payout_topics(wallet: &Address, key_id: &Address) -> (Symbol, Address, Address) {
    (INSURANCE_PAYOUT_NAME, wallet.clone(), key_id.clone())
}
