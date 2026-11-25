#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

pub mod events;

// ── Error types ──────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
/// Contract error variants.
///
/// # Stability and Ordering
///
/// **IMPORTANT**: New error variants MUST be appended at the end and NEVER inserted
/// mid-enum. The numeric discriminants are part of the ABI.
pub enum ContractError {
    /// Caller is not in the authorised-callers set.
    Unauthorised = 1,
    /// A metric value was outside the accepted 0–1000 range.
    MetricOutOfRange = 2,
    /// The requested metric type discriminant is unknown.
    UnknownMetric = 3,
    /// A weight update would push the total weight beyond 10_000 bps.
    WeightOverflow = 4,
    /// Admin address has not been set.
    AdminNotSet = 5,
    /// Arithmetic overflow during score computation.
    Overflow = 6,
}

// ── MetricType ───────────────────────────────────────────────────────────────

/// Discriminants for each metric fed into the composite score.
///
/// The `u32` representation is stable ABI: existing values must not be renumbered.
pub const METRIC_TRADE_VOLUME: u32 = 0;
pub const METRIC_HOLDER_COUNT: u32 = 1;
pub const METRIC_PRICE_STABILITY_BPS: u32 = 2;
pub const METRIC_RATING_AVG: u32 = 3;

/// Number of recognised metric types.
pub const METRIC_COUNT: usize = 4;

/// Maximum value a single metric may carry (maps to full score of 1000).
pub const METRIC_MAX: u32 = 1_000;

/// Total weight denominator — weights for all metrics must sum to this.
pub const WEIGHT_TOTAL: u32 = 10_000;

// ── Storage keys ─────────────────────────────────────────────────────────────

#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    /// Admin address (can update weights and manage the authorised-callers set).
    Admin,
    /// Authorised caller flag: `DataKey::AuthorisedCaller(caller_address)`.
    AuthorisedCaller(Address),
    /// Per-metric weights tuple stored as a single value: `DataKey::Weights`.
    Weights,
    /// Per-key metric values: `DataKey::Metrics(key_id)`.
    Metrics(Address),
}

// ── Public data structures ────────────────────────────────────────────────────

/// Per-metric weights. All values are in basis points and must sum to [`WEIGHT_TOTAL`].
///
/// Default (50/15/20/15 split):
/// - trade_volume_weight:       5000 (50%)
/// - holder_count_weight:       1500 (15%)
/// - price_stability_weight:    2000 (20%)
/// - rating_avg_weight:         1500 (15%)
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct MetricWeights {
    pub trade_volume_weight: u32,
    pub holder_count_weight: u32,
    pub price_stability_weight: u32,
    pub rating_avg_weight: u32,
}

impl MetricWeights {
    /// Default weights shipped with the contract.
    pub fn default_weights() -> Self {
        MetricWeights {
            trade_volume_weight: 5_000,
            holder_count_weight: 1_500,
            price_stability_weight: 2_000,
            rating_avg_weight: 1_500,
        }
    }

    /// Returns `true` when all weights sum to [`WEIGHT_TOTAL`].
    pub fn is_valid(&self) -> bool {
        let sum = self
            .trade_volume_weight
            .checked_add(self.holder_count_weight)
            .and_then(|s| s.checked_add(self.price_stability_weight))
            .and_then(|s| s.checked_add(self.rating_avg_weight));
        sum == Some(WEIGHT_TOTAL)
    }
}

/// Snapshot of all four metric values for a single key.
///
/// Each field is in the 0–1000 normalised range.
#[derive(Clone, Eq, PartialEq, Default)]
#[contracttype]
pub struct KeyMetrics {
    pub trade_volume: u32,
    pub holder_count: u32,
    pub price_stability_bps: u32,
    pub rating_avg: u32,
}

/// Composite score plus the per-metric breakdown returned by `get_key_score`.
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct KeyScoreView {
    pub key_id: Address,
    /// Weighted composite in the 0–1000 range.
    pub composite_score: u32,
    /// Individual metric snapshots (each 0–1000).
    pub metrics: KeyMetrics,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn read_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&DataKey::Admin)
}

fn read_weights(env: &Env) -> MetricWeights {
    env.storage()
        .persistent()
        .get(&DataKey::Weights)
        .unwrap_or_else(MetricWeights::default_weights)
}

fn read_metrics(env: &Env, key_id: &Address) -> KeyMetrics {
    env.storage()
        .persistent()
        .get(&DataKey::Metrics(key_id.clone()))
        .unwrap_or_default()
}

