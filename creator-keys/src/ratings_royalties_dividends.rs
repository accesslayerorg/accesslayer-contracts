#![no_std]
use crate::ContractError;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyRatingSummary {
    pub total_score: u64,
    pub count: u32,
    pub average_score_scaled: u32, // scaled by 100 (e.g. 450 = 4.50)
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendPoolState {
    pub total_distributed: i128,
    pub current_cycle: u32,
    pub last_snapshot_ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VolumeFeeConfig {
    pub threshold_low_volume: i128,
    pub threshold_high_volume: i128,
    pub base_fee_bps: u32,
    pub discounted_fee_bps: u32,
}

#[derive(Clone)]
#[contracttype]
pub enum DevPeterDataKey {
    KeyRating(Address),
    HolderRating(Address, Address),
    CreatorRoyaltyBps(Address),
    DividendPool(Address),
    HolderDividendClaim(Address, Address), // (creator, holder) -> cycle
    VolumeFeeConfig,
}

pub const KEY_RATED_EVENT: Symbol = symbol_short!("rated");
pub const ROYALTY_PAID_EVENT: Symbol = symbol_short!("royalty");
pub const DIVIDEND_DISTRIBUTED_EVENT: Symbol = symbol_short!("div_dist");
pub const DIVIDEND_CLAIMED_EVENT: Symbol = symbol_short!("div_claim");
pub const FEE_TIER_CHANGED_EVENT: Symbol = symbol_short!("fee_tier");

/// #946: Record rating (1-5) from key holder, updating aggregate score atomically.
pub fn record_key_rating(
    env: &Env,
    creator: &Address,
    rater: &Address,
    score: u32,
    rater_balance: u32,
) -> Result<KeyRatingSummary, ContractError> {
    rater.require_auth();

    if score < 1 || score > 5 {
        return Err(ContractError::Unauthorized);
    }
    if rater_balance == 0 {
        return Err(ContractError::InsufficientBalance);
    }

    let summary_key = DevPeterDataKey::KeyRating(creator.clone());
    let rater_key = DevPeterDataKey::HolderRating(creator.clone(), rater.clone());

    let mut summary: KeyRatingSummary =
        env.storage()
            .instance()
            .get(&summary_key)
            .unwrap_or(KeyRatingSummary {
                total_score: 0,
                count: 0,
                average_score_scaled: 0,
            });

    let previous_rating: Option<u32> = env.storage().instance().get(&rater_key);

    if let Some(prev) = previous_rating {
        summary.total_score = summary.total_score.saturating_sub(prev as u64) + (score as u64);
    } else {
        summary.total_score += score as u64;
        summary.count += 1;
    }

    summary.average_score_scaled = if summary.count > 0 {
        ((summary.total_score * 100) / (summary.count as u64)) as u32
    } else {
        0
    };

    env.storage().instance().set(&summary_key, &summary);
    env.storage().instance().set(&rater_key, &score);

    env.events().publish(
        (KEY_RATED_EVENT, creator.clone(), rater.clone()),
        (score, summary.average_score_scaled, summary.count),
    );

    Ok(summary)
}

/// Read aggregate key rating.
pub fn get_key_rating(env: &Env, creator: &Address) -> KeyRatingSummary {
    env.storage()
        .instance()
        .get(&DevPeterDataKey::KeyRating(creator.clone()))
        .unwrap_or(KeyRatingSummary {
            total_score: 0,
            count: 0,
            average_score_scaled: 0,
        })
}

/// #950: Configure creator royalty fee on secondary transfers.
pub fn set_creator_royalty(
    env: &Env,
    creator: &Address,
    royalty_bps: u32,
) -> Result<(), ContractError> {
    creator.require_auth();
    if royalty_bps > 2500 {
        // Max 25% royalty bound
        return Err(ContractError::InvalidFeeConfig);
    }
    env.storage().instance().set(
        &DevPeterDataKey::CreatorRoyaltyBps(creator.clone()),
        &royalty_bps,
    );
    Ok(())
}

/// Get configured creator royalty fee.
pub fn get_creator_royalty(env: &Env, creator: &Address) -> u32 {
    env.storage()
        .instance()
        .get(&DevPeterDataKey::CreatorRoyaltyBps(creator.clone()))
        .unwrap_or(0)
}

/// Calculate and deduct royalty amount from secondary transfer value.
pub fn process_transfer_royalty(
    env: &Env,
    creator: &Address,
    from: &Address,
    transfer_value: i128,
) -> (i128, i128) {
    let bps = get_creator_royalty(env, creator);
    if bps == 0 || transfer_value <= 0 {
        return (transfer_value, 0);
    }
    let royalty_amount = (transfer_value * (bps as i128)) / 10_000;
    let net_value = transfer_value.saturating_sub(royalty_amount);

    env.events().publish(
        (ROYALTY_PAID_EVENT, creator.clone(), from.clone()),
        royalty_amount,
    );

    (net_value, royalty_amount)
}

/// #944: Distribute trading revenue dividends to key holders.
pub fn distribute_holder_dividends(
    env: &Env,
    creator: &Address,
    amount: i128,
) -> Result<u32, ContractError> {
    creator.require_auth();
    if amount <= 0 {
        return Err(ContractError::ZeroDistributionAmount);
    }

    let pool_key = DevPeterDataKey::DividendPool(creator.clone());
    let mut pool: DividendPoolState =
        env.storage()
            .instance()
            .get(&pool_key)
            .unwrap_or(DividendPoolState {
                total_distributed: 0,
                current_cycle: 0,
                last_snapshot_ledger: env.ledger().sequence(),
            });

    pool.total_distributed += amount;
    pool.current_cycle += 1;
    pool.last_snapshot_ledger = env.ledger().sequence();

    env.storage().instance().set(&pool_key, &pool);

    env.events().publish(
        (DIVIDEND_DISTRIBUTED_EVENT, creator.clone()),
        (pool.current_cycle, amount),
    );

    Ok(pool.current_cycle)
}

/// Claim dividend share for current cycle.
pub fn claim_holder_dividend(
    env: &Env,
    creator: &Address,
    holder: &Address,
    holder_supply_share_bps: u32,
) -> Result<i128, ContractError> {
    holder.require_auth();

    let pool_key = DevPeterDataKey::DividendPool(creator.clone());
    let claim_key = DevPeterDataKey::HolderDividendClaim(creator.clone(), holder.clone());

    let pool: DividendPoolState = env
        .storage()
        .instance()
        .get(&pool_key)
        .ok_or(ContractError::NoDividendClaimable)?;

    let last_claimed_cycle: u32 = env.storage().instance().get(&claim_key).unwrap_or(0);
    if last_claimed_cycle >= pool.current_cycle {
        return Err(ContractError::AlreadyClaimed);
    }

    let claim_amount = (pool.total_distributed * (holder_supply_share_bps as i128)) / 10_000;
    if claim_amount <= 0 {
        return Err(ContractError::NoDividendClaimable);
    }

    env.storage()
        .instance()
        .set(&claim_key, &pool.current_cycle);

    env.events().publish(
        (DIVIDEND_CLAIMED_EVENT, creator.clone(), holder.clone()),
        (pool.current_cycle, claim_amount),
    );

    Ok(claim_amount)
}

/// #943: Get dynamic fee percentage based on 24h rolling volume.
pub fn calculate_dynamic_fee(env: &Env, creator: &Address, rolling_24h_volume: i128) -> u32 {
    let config: VolumeFeeConfig = env
        .storage()
        .instance()
        .get(&DevPeterDataKey::VolumeFeeConfig)
        .unwrap_or(VolumeFeeConfig {
            threshold_low_volume: 10_000_0000000,   // 10k XLM
            threshold_high_volume: 100_000_0000000, // 100k XLM
            base_fee_bps: 500,                      // 5%
            discounted_fee_bps: 250,                // 2.5%
        });

    let active_fee = if rolling_24h_volume >= config.threshold_high_volume {
        config.discounted_fee_bps
    } else {
        config.base_fee_bps
    };

    env.events().publish(
        (FEE_TIER_CHANGED_EVENT, creator.clone()),
        (rolling_24h_volume, active_fee),
    );

    active_fee
}
