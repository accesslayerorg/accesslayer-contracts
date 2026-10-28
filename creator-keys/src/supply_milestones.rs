//! Graduated supply milestones with automatic per-key fee reduction.
//!
//! Tracks bonding-curve graduation thresholds per key and automatically lowers
//! the key's fee once its circulating supply crosses configured milestones.
//! Milestones are applied in ascending supply order and the fee never regresses
//! to a higher value once a lower tier has been reached.
//!
//! Additionally provides a graduated bonding curve: a base exponent applied
//! below the first supply milestone and steeper exponents above each
//! configurable supply threshold.

use soroban_sdk::{contracttype, Address, BytesN, Env, Vec};

use crate::{
    assert_is_admin, extend_key_ttl_to_full_window, read_creator_supply,
    read_registered_creator_profile, ContractError, DataKey,
};

/// Maximum number of milestone tiers that can be configured for a single key.
pub const MAX_MILESTONE_TIERS: u32 = 5;

/// Maximum fee in basis points (`10_000` bps = 100%).
pub const MAX_FEE_BPS: u32 = 10_000;

/// Minimum allowed bonding-curve exponent.
pub const MIN_CURVE_EXPONENT: u32 = 1;

/// Maximum allowed bonding-curve exponent.
pub const MAX_CURVE_EXPONENT: u32 = 5;

/// A graduated milestone tier: a supply threshold and the fee bps that
/// applies once that threshold is reached.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MilestoneTier {
    /// Circulating supply threshold at which this tier becomes active.
    pub supply: u32,
    /// Fee in basis points applied once `supply` is reached.
    pub fee_bps: u32,
}

/// Persisted milestone state for a single key.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MilestoneConfig {
    /// Milestone tiers in strictly ascending supply order.
    pub tiers: Vec<MilestoneTier>,
    /// Number of tiers whose supply threshold has been reached (monotonic).
    pub applied_count: u32,
    /// Fee bps currently in effect for the key (`0` when no tier reached yet).
    pub current_fee_bps: u32,
}

/// A single graduated curve milestone: a supply threshold and the exponent
/// that applies once that threshold is reached.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CurveMilestone {
    /// Circulating supply threshold at which this exponent becomes active.
    pub supply: u32,
    /// Exponent applied once `supply` is reached (1..=5).
    pub exponent: u32,
}

/// Persisted graduated curve configuration for a single key.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct GraduatedCurveConfig {
    /// Milestones in strictly ascending supply order.
    pub milestones: Vec<CurveMilestone>,
}

/// Storage key for a key's milestone configuration.
fn storage_key(key_id: &Address) -> DataKey {
    DataKey::KeyMilestones(key_id.clone())
}

/// Storage key for a key's graduated curve configuration.
fn curve_storage_key(key_id: &Address) -> DataKey {
    DataKey::KeyGraduatedCurve(key_id.clone())
}

/// Configure the graduated milestone tiers for a key.
///
/// # Access
/// Admin-only (`admin.require_auth()` plus [`assert_is_admin`]). Intended to be
/// called at key creation, before any swaps move the key's supply.
///
/// # Validation
/// - `key_id` must be a registered key, otherwise [`ContractError::NotRegistered`].
/// - `milestones` must contain `1..=MAX_MILESTONE_TIERS` tiers.
/// - Tiers must be in strictly ascending `supply` order, each `supply > 0`,
///   otherwise [`ContractError::NotPositiveAmount`].
/// - Each `fee_bps` must be `<= MAX_FEE_BPS`, otherwise
///   [`ContractError::InvalidFeeConfig`].
///
/// On success the config is stored with no tier applied yet (`applied_count =
/// 0`, `current_fee_bps = 0`).
pub fn set_milestones(
    env: &Env,
    admin: &Address,
    key_id: &Address,
    milestones: Vec<MilestoneTier>,
) -> Result<(), ContractError> {
    admin.require_auth();
    assert_is_admin(env, admin)?;
    read_registered_creator_profile(env, key_id)?;

    let count = milestones.len();
    if count == 0 || count > MAX_MILESTONE_TIERS {
        return Err(ContractError::NotPositiveAmount);
    }

    let mut previous_supply = 0u32;
    for tier in milestones.iter() {
        if tier.supply == 0 || tier.supply <= previous_supply {
            return Err(ContractError::NotPositiveAmount);
        }
        if tier.fee_bps > MAX_FEE_BPS {
            return Err(ContractError::InvalidFeeConfig);
        }
        previous_supply = tier.supply;
    }

    let config = MilestoneConfig {
        tiers: milestones,
        applied_count: 0,
        current_fee_bps: 0,
    };
    env.storage()
        .persistent()
        .set(&storage_key(key_id), &config);
    extend_key_ttl_to_full_window(env, &storage_key(key_id));

    Ok(())
}