/// Compute the weighted composite score from current metrics and weights.
///
/// Each metric is in 0–1000 and weighted by its bps share of [`WEIGHT_TOTAL`].
/// Returns `None` on arithmetic overflow (should be unreachable for in-range inputs).
fn compute_composite(metrics: &KeyMetrics, weights: &MetricWeights) -> Option<u32> {
    // Each term: metric_value * weight_bps, then sum / WEIGHT_TOTAL.
    // All values ≤ 1000 and weights ≤ 10_000 so max intermediate = 1000 * 10_000 = 10_000_000
    // which fits comfortably in u32 per term and u64 for the accumulated sum.
    let sum = (metrics.trade_volume as u64)
        .checked_mul(weights.trade_volume_weight as u64)?
        .checked_add((metrics.holder_count as u64).checked_mul(weights.holder_count_weight as u64)?)?
        .checked_add(
            (metrics.price_stability_bps as u64)
                .checked_mul(weights.price_stability_weight as u64)?,
        )?
        .checked_add(
            (metrics.rating_avg as u64).checked_mul(weights.rating_avg_weight as u64)?,
        )?;

    // Divide by WEIGHT_TOTAL (10_000); result ≤ 1000.
    Some((sum / WEIGHT_TOTAL as u64) as u32)
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct KeyReputationContract;

#[contractimpl]
impl KeyReputationContract {
    // ── Admin bootstrap ──────────────────────────────────────────────────────

    /// Set the protocol admin address.
    ///
    /// The first caller can set admin freely; subsequent calls require the existing
    /// admin to authorise. This mirrors the pattern in `creator-keys`.
    pub fn set_admin(env: Env, admin: Address) {
        if let Some(existing) = read_admin(&env) {
            existing.require_auth();
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
    }

    // ── Authorised-caller management (admin only) ─────────────────────────

    /// Grant a contract address permission to call `update_key_score`.
    pub fn add_authorised_caller(
        env: Env,
        admin: Address,
        caller: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        let stored_admin = read_admin(&env).ok_or(ContractError::AdminNotSet)?;
        if admin != stored_admin {
            return Err(ContractError::Unauthorised);
        }
        env.storage()
            .persistent()
            .set(&DataKey::AuthorisedCaller(caller), &true);
        Ok(())
    }

    /// Revoke an address's permission to call `update_key_score`.
    pub fn remove_authorised_caller(
        env: Env,
        admin: Address,
        caller: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        let stored_admin = read_admin(&env).ok_or(ContractError::AdminNotSet)?;
        if admin != stored_admin {
            return Err(ContractError::Unauthorised);
        }
        env.storage()
            .persistent()
            .remove(&DataKey::AuthorisedCaller(caller));
        Ok(())
    }

    /// Returns `true` when `caller` is in the authorised-callers set.
    pub fn is_authorised_caller(env: Env, caller: Address) -> bool {
        env.storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::AuthorisedCaller(caller))
            .unwrap_or(false)
    }

    // ── Weight management (admin only) ───────────────────────────────────────

    /// Update the metric weights used for composite score computation.
    ///
    /// All four weights must sum to exactly [`WEIGHT_TOTAL`] (10_000).
    pub fn set_metric_weights(
        env: Env,
        admin: Address,
        trade_volume_weight: u32,
        holder_count_weight: u32,
        price_stability_weight: u32,
        rating_avg_weight: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        let stored_admin = read_admin(&env).ok_or(ContractError::AdminNotSet)?;
        if admin != stored_admin {
            return Err(ContractError::Unauthorised);
        }

        let weights = MetricWeights {
            trade_volume_weight,
            holder_count_weight,
            price_stability_weight,
            rating_avg_weight,
        };
        if !weights.is_valid() {
            return Err(ContractError::WeightOverflow);
        }

        env.storage().persistent().set(&DataKey::Weights, &weights);
        Ok(())
    }

    /// Read the current metric weights.
    pub fn get_metric_weights(env: Env) -> MetricWeights {
        read_weights(&env)
    }

    // ── Score writes (authorised callers only) ───────────────────────────────

    /// Update a single metric for `key_id` and recompute the composite score.
    ///
    /// Only addresses registered via `add_authorised_caller` may call this.
    /// `metric_type` must be one of the `METRIC_*` constants (0–3).
    /// `value` must be in the 0–1000 range.
    ///
    /// Emits [`events::KeyScoreUpdatedEvent`] with the new composite score.
    pub fn update_key_score(
        env: Env,
        caller: Address,
        key_id: Address,
        metric_type: u32,
        value: u32,
    ) -> Result<u32, ContractError> {
        caller.require_auth();

        // Authorisation check.
        let is_auth: bool = env
            .storage()
            .persistent()
            .get(&DataKey::AuthorisedCaller(caller))
            .unwrap_or(false);
        if !is_auth {
            return Err(ContractError::Unauthorised);
        }

        // Range-check metric value.
        if value > METRIC_MAX {
            return Err(ContractError::MetricOutOfRange);
        }

        // Load current metrics and apply the update.
        let mut metrics = read_metrics(&env, &key_id);
        match metric_type {
            METRIC_TRADE_VOLUME => metrics.trade_volume = value,
            METRIC_HOLDER_COUNT => metrics.holder_count = value,
            METRIC_PRICE_STABILITY_BPS => metrics.price_stability_bps = value,
            METRIC_RATING_AVG => metrics.rating_avg = value,
            _ => return Err(ContractError::UnknownMetric),
        }

        // Persist updated metrics.
        env.storage()
            .persistent()
            .set(&DataKey::Metrics(key_id.clone()), &metrics);

        // Compute composite score.
        let weights = read_weights(&env);
        let composite = compute_composite(&metrics, &weights).ok_or(ContractError::Overflow)?;

        // Emit event before returning so indexers observe the update.
        env.events().publish(
            events::key_score_updated_topics(&key_id, metric_type),
            events::KeyScoreUpdatedEvent {
                key_id: key_id.clone(),
                metric: metric_type,
                composite_score: composite,
            },
        );

        Ok(composite)
    }

    // ── Score reads ──────────────────────────────────────────────────────────

    /// Return the composite score and per-metric breakdown for `key_id`.
    ///
    /// Returns zeros for an unknown key (sparse storage, never panics).
    pub fn get_key_score(env: Env, key_id: Address) -> KeyScoreView {
        let metrics = read_metrics(&env, &key_id);
        let weights = read_weights(&env);
        // compute_composite is infallible for in-range inputs; default to 0 on edge overflow.
        let composite = compute_composite(&metrics, &weights).unwrap_or(0);
        KeyScoreView {
            key_id,
            composite_score: composite,
            metrics,
        }
    }
}
