#![no_std]

//! Epoch-reward staking for creator key holders.
//!
//! Key holders lock an amount for a fixed number of epochs. The protocol fee
//! treasury funds a reward pool per epoch via [`KeyStakingContract::fund_epoch`];
//! each epoch's pool is split pro-rata to the stake weight (`amount`) active in
//! that epoch. Like the other reward crates in this workspace the contract does
//! pure bookkeeping and never moves tokens itself.
//!
//! Epochs are derived from the ledger timestamp: epoch `n` covers
//! `[genesis + n * epoch_duration, genesis + (n + 1) * epoch_duration)`.
//! A stake created in epoch `s` for `k` epochs earns from epochs `s..s + k` and
//! may be unstaked once the current epoch reaches `s + k`.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

#[cfg(test)]
mod test;

/// Upper bound on a single lock, keeping per-stake epoch iteration bounded.
pub const MAX_LOCK_EPOCHS: u32 = 52;

pub const STAKED_EVENT_NAME: Symbol = symbol_short!("staked");
pub const UNSTAKED_EVENT_NAME: Symbol = symbol_short!("unstaked");
pub const EPOCH_FUNDED_EVENT_NAME: Symbol = symbol_short!("ep_fund");
pub const EPOCH_DISTRIBUTED_EVENT_NAME: Symbol = symbol_short!("ep_dist");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum KeyStakingError {
    NotPositiveAmount = 1,
    InvalidEpochs = 2,
    StakeNotFound = 3,
    /// The stake's lock period (or the epoch being distributed) has not ended.
    EpochNotComplete = 4,
    StakeAlreadyClosed = 5,
    AlreadyInitialized = 6,
    NotInitialized = 7,
    /// The epoch being funded has already ended.
    EpochEnded = 8,
    EpochAlreadyDistributed = 9,
    ArithmeticOverflow = 10,
    InvalidEpochDuration = 11,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakePosition {
    pub stake_id: u64,
    pub key_id: Address,
    pub staker: Address,
    pub amount: i128,
    pub start_epoch: u32,
    /// First epoch in which the stake no longer earns; unstake opens here.
    pub end_epoch: u32,
    pub is_closed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EpochState {
    pub funded: i128,
    pub total_weight: i128,
    pub distributed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakeRewards {
    /// Rewards from epochs that have already completed.
    pub accrued: i128,
    /// Rewards from epochs still in progress or upcoming, at current pools and weights.
    pub projected: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Staked {
    pub stake_id: u64,
    pub key_id: Address,
    pub staker: Address,
    pub amount: i128,
    pub start_epoch: u32,
    pub end_epoch: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Unstaked {
    pub stake_id: u64,
    pub staker: Address,
    pub principal: i128,
    pub rewards: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EpochFunded {
    pub epoch_id: u32,
    pub amount: i128,
    pub total_funded: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EpochRewardDistributed {
    pub epoch_id: u32,
    pub total_distributed: i128,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Treasury,
    Genesis,
    EpochDuration,
    NextStakeId,
    Stake(u64),
    Epoch(u32),
}

#[contract]
pub struct KeyStakingContract;

fn read_treasury(env: &Env) -> Result<Address, KeyStakingError> {
    env.storage()
        .persistent()
        .get(&DataKey::Treasury)
        .ok_or(KeyStakingError::NotInitialized)
}

fn current_epoch_of(env: &Env) -> Result<u32, KeyStakingError> {
    let genesis: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::Genesis)
        .ok_or(KeyStakingError::NotInitialized)?;
    let duration: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::EpochDuration)
        .ok_or(KeyStakingError::NotInitialized)?;
    let elapsed = env.ledger().timestamp().saturating_sub(genesis);
    u32::try_from(elapsed / duration).map_err(|_| KeyStakingError::ArithmeticOverflow)
}

fn read_epoch(env: &Env, epoch_id: u32) -> EpochState {
    env.storage()
        .persistent()
        .get(&DataKey::Epoch(epoch_id))
        .unwrap_or(EpochState {
            funded: 0,
            total_weight: 0,
            distributed: false,
        })
}

fn write_epoch(env: &Env, epoch_id: u32, state: &EpochState) {
    env.storage()
        .persistent()
        .set(&DataKey::Epoch(epoch_id), state);
}

fn read_stake(env: &Env, stake_id: u64) -> Result<StakePosition, KeyStakingError> {
    env.storage()
        .persistent()
        .get(&DataKey::Stake(stake_id))
        .ok_or(KeyStakingError::StakeNotFound)
}

fn write_stake(env: &Env, stake: &StakePosition) {
    env.storage()
        .persistent()
        .set(&DataKey::Stake(stake.stake_id), stake);
}

/// Pro-rata share of an epoch's pool for `amount` of weight (floor division).
fn epoch_share(epoch: &EpochState, amount: i128) -> Result<i128, KeyStakingError> {
    if epoch.total_weight == 0 {
        return Ok(0);
    }
    Ok(epoch
        .funded
        .checked_mul(amount)
        .ok_or(KeyStakingError::ArithmeticOverflow)?
        / epoch.total_weight)
}

/// Splits a stake's rewards into accrued and projected at the `current` epoch.
fn compute_rewards(
    env: &Env,
    stake: &StakePosition,
    current: u32,
) -> Result<StakeRewards, KeyStakingError> {
    let mut accrued: i128 = 0;
    let mut projected: i128 = 0;
    for epoch_id in stake.start_epoch..stake.end_epoch {
        let share = epoch_share(&read_epoch(env, epoch_id), stake.amount)?;
        let bucket = if epoch_id < current {
            &mut accrued
        } else {
            &mut projected
        };
        *bucket = bucket
            .checked_add(share)
            .ok_or(KeyStakingError::ArithmeticOverflow)?;
    }
    Ok(StakeRewards { accrued, projected })
}

#[contractimpl]
impl KeyStakingContract {
    /// One-time setup. `treasury` is the protocol fee treasury contract, the
    /// only address allowed to fund epochs. Epoch 0 starts at the current
    /// ledger timestamp.
    pub fn initialize(
        env: Env,
        treasury: Address,
        epoch_duration: u64,
    ) -> Result<(), KeyStakingError> {
        if env.storage().persistent().has(&DataKey::Treasury) {
            return Err(KeyStakingError::AlreadyInitialized);
        }
        if epoch_duration == 0 {
            return Err(KeyStakingError::InvalidEpochDuration);
        }
        let store = env.storage().persistent();
        store.set(&DataKey::Treasury, &treasury);
        store.set(&DataKey::Genesis, &env.ledger().timestamp());
        store.set(&DataKey::EpochDuration, &epoch_duration);
        Ok(())
    }

    /// Locks `amount` for `epochs` epochs starting with the current epoch.
    /// Returns the new `stake_id`.
    pub fn stake(
        env: Env,
        key_id: Address,
        staker: Address,
        amount: i128,
        epochs: u32,
    ) -> Result<u64, KeyStakingError> {
        staker.require_auth();

        if amount <= 0 {
            return Err(KeyStakingError::NotPositiveAmount);
        }
        if epochs == 0 || epochs > MAX_LOCK_EPOCHS {
            return Err(KeyStakingError::InvalidEpochs);
        }

        let start_epoch = current_epoch_of(&env)?;
        let end_epoch = start_epoch
            .checked_add(epochs)
            .ok_or(KeyStakingError::ArithmeticOverflow)?;

        for epoch_id in start_epoch..end_epoch {
            let mut state = read_epoch(&env, epoch_id);
            state.total_weight = state
                .total_weight
                .checked_add(amount)
                .ok_or(KeyStakingError::ArithmeticOverflow)?;
            write_epoch(&env, epoch_id, &state);
        }

        let stake_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::NextStakeId)
            .unwrap_or(1);
        env.storage().persistent().set(
            &DataKey::NextStakeId,
            &stake_id
                .checked_add(1)
                .ok_or(KeyStakingError::ArithmeticOverflow)?,
        );

        write_stake(
            &env,
            &StakePosition {
                stake_id,
                key_id: key_id.clone(),
                staker: staker.clone(),
                amount,
                start_epoch,
                end_epoch,
                is_closed: false,
            },
        );

        env.events().publish(
            (STAKED_EVENT_NAME, key_id.clone(), staker.clone()),
            Staked {
                stake_id,
                key_id,
                staker,
                amount,
                start_epoch,
                end_epoch,
            },
        );

        Ok(stake_id)
    }

    /// Returns principal plus all epoch rewards once the lock has completed.
    pub fn unstake(env: Env, stake_id: u64) -> Result<i128, KeyStakingError> {
        let mut stake = read_stake(&env, stake_id)?;
        stake.staker.require_auth();

        if stake.is_closed {
            return Err(KeyStakingError::StakeAlreadyClosed);
        }
        let current = current_epoch_of(&env)?;
        if current < stake.end_epoch {
            return Err(KeyStakingError::EpochNotComplete);
        }

        let rewards = compute_rewards(&env, &stake, current)?.accrued;
        let principal = stake.amount;
        let total_return = principal
            .checked_add(rewards)
            .ok_or(KeyStakingError::ArithmeticOverflow)?;

        stake.is_closed = true;
        write_stake(&env, &stake);

        env.events().publish(
            (UNSTAKED_EVENT_NAME, stake.key_id, stake.staker.clone()),
            Unstaked {
                stake_id,
                staker: stake.staker,
                principal,
                rewards,
            },
        );

        Ok(total_return)
    }

    /// Adds `amount` to the reward pool of `epoch_id` (current or future).
    /// Restricted to the protocol fee treasury contract.
    pub fn fund_epoch(env: Env, epoch_id: u32, amount: i128) -> Result<(), KeyStakingError> {
        read_treasury(&env)?.require_auth();

        if amount <= 0 {
            return Err(KeyStakingError::NotPositiveAmount);
        }
        if epoch_id < current_epoch_of(&env)? {
            return Err(KeyStakingError::EpochEnded);
        }

        let mut state = read_epoch(&env, epoch_id);
        state.funded = state
            .funded
            .checked_add(amount)
            .ok_or(KeyStakingError::ArithmeticOverflow)?;
        write_epoch(&env, epoch_id, &state);

        env.events().publish(
            (EPOCH_FUNDED_EVENT_NAME, epoch_id),
            EpochFunded {
                epoch_id,
                amount,
                total_funded: state.funded,
            },
        );
        Ok(())
    }

    /// Finalises a completed epoch and emits `EpochRewardDistributed`.
    /// Permissionless. An epoch with no stake weight distributes nothing;
    /// otherwise the whole pool is allocated pro-rata (floor-division dust,
    /// under one unit per stake, stays unclaimed).
    pub fn distribute_epoch(env: Env, epoch_id: u32) -> Result<i128, KeyStakingError> {
        if epoch_id >= current_epoch_of(&env)? {
            return Err(KeyStakingError::EpochNotComplete);
        }
        let mut state = read_epoch(&env, epoch_id);
        if state.distributed {
            return Err(KeyStakingError::EpochAlreadyDistributed);
        }

        let total_distributed = if state.total_weight > 0 {
            state.funded
        } else {
            0
        };
        state.distributed = true;
        write_epoch(&env, epoch_id, &state);

        env.events().publish(
            (EPOCH_DISTRIBUTED_EVENT_NAME, epoch_id),
            EpochRewardDistributed {
                epoch_id,
                total_distributed,
            },
        );
        Ok(total_distributed)
    }

    /// Accrued (completed epochs) and projected (remaining epochs) rewards.
    /// A closed stake reports zero for both.
    pub fn get_stake_rewards(env: Env, stake_id: u64) -> Result<StakeRewards, KeyStakingError> {
        let stake = read_stake(&env, stake_id)?;
        if stake.is_closed {
            return Ok(StakeRewards {
                accrued: 0,
                projected: 0,
            });
        }
        compute_rewards(&env, &stake, current_epoch_of(&env)?)
    }

    pub fn get_stake(env: Env, stake_id: u64) -> Result<StakePosition, KeyStakingError> {
        read_stake(&env, stake_id)
    }

    pub fn get_epoch(env: Env, epoch_id: u32) -> EpochState {
        read_epoch(&env, epoch_id)
    }

    pub fn current_epoch(env: Env) -> Result<u32, KeyStakingError> {
        current_epoch_of(&env)
    }
}