/// Re-evaluate the active milestone tier for `key_id` after a supply change.
///
/// Intended to be called post-swap. Finds the highest tier whose supply
/// threshold is at or below the current circulating supply and applies its fee
/// when it is lower than the fee currently in effect. The fee only ever moves
/// downward, so a supply drop below a milestone never triggers a regression to
/// a higher fee.
///
/// Returns `(tier, fee_bps)` where `tier` is the 1-based count of tiers reached
/// (`0` when none) and `fee_bps` is the fee currently in effect.
///
/// A [`MilestoneReachedEvent`](crate::events::MilestoneReachedEvent) is emitted
/// whenever a newly reached tier lowers the fee in effect.
pub fn check_and_apply_milestone(env: &Env, key_id: &Address) -> Result<[u32; 2], ContractError> {
    let supply = read_creator_supply(env, key_id);
    let mut config: MilestoneConfig = match env.storage().persistent().get(&storage_key(key_id)) {
        Some(config) => config,
        None => return Ok([0, 0]),
    };

    if config.tiers.is_empty() || supply == 0 {
        return Ok([config.applied_count, config.current_fee_bps]);
    }

    // Find the highest tier whose threshold has been reached (tiers are ascending).
    let mut reached = 0u32;
    for (i, tier) in config.tiers.iter().enumerate() {
        if tier.supply <= supply {
            reached = i as u32 + 1;
        } else {
            break;
        }
    }

    if reached <= config.applied_count {
        // No newly reached tier (also covers a supply drop below a milestone).
        return Ok([config.applied_count, config.current_fee_bps]);
    }

    // Apply newly reached tiers in ascending order. The fee only ever drops,
    // so a tier that does not lower the fee is skipped (no regression to a
    // higher fee). When no fee is active yet, the first reached tier
    // establishes it.
    let mut effective_fee = config.current_fee_bps;
    let mut fee_established = config.current_fee_bps != 0;
    for i in config.applied_count..reached {
        let tier = config.tiers.get(i).ok_or(ContractError::Overflow)?;
        if !fee_established || tier.fee_bps < effective_fee {
            effective_fee = tier.fee_bps;
            fee_established = true;
            let ledger = env.ledger().sequence();
            crate::events::publish_milestone_reached(
                env,
                key_id,
                i + 1,
                supply,
                effective_fee,
                ledger,
            );
        }
    }

    config.applied_count = reached;
    config.current_fee_bps = effective_fee;
    env.storage()
        .persistent()
        .set(&storage_key(key_id), &config);
    extend_key_ttl_to_full_window(env, &storage_key(key_id));

    Ok([reached, effective_fee])
}

/// Read-only view: the active milestone tier and fee for `key_id`.
///
/// Returns `(tier, fee_bps)` where `tier` is the 1-based count of tiers reached
/// (`0` when none or no config exists) and `fee_bps` is the fee currently in
/// effect. Returns [`ContractError::NotRegistered`] when the key has not been
/// registered.
pub fn get_current_milestone(env: &Env, key_id: &Address) -> Result<(u32, u32), ContractError> {
    read_registered_creator_profile(env, key_id)?;
    let config: MilestoneConfig = env
        .storage()
        .persistent()
        .get(&storage_key(key_id))
        .unwrap_or_else(|| MilestoneConfig {
            tiers: Vec::new(env),
            applied_count: 0,
            current_fee_bps: 0,
        });
    Ok((config.applied_count, config.current_fee_bps))
}

/// Configure a graduated bonding curve for a key.
///
/// Each milestone is a (supply_threshold, exponent) pair. Below the first
/// milestone the base exponent is used; at or above each threshold the
/// corresponding exponent applies.
///
/// # Access
/// Callable only by the key creator.
///
/// # Validation
/// - `key_id` must be a registered key, otherwise [`ContractError::NotRegistered`].
/// - `milestones` must contain `1..=MAX_MILESTONE_TIERS` pairs.
/// - Thresholds must be in strictly ascending order and nonzero, otherwise
///   [`ContractError::NotPositiveAmount`].
/// - Exponents must be in `1..=MAX_CURVE_EXPONENT`, otherwise
///   [`ContractError::InvalidCurveConfig`].
/// - No keys may have been sold yet, otherwise
///   [`ContractError::CurveActive`].
///
/// Emits a `graduated_curve_configured` event with all milestone pairs.
pub fn configure_graduated_curve(
    env: &Env,
    key_id: &Address,
    creator: &Address,
    milestones: Vec<(u32, u32)>,
) -> Result<(), ContractError> {
    creator.require_auth();
    let profile = read_registered_creator_profile(env, key_id)?;
    if profile.creator != *creator {
        return Err(ContractError::Unauthorized);
    }

    if read_creator_supply(env, key_id) != 0 {
        return Err(ContractError::CurveActive);
    }

    let count = milestones.len();
    if count == 0 || count > MAX_MILESTONE_TIERS {
        return Err(ContractError::NotPositiveAmount);
    }

    let mut previous_supply = 0u32;
    let mut stored: Vec<CurveMilestone> = Vec::new(env);
    for (supply, exponent) in milestones.iter() {
        if supply == 0 || supply <= previous_supply {
            return Err(ContractError::NotPositiveAmount);
        }
        if !(MIN_CURVE_EXPONENT..=MAX_CURVE_EXPONENT).contains(&exponent) {
            return Err(ContractError::InvalidCurveConfig);
        }
        previous_supply = supply;
        stored.push_back(CurveMilestone { supply, exponent });
    }

    env.storage().persistent().set(
        &curve_storage_key(key_id),
        &GraduatedCurveConfig { milestones: stored },
    );
    extend_key_ttl_to_full_window(env, &curve_storage_key(key_id));

    crate::events::publish_graduated_curve_configured(env, key_id, &milestones);

    Ok(())
}
