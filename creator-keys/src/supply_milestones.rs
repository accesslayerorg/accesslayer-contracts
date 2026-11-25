//! Graduated supply milestones with fee reduction and curve-slope updates.
//!
//! Tracks bonding-curve graduation thresholds per key and automatically lowers
//! the key's fee once its circulating supply crosses configured milestones.
//! Slope milestones also apply a one-time schedule of per-key curve coefficients.
//! Milestones are applied in ascending supply order and never regress.

use crate::{
    assert_is_admin, extend_key_ttl_to_full_window, read_creator_supply,
    read_registered_creator_profile, ContractError, DataKey, FeatureError, FeatureKey,
};
use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

/// Maximum number of milestone tiers that can be configured for a single key.
pub const MAX_MILESTONE_TIERS: u32 = 5;

/// Maximum fee in basis points (`10_000` bps = 100%).
pub const MAX_FEE_BPS: u32 = 10_000;

/// A single graduated milestone tier: a supply threshold and the fee bps that
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

/// A supply threshold and the curve slope that becomes active at that threshold.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CurveSlopeTier {
    pub supply: u32,
    pub slope: i128,
}

/// Per-key immutable slope schedule and its monotonic activation state.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CurveSlopeConfig {
    pub tiers: Vec<CurveSlopeTier>,
    pub applied_count: u32,
    pub current_slope: Option<i128>,
}

/// Event payload published when a supply threshold activates a curve slope.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SupplyMilestoneUnlockedEvent {
    pub milestone_index: u32,
    pub supply: u32,
    pub new_slope: i128,
}

/// Storage key for a key's milestone configuration.
fn storage_key(key_id: &Address) -> DataKey {
    DataKey::KeyMilestones(key_id.clone())
}

fn curve_slope_storage_key(key_id: &Address) -> FeatureKey {
    FeatureKey::CurveSlopeMilestones(key_id.clone())
}

/// Configure a one-time per-key curve slope schedule.
pub fn set_curve_slope_milestones(
    env: &Env,
    admin: &Address,
    key_id: &Address,
    tiers: Vec<CurveSlopeTier>,
) -> Result<(), FeatureError> {
    admin.require_auth();
    assert_is_admin(env, admin).map_err(|_| FeatureError::Unauthorized)?;
    read_registered_creator_profile(env, key_id).map_err(|_| FeatureError::NotRegistered)?;

    let key = curve_slope_storage_key(key_id);
    if env.storage().persistent().has(&key) {
        return Err(FeatureError::AlreadyConfigured);
    }
    if tiers.is_empty() || tiers.len() > MAX_MILESTONE_TIERS {
        return Err(FeatureError::InvalidCurveMilestones);
    }

    let mut previous_supply = 0;
    for tier in tiers.iter() {
        if tier.supply == 0 || tier.supply <= previous_supply || tier.slope < 0 {
            return Err(FeatureError::InvalidCurveMilestones);
        }
        previous_supply = tier.supply;
    }

    env.storage().persistent().set(
        &key,
        &CurveSlopeConfig {
            tiers,
            applied_count: 0,
            current_slope: None,
        },
    );
    extend_key_ttl_to_full_window(env, &key);
    Ok(())
}

/// Activate each newly crossed slope tier. Multiple thresholds crossed in one
/// mint are applied and emitted once, in ascending threshold order.
pub fn check_and_apply_curve_slope_milestone(
    env: &Env,
    key_id: &Address,
) -> Result<(u32, i128), ContractError> {
    let key = curve_slope_storage_key(key_id);
    let Some(mut config) = env
        .storage()
        .persistent()
        .get::<FeatureKey, CurveSlopeConfig>(&key)
    else {
        return Ok((0, crate::resolve_curve_slope(env, key_id)));
    };
    let supply = read_creator_supply(env, key_id);
    let old_count = config.applied_count;

    while config.applied_count < config.tiers.len() {
        let tier = config.tiers.get(config.applied_count).unwrap();
        if tier.supply > supply {
            break;
        }

        config.applied_count += 1;
        config.current_slope = Some(tier.slope);
        env.events().publish(
            (Symbol::new(env, "SupplyMilestoneUnlocked"), key_id.clone()),
            SupplyMilestoneUnlockedEvent {
                milestone_index: config.applied_count,
                supply,
                new_slope: tier.slope,
            },
        );
    }

    if config.applied_count != old_count {
        env.storage().persistent().set(&key, &config);
        extend_key_ttl_to_full_window(env, &key);
    }

    let slope = config
        .current_slope
        .unwrap_or_else(|| crate::resolve_curve_slope(env, key_id));
    Ok((config.applied_count, slope))
}

/// Return the active one-based tier and effective slope (zero tier before the
/// first threshold or when no schedule is configured).
pub fn get_current_curve_slope_milestone(
    env: &Env,
    key_id: &Address,
) -> Result<(u32, i128), ContractError> {
    read_registered_creator_profile(env, key_id)?;
    let config: Option<CurveSlopeConfig> = env
        .storage()
        .persistent()
        .get(&curve_slope_storage_key(key_id));
    Ok(match config {
        Some(config) => {
            let slope = config
                .current_slope
                .unwrap_or_else(|| crate::resolve_curve_slope(env, key_id));
            (config.applied_count, slope)
        }
        None => (0, crate::resolve_curve_slope(env, key_id)),
    })
}

/// Active milestone slope, when a configured threshold has been crossed.
pub(crate) fn curve_slope_override(env: &Env, key_id: &Address) -> Option<i128> {
    env.storage()
        .persistent()
        .get::<FeatureKey, CurveSlopeConfig>(&curve_slope_storage_key(key_id))
        .and_then(|config| config.current_slope)
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
pub fn check_and_apply_milestone(env: &Env, key_id: &Address) -> Result<(u32, u32), ContractError> {
    let supply = read_creator_supply(env, key_id);
    let mut config: MilestoneConfig = match env.storage().persistent().get(&storage_key(key_id)) {
        Some(config) => config,
        None => return Ok((0, 0)),
    };

    if config.tiers.is_empty() || supply == 0 {
        return Ok((config.applied_count, config.current_fee_bps));
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
        return Ok((config.applied_count, config.current_fee_bps));
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

    Ok((reached, effective_fee))
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
