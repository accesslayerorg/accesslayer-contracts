use crate::{events, read_registered_creator_profile, ContractError};
use soroban_sdk::{contracttype, vec, Address, Env, Vec};

pub const ROLLING_WINDOW_SECS: u64 = 2_592_000;
pub const VOLUME_BUCKET_SECS: u64 = 300;
pub const MAX_VOLUME_BUCKETS: u32 = 289;
pub const DEFAULT_VOLUME_BRACKETS: u32 = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VolumeBracket {
    /// Inclusive lower volume boundary, in the trade amount's raw base units.
    pub min_volume: i128,
    pub fee_bps: u32,
}

/// A discount tier applied to bonding curve fees for high-volume traders.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DiscountTier {
    /// Inclusive lower 30-day volume boundary, in the trade amount's raw base units.
    pub min_volume: i128,
    /// Discount applied to the base fee, in basis points.
    pub discount_bps: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VolumeBucket {
    pub bucket_start: u64,
    pub volume: i128,
}

/// Snapshot of the fee tier currently applying to a key.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ActiveFeeTier {
    /// Zero-based index into the configured volume brackets.
    pub tier_index: u32,
    pub fee_bps: u32,
    /// The key's rolling 24-hour volume the tier was resolved from.
    pub rolling_volume: i128,
    /// Additional volume needed to reach the next tier, or `0` in the top tier.
    pub volume_to_next_tier: i128,
}

/// Snapshot of the discount tier currently applying to a wallet.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ActiveDiscount {
    /// Zero-based index into the configured discount tiers.
    pub tier_index: u32,
    pub discount_bps: u32,
}

#[derive(Clone)]
#[contracttype]
enum DataKey {
    VolumeBrackets,
    KeyVolumeBuckets(Address),
    LastFeeBps(Address),
    DiscountTiers,
    WalletVolumeBuckets(Address),
    LastDiscountBps(Address),
}

pub fn default_volume_brackets(env: &Env) -> Vec<VolumeBracket> {
    vec![
        env,
        VolumeBracket {
            min_volume: 0,
            fee_bps: 150,
        },
        VolumeBracket {
            min_volume: 100,
            fee_bps: 100,
        },
        VolumeBracket {
            min_volume: 1_001,
            fee_bps: 75,
        },
    ]
}

pub fn default_discount_tiers(env: &Env) -> Vec<DiscountTier> {
    vec![
        env,
        DiscountTier {
            min_volume: 0,
            discount_bps: 0,
        },
        DiscountTier {
            min_volume: 1_000,
            discount_bps: 500,
        },
        DiscountTier {
            min_volume: 10_000,
            discount_bps: 1_000,
        },
        DiscountTier {
            min_volume: 100_000,
            discount_bps: 2_000,
        },
    ]
}

pub fn read_volume_brackets(env: &Env) -> Vec<VolumeBracket> {
    env.storage()
        .persistent()
        .get(&DataKey::VolumeBrackets)
        .unwrap_or_else(|| default_volume_brackets(env))
}

pub fn read_discount_tiers(env: &Env) -> Vec<DiscountTier> {
    env.storage()
        .persistent()
        .get(&DataKey::DiscountTiers)
        .unwrap_or_else(|| default_discount_tiers(env))
}

pub fn validate_volume_brackets(brackets: &Vec<VolumeBracket>) -> Result<(), ContractError> {
    if brackets.is_empty() || brackets.len() > crate::MAX_FEE_TIERS {
        return Err(ContractError::InvalidFeeConfig);
    }
    if brackets.get(0).map(|bracket| bracket.min_volume) != Some(0) {
        return Err(ContractError::InvalidFeeConfig);
    }

    let mut previous_minimum = None;
    for bracket in brackets.iter() {
        if bracket.min_volume < 0 || bracket.fee_bps > crate::fee::BPS_MAX {
            return Err(ContractError::InvalidFeeConfig);
        }
        if let Some(previous) = previous_minimum {
            if bracket.min_volume <= previous {
                return Err(ContractError::InvalidFeeConfig);
            }
        }
        previous_minimum = Some(bracket.min_volume);
    }
    Ok(())
}

