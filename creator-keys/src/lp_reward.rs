use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol};

pub const SCALE: i128 = 1_000_000_000_000; // 1e12 for fixed-point fee accumulator
pub const BPS_SCALE: i128 = 10_000;

pub const LP_ADDED_EVENT_NAME: Symbol = symbol_short!("LPAdded");
pub const LP_CLAIMED_EVENT_NAME: Symbol = symbol_short!("lp_claim");
pub const LP_REMOVED_EVENT_NAME: Symbol = symbol_short!("lp_rem");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LpRewardError {
    NotPositiveAmount = 1,
    PositionNotFound = 2,
    Unauthorized = 3,
    NoRewardsPending = 4,
    ZeroLiquidityPool = 5,
    PositionAlreadyClosed = 6,
    ArithmeticOverflow = 7,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LpPosition {
    pub lp_id: u64,
    pub provider: Address,
    pub key_id: Address,
    pub contribution: i128,
    pub share: i128, // Proportional share in basis points (0..=10_000)
    pub pending_rewards: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StoredLpPosition {
    pub lp_id: u64,
    pub provider: Address,
    pub key_id: Address,
    pub contribution: i128,
    pub reward_debt: i128,
    pub accumulated_rewards: i128,
    pub is_closed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LpPool {
    pub key_id: Address,
    pub total_liquidity: i128,
    pub acc_reward_per_share: i128,
    pub total_rewards_collected: i128,
    pub total_rewards_claimed: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LPAdded {
    pub lp_id: u64,
    pub key_id: Address,
    pub provider: Address,
    pub amount: i128,
    pub share: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LPRewardClaimed {
    pub lp_id: u64,
    pub provider: Address,
    pub key_id: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LPRemoved {
    pub lp_id: u64,
    pub provider: Address,
    pub key_id: Address,
    pub principal: i128,
    pub rewards: i128,
    pub total_returned: i128,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    NextLpId,
    Position(u64),
    Pool(Address),
    TotalPositions,
}

fn read_next_lp_id(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::NextLpId)
        .unwrap_or(1)
}

fn write_next_lp_id(env: &Env, next_id: u64) {
    env.storage().persistent().set(&DataKey::NextLpId, &next_id);
}

fn read_pool(env: &Env, key_id: &Address) -> LpPool {
    env.storage()
        .persistent()
        .get(&DataKey::Pool(key_id.clone()))
        .unwrap_or(LpPool {
            key_id: key_id.clone(),
            total_liquidity: 0,
            acc_reward_per_share: 0,
            total_rewards_collected: 0,
            total_rewards_claimed: 0,
        })
}

fn write_pool(env: &Env, pool: &LpPool) {
    env.storage()
        .persistent()
        .set(&DataKey::Pool(pool.key_id.clone()), pool);
}

fn read_position(env: &Env, lp_id: u64) -> Result<StoredLpPosition, LpRewardError> {
    env.storage()
        .persistent()
        .get(&DataKey::Position(lp_id))
        .ok_or(LpRewardError::PositionNotFound)
}

fn write_position(env: &Env, position: &StoredLpPosition) {
    env.storage()
        .persistent()
        .set(&DataKey::Position(position.lp_id), position);
}

pub fn add_liquidity(
    env: &Env,
    key_id: Address,
    provider: Address,
    amount: i128,
) -> Result<u64, LpRewardError> {
    provider.require_auth();

    if amount <= 0 {
        return Err(LpRewardError::NotPositiveAmount);
    }

    let mut pool = read_pool(env, &key_id);
    let new_total_liquidity = pool
        .total_liquidity
        .checked_add(amount)
        .ok_or(LpRewardError::ArithmeticOverflow)?;
    pool.total_liquidity = new_total_liquidity;

    let share_bps = (amount
        .checked_mul(BPS_SCALE)
        .ok_or(LpRewardError::ArithmeticOverflow)?)
        / new_total_liquidity;

    let lp_id = read_next_lp_id(env);
    let next_id = lp_id
        .checked_add(1)
        .ok_or(LpRewardError::ArithmeticOverflow)?;
    write_next_lp_id(env, next_id);

    let reward_debt = (amount
        .checked_mul(pool.acc_reward_per_share)
        .ok_or(LpRewardError::ArithmeticOverflow)?)
        / SCALE;

    let position = StoredLpPosition {
        lp_id,
        provider: provider.clone(),
        key_id: key_id.clone(),
        contribution: amount,
        reward_debt,
        accumulated_rewards: 0,
        is_closed: false,
    };

    write_position(env, &position);
    write_pool(env, &pool);

    let total_pos: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::TotalPositions)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::TotalPositions, &(total_pos + 1));

    env.events().publish(
        (LP_ADDED_EVENT_NAME, key_id.clone(), provider.clone()),
        LPAdded {
            lp_id,
            key_id,
            provider,
            amount,
            share: share_bps,
        },
    );

    Ok(lp_id)
}

pub fn accrue_trading_fee(
    env: &Env,
    key_id: Address,
    fee_amount: i128,
) -> Result<(), LpRewardError> {
    if fee_amount <= 0 {
        return Err(LpRewardError::NotPositiveAmount);
    }

    let mut pool = read_pool(env, &key_id);
    if pool.total_liquidity == 0 {
        return Err(LpRewardError::ZeroLiquidityPool);
    }

    let delta = (fee_amount
        .checked_mul(SCALE)
        .ok_or(LpRewardError::ArithmeticOverflow)?)
        / pool.total_liquidity;

    pool.acc_reward_per_share = pool
        .acc_reward_per_share
        .checked_add(delta)
        .ok_or(LpRewardError::ArithmeticOverflow)?;
    pool.total_rewards_collected = pool
        .total_rewards_collected
        .checked_add(fee_amount)
        .ok_or(LpRewardError::ArithmeticOverflow)?;

    write_pool(env, &pool);
    Ok(())
}

pub fn remove_liquidity(env: &Env, lp_id: u64) -> Result<i128, LpRewardError> {
    let mut position = read_position(env, lp_id)?;
    position.provider.require_auth();

    if position.is_closed {
        return Err(LpRewardError::PositionAlreadyClosed);
    }

    let mut pool = read_pool(env, &position.key_id);

    let accumulated = (position
        .contribution
        .checked_mul(pool.acc_reward_per_share)
        .ok_or(LpRewardError::ArithmeticOverflow)?)
        / SCALE;
    let pending = position
        .accumulated_rewards
        .checked_add(accumulated.saturating_sub(position.reward_debt))
        .ok_or(LpRewardError::ArithmeticOverflow)?;

    let principal = position.contribution;
    let total_return = principal
        .checked_add(pending)
        .ok_or(LpRewardError::ArithmeticOverflow)?;

    pool.total_liquidity = pool.total_liquidity.saturating_sub(principal);
    pool.total_rewards_claimed = pool
        .total_rewards_claimed
        .checked_add(pending)
        .ok_or(LpRewardError::ArithmeticOverflow)?;

    position.is_closed = true;
    position.contribution = 0;
    position.accumulated_rewards = 0;
    position.reward_debt = 0;

    write_position(env, &position);
    write_pool(env, &pool);

    if pending > 0 {
        env.events().publish(
            (
                LP_CLAIMED_EVENT_NAME,
                position.key_id.clone(),
                position.provider.clone(),
            ),
            LPRewardClaimed {
                lp_id,
                provider: position.provider.clone(),
                key_id: position.key_id.clone(),
                amount: pending,
            },
        );
    }

    env.events().publish(
        (
            LP_REMOVED_EVENT_NAME,
            position.key_id.clone(),
            position.provider.clone(),
        ),
        LPRemoved {
            lp_id,
            provider: position.provider,
            key_id: position.key_id,
            principal,
            rewards: pending,
            total_returned: total_return,
        },
    );

    Ok(total_return)
}

pub fn claim_lp_rewards(env: &Env, lp_id: u64) -> Result<i128, LpRewardError> {
    let mut position = read_position(env, lp_id)?;
    position.provider.require_auth();

    if position.is_closed {
        return Err(LpRewardError::PositionAlreadyClosed);
    }

    let mut pool = read_pool(env, &position.key_id);

    let accumulated = (position
        .contribution
        .checked_mul(pool.acc_reward_per_share)
        .ok_or(LpRewardError::ArithmeticOverflow)?)
        / SCALE;
    let pending = position
        .accumulated_rewards
        .checked_add(accumulated.saturating_sub(position.reward_debt))
        .ok_or(LpRewardError::ArithmeticOverflow)?;

    if pending == 0 {
        return Ok(0);
    }

    position.accumulated_rewards = 0;
    position.reward_debt = (position
        .contribution
        .checked_mul(pool.acc_reward_per_share)
        .ok_or(LpRewardError::ArithmeticOverflow)?)
        / SCALE;

    pool.total_rewards_claimed = pool
        .total_rewards_claimed
        .checked_add(pending)
        .ok_or(LpRewardError::ArithmeticOverflow)?;

    write_position(env, &position);
    write_pool(env, &pool);

    env.events().publish(
        (
            LP_CLAIMED_EVENT_NAME,
            position.key_id.clone(),
            position.provider.clone(),
        ),
        LPRewardClaimed {
            lp_id,
            provider: position.provider,
            key_id: position.key_id,
            amount: pending,
        },
    );

    Ok(pending)
}

pub fn get_lp_position(env: &Env, lp_id: u64) -> Result<LpPosition, LpRewardError> {
    let position = read_position(env, lp_id)?;
    if position.is_closed {
        return Ok(LpPosition {
            lp_id,
            provider: position.provider,
            key_id: position.key_id,
            contribution: 0,
            share: 0,
            pending_rewards: 0,
        });
    }

    let pool = read_pool(env, &position.key_id);
    let accumulated = (position
        .contribution
        .checked_mul(pool.acc_reward_per_share)
        .unwrap_or(0))
        / SCALE;
    let pending = position
        .accumulated_rewards
        .saturating_add(accumulated.saturating_sub(position.reward_debt));

    let share = if pool.total_liquidity > 0 {
        (position.contribution.saturating_mul(BPS_SCALE)) / pool.total_liquidity
    } else {
        0
    };

    Ok(LpPosition {
        lp_id,
        provider: position.provider,
        key_id: position.key_id,
        contribution: position.contribution,
        share,
        pending_rewards: pending,
    })
}

pub fn get_total_liquidity(env: &Env, key_id: Address) -> i128 {
    read_pool(env, &key_id).total_liquidity
}

pub fn get_pool_rewards(env: &Env, key_id: Address) -> i128 {
    read_pool(env, &key_id).total_rewards_collected
}

pub fn get_lp_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::TotalPositions)
        .unwrap_or(0)
}