pub fn validate_discount_tiers(tiers: &Vec<DiscountTier>) -> Result<(), ContractError> {
    if tiers.is_empty() || tiers.len() > crate::MAX_FEE_TIERS {
        return Err(ContractError::InvalidFeeConfig);
    }
    if tiers.get(0).map(|tier| tier.min_volume) != Some(0) {
        return Err(ContractError::InvalidFeeConfig);
    }

    let mut previous_minimum = None;
    for tier in tiers.iter() {
        if tier.min_volume < 0 || tier.discount_bps > crate::fee::BPS_MAX {
            return Err(ContractError::InvalidFeeConfig);
        }
        if let Some(previous) = previous_minimum {
            if tier.min_volume <= previous {
                return Err(ContractError::InvalidFeeConfig);
            }
        }
        previous_minimum = Some(tier.min_volume);
    }
    Ok(())
}

pub fn set_volume_brackets(env: &Env, brackets: &Vec<VolumeBracket>) {
    let key = DataKey::VolumeBrackets;
    env.storage().persistent().set(&key, brackets);
    crate::extend_key_ttl_to_full_window(env, &key);
}

pub fn set_discount_tiers(env: &Env, tiers: &Vec<DiscountTier>) {
    let key = DataKey::DiscountTiers;
    env.storage().persistent().set(&key, tiers);
    crate::extend_key_ttl_to_full_window(env, &key);
}

fn read_volume_buckets(env: &Env, key_id: &Address) -> Vec<VolumeBucket> {
    env.storage()
        .persistent()
        .get(&DataKey::KeyVolumeBuckets(key_id.clone()))
        .unwrap_or(Vec::new(env))
}

fn read_wallet_volume_buckets(env: &Env, wallet: &Address) -> Vec<VolumeBucket> {
    env.storage()
        .persistent()
        .get(&DataKey::WalletVolumeBuckets(wallet.clone()))
        .unwrap_or(Vec::new(env))
}

fn rolling_volume_from(env: &Env, buckets: &Vec<VolumeBucket>) -> Result<i128, ContractError> {
    let cutoff = env.ledger().timestamp().saturating_sub(ROLLING_WINDOW_SECS);
    let mut total = 0i128;
    for bucket in buckets.iter() {
        if bucket.bucket_start >= cutoff {
            total = total
                .checked_add(bucket.volume)
                .ok_or(ContractError::Overflow)?;
        }
    }
    Ok(total)
}

pub fn rolling_volume(env: &Env, key_id: &Address) -> Result<i128, ContractError> {
    rolling_volume_from(env, &read_volume_buckets(env, key_id))
}

pub fn wallet_rolling_volume(env: &Env, wallet: &Address) -> Result<i128, ContractError> {
    rolling_volume_from(env, &read_wallet_volume_buckets(env, wallet))
}

/// Resolves the `(tier_index, fee_bps)` whose bracket contains `volume`.
pub fn tier_for_volume(env: &Env, volume: i128) -> (u32, u32) {
    let brackets = read_volume_brackets(env);
    let mut tier = (
        0,
        brackets
            .get(0)
            .map(|bracket| bracket.fee_bps)
            .unwrap_or(150),
    );
    for (index, bracket) in brackets.iter().enumerate() {
        if volume < bracket.min_volume {
            break;
        }
        tier = (index as u32, bracket.fee_bps);
    }
    tier
}

/// Resolves the `(tier_index, discount_bps)` whose tier contains `volume`.
pub fn discount_tier_for_volume(env: &Env, volume: i128) -> (u32, u32) {
    let tiers = read_discount_tiers(env);
    let mut tier = (0, tiers.get(0).map(|tier| tier.discount_bps).unwrap_or(0));
    for (index, tier_entry) in tiers.iter().enumerate() {
        if volume < tier_entry.min_volume {
            break;
        }
        tier = (index as u32, tier_entry.discount_bps);
    }
    tier
}

pub fn fee_for_volume(env: &Env, volume: i128) -> u32 {
    tier_for_volume(env, volume).1
}

pub fn active_fee_tier(env: &Env, key_id: &Address) -> Result<ActiveFeeTier, ContractError> {
    read_registered_creator_profile(env, key_id)?;
    let rolling_volume = rolling_volume(env, key_id)?;
    let (tier_index, fee_bps) = tier_for_volume(env, rolling_volume);
    let volume_to_next_tier = read_volume_brackets(env)
        .get(tier_index + 1)
        .map(|next| next.min_volume - rolling_volume)
        .unwrap_or(0);
    Ok(ActiveFeeTier {
        tier_index,
        fee_bps,
        rolling_volume,
        volume_to_next_tier,
    })
}

pub fn get_discount(env: &Env, wallet: &Address) -> Result<ActiveDiscount, ContractError> {
    let rolling_volume = wallet_rolling_volume(env, wallet)?;
    let (tier_index, discount_bps) = discount_tier_for_volume(env, rolling_volume);
    Ok(ActiveDiscount {
        tier_index,
        discount_bps,
    })
}

pub fn discount_bps_for_wallet(env: &Env, wallet: &Address) -> Result<u32, ContractError> {
    Ok(get_discount(env, wallet)?.discount_bps)
}

pub fn current_fee_bps(env: &Env, key_id: &Address) -> Result<u32, ContractError> {
    read_registered_creator_profile(env, key_id)?;
    Ok(fee_for_volume(env, rolling_volume(env, key_id)?))
}

fn emit_fee_adjusted(
    env: &Env,
    key_id: &Address,
    previous_fee_bps: u32,
    new_fee_bps: u32,
    volume: i128,
) {
    env.events().publish(
        (events::FEE_ADJUSTED_EVENT_NAME, key_id.clone()),
        events::FeeAdjustedEvent {
            key_id: key_id.clone(),
            previous_fee_bps,
            new_fee_bps,
            rolling_volume: volume,
            timestamp: env.ledger().timestamp(),
        },
    );
    env.events().publish(
        (events::KEY_FEE_TIER_CHANGED_EVENT_NAME, key_id.clone()),
        events::KeyFeeTierChangedEvent {
            key_id: key_id.clone(),
            new_tier_index: tier_for_volume(env, volume).0,
            previous_fee_bps,
            new_fee_bps,
            rolling_volume: volume,
            timestamp: env.ledger().timestamp(),
        },
    );
}

fn emit_discount_tier_changed(
    env: &Env,
    wallet: &Address,
    previous_discount_bps: u32,
    new_discount_bps: u32,
    volume: i128,
) {
    env.events().publish(
        (
            events::VOLUME_DISCOUNT_TIER_CHANGED_EVENT_NAME,
            wallet.clone(),
        ),
        events::VolumeDiscountTierChangedEvent {
            wallet: wallet.clone(),
            previous_discount_bps,
            new_discount_bps,
            rolling_volume: volume,
            timestamp: env.ledger().timestamp(),
        },
    );
}

/// Records a completed trade and synchronizes any fee changes observable now.
/// Contract transaction atomicity rolls this write back if the enclosing swap fails.
pub fn record_trade(env: &Env, key_id: &Address, volume: i128) -> Result<(), ContractError> {
    if volume <= 0 {
        return Ok(());
    }
    let buckets = read_volume_buckets(env, key_id);
    let current_volume = rolling_volume_from(env, &buckets)?;
    let current_fee = fee_for_volume(env, current_volume);

    if let Some(previous_fee) = env
        .storage()
        .persistent()
        .get::<DataKey, u32>(&DataKey::LastFeeBps(key_id.clone()))
    {
        if previous_fee != current_fee {
            emit_fee_adjusted(env, key_id, previous_fee, current_fee, current_volume);
        }
    }

    let now = env.ledger().timestamp();
    let cutoff = now.saturating_sub(ROLLING_WINDOW_SECS);
    let bucket_start = now - (now % VOLUME_BUCKET_SECS);
    let mut updated = Vec::new(env);
    let mut bucket_found = false;
    for bucket in buckets.iter() {
        if bucket.bucket_start < cutoff {
            continue;
        }
        if bucket.bucket_start == bucket_start {
            updated.push_back(VolumeBucket {
                bucket_start,
                volume: bucket
                    .volume
                    .checked_add(volume)
                    .ok_or(ContractError::Overflow)?,
            });
            bucket_found = true;
        } else {
            updated.push_back(bucket);
        }
    }
    if !bucket_found {
        updated.push_back(VolumeBucket {
            bucket_start,
            volume,
        });
    }
    if updated.len() > MAX_VOLUME_BUCKETS {
        return Err(ContractError::Overflow);
    }

    let bucket_key = DataKey::KeyVolumeBuckets(key_id.clone());
    env.storage().persistent().set(&bucket_key, &updated);
    crate::extend_key_ttl_to_full_window(env, &bucket_key);
    Ok(())
}
