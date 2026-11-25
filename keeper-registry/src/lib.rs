//! Bonded keeper registry with performance tracking, slashing and rewards.
//!
//! Keepers lock a minimum bond before they may act during the exclusive
//! keeper-only window. After that window selected entrypoints fall back to
//! permissionless callers. Misbehaviour proven by evidence can slash a keeper,
//! and protocol fees funded into the contract are shared pro-rata to successful
//! actions each reward period.

#![no_std]
// Soroban's `contractimpl` generates client/args helpers that inherit the
// argument count of configuration entrypoints.
#![allow(clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, BytesN, Env, Symbol, Vec,
};

#[cfg(test)]
mod test;

/// `keeper_registered` event topics: `(name, keeper)`.
pub const KEEPER_REGISTERED_EVENT: &str = "keeper_registered";
/// `keeper_slashed` event topics: `(name, keeper)`.
pub const KEEPER_SLASHED_EVENT: &str = "keeper_slashed";
/// `keeper_rewarded` event topics: `(name, keeper)`.
pub const KEEPER_REWARDED_EVENT: &str = "keeper_rewarded";

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum KeeperRegistryError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    BondTooLow = 5,
    AlreadyRegistered = 6,
    NotRegistered = 7,
    UnbondingInProgress = 8,
    UnbondingNotReady = 9,
    InsufficientBond = 10,
    NoRewardPool = 11,
    NoSuccessfulActions = 12,
    AlreadyRewarded = 13,
    ArithmeticOverflow = 14,
    InvalidConfiguration = 15,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    Governor,
    BondToken,
    Treasury,
    MinBond,
    UnbondingDelay,
    KeeperOnly,
    ExclusiveUntil,
    RewardPeriod,
    RewardPool,
    PeriodSuccesses,
    KeeperList,
    Keeper(Address),
    Rewarded(Address, u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeeperRecord {
    pub bond: i128,
    pub slashed: i128,
    pub successful_actions: u64,
    pub failed_submissions: u64,
    pub period_successes: u64,
    pub last_period: u64,
    pub registered_at: u32,
    /// Ledger at which the keeper stops being active. `None` while active.
    pub deregister_at: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeeperStats {
    pub successful_actions: u64,
    pub failed_submissions: u64,
    pub slashed_amount: i128,
    pub active_bond: i128,
    pub deregister_at: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeeperRegisteredEvent {
    pub bond: i128,
    pub registered_at: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeeperSlashedEvent {
    pub amount: i128,
    pub evidence_hash: BytesN<32>,
    pub removed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeeperRewardedEvent {
    pub period: u64,
    pub amount: i128,
    pub successful_actions: u64,
}

#[contract]
pub struct KeeperRegistryContract;

#[contractimpl]
impl KeeperRegistryContract {
    /// Configures the registry. `exclusive_delay` is counted from the
    /// initialization ledger; after it, protected entrypoints accept anyone.
    pub fn initialize(
        env: Env,
        admin: Address,
        governor: Address,
        bond_token: Address,
        treasury: Address,
        min_bond: i128,
        unbonding_delay: u32,
        exclusive_delay: u32,
        keeper_only: bool,
    ) -> Result<(), KeeperRegistryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(KeeperRegistryError::AlreadyInitialized);
        }
        if min_bond <= 0 {
            return Err(KeeperRegistryError::InvalidConfiguration);
        }
        admin.require_auth();

        let now = env.ledger().sequence();
        let instance = env.storage().instance();
        instance.set(&DataKey::Admin, &admin);
        instance.set(&DataKey::Governor, &governor);
        instance.set(&DataKey::BondToken, &bond_token);
        instance.set(&DataKey::Treasury, &treasury);
        instance.set(&DataKey::MinBond, &min_bond);
        instance.set(&DataKey::UnbondingDelay, &unbonding_delay);
        instance.set(&DataKey::KeeperOnly, &keeper_only);
        instance.set(
            &DataKey::ExclusiveUntil,
            &now.saturating_add(if keeper_only { exclusive_delay } else { 0 }),
        );
        instance.set(&DataKey::RewardPool, &0i128);
        instance.set(&DataKey::PeriodSuccesses, &0u64);
        instance.set(&DataKey::RewardPeriod, &0u64);
        instance.set(&DataKey::KeeperList, &Vec::<Address>::new(&env));
        Ok(())
    }

    /// Locks `bond` tokens and adds `keeper` to the active registry.
    pub fn register_keeper(
        env: Env,
        keeper: Address,
        bond: i128,
    ) -> Result<(), KeeperRegistryError> {
        Self::require_initialized(&env)?;
        keeper.require_auth();
        if bond <= 0 {
            return Err(KeeperRegistryError::InvalidAmount);
        }
        let min_bond: i128 = env.storage().instance().get(&DataKey::MinBond).unwrap();
        if bond < min_bond {
            return Err(KeeperRegistryError::BondTooLow);
        }
        let key = DataKey::Keeper(keeper.clone());
        if env.storage().persistent().has(&key) {
            return Err(KeeperRegistryError::AlreadyRegistered);
        }

        let token_address: Address = env.storage().instance().get(&DataKey::BondToken).unwrap();
        token::Client::new(&env, &token_address).transfer(
            &keeper,
            &env.current_contract_address(),
            &bond,
        );

        let now = env.ledger().sequence();
        let record = KeeperRecord {
            bond,
            slashed: 0,
            successful_actions: 0,
            failed_submissions: 0,
            period_successes: 0,
            last_period: Self::current_period(&env),
            registered_at: now,
            deregister_at: None,
        };
        env.storage().persistent().set(&key, &record);
        let mut keepers: Vec<Address> = env.storage().instance().get(&DataKey::KeeperList).unwrap();
        keepers.push_back(keeper.clone());
        env.storage().instance().set(&DataKey::KeeperList, &keepers);

        env.events().publish(
            (Symbol::new(&env, KEEPER_REGISTERED_EVENT), keeper),
            KeeperRegisteredEvent {
                bond,
                registered_at: now,
            },
        );
        Ok(())
    }

    /// Starts the unbonding delay. The keeper remains active until
    /// `finalize_deregistration` is called after the delay elapses.
    pub fn deregister_keeper(env: Env, keeper: Address) -> Result<(), KeeperRegistryError> {
        Self::require_initialized(&env)?;
        keeper.require_auth();
        let key = DataKey::Keeper(keeper.clone());
        let mut record: KeeperRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(KeeperRegistryError::NotRegistered)?;
        if record.deregister_at.is_some() {
            return Err(KeeperRegistryError::UnbondingInProgress);
        }
        let delay: u32 = env
            .storage()
            .instance()
            .get(&DataKey::UnbondingDelay)
            .unwrap();
        record.deregister_at = Some(env.ledger().sequence().saturating_add(delay));
        env.storage().persistent().set(&key, &record);
        Ok(())
    }

    /// Releases the remaining bond once the unbonding delay has elapsed.
    pub fn finalize_deregistration(env: Env, keeper: Address) -> Result<i128, KeeperRegistryError> {
        Self::require_initialized(&env)?;
        keeper.require_auth();
        let key = DataKey::Keeper(keeper.clone());
        let record: KeeperRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(KeeperRegistryError::NotRegistered)?;
        let ready_at = record
            .deregister_at
            .ok_or(KeeperRegistryError::UnbondingInProgress)?;
        if env.ledger().sequence() < ready_at {
            return Err(KeeperRegistryError::UnbondingNotReady);
        }
        let remaining = record.bond - record.slashed;
        let token_address: Address = env.storage().instance().get(&DataKey::BondToken).unwrap();
        if remaining > 0 {
            token::Client::new(&env, &token_address).transfer(
                &env.current_contract_address(),
                &keeper,
                &remaining,
            );
        }
        env.storage().persistent().remove(&key);
        let mut keepers: Vec<Address> = env.storage().instance().get(&DataKey::KeeperList).unwrap();
        if let Some(index) = keepers.iter().position(|entry| entry == keeper) {
            keepers.remove(index as u32);
        }
        env.storage().instance().set(&DataKey::KeeperList, &keepers);
        Ok(remaining)
    }

    /// View of a keeper's recorded performance. Unknown keepers return zeros.
    pub fn keeper_stats(env: Env, keeper: Address) -> KeeperStats {
        let record: Option<KeeperRecord> = env.storage().persistent().get(&DataKey::Keeper(keeper));
        match record {
            Some(r) => KeeperStats {
                successful_actions: r.successful_actions,
                failed_submissions: r.failed_submissions,
                slashed_amount: r.slashed,
                active_bond: r.bond - r.slashed,
                deregister_at: r.deregister_at,
            },
            None => KeeperStats {
                successful_actions: 0,
                failed_submissions: 0,
                slashed_amount: 0,
                active_bond: 0,
                deregister_at: None,
            },
        }
    }

    /// Representative keeper-only entrypoint. During the exclusive window only
    /// active bonded keepers may call it; afterwards it is permissionless.
    pub fn record_action(
        env: Env,
        caller: Address,
        success: bool,
    ) -> Result<(), KeeperRegistryError> {
        Self::require_initialized(&env)?;
        let exclusive_until: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ExclusiveUntil)
            .unwrap();
        let keeper_only: bool = env.storage().instance().get(&DataKey::KeeperOnly).unwrap();
        if keeper_only && env.ledger().sequence() < exclusive_until {
            Self::require_active_keeper(&env, &caller)?;
        }

        let key = DataKey::Keeper(caller.clone());
        if let Some(mut record) = env.storage().persistent().get::<_, KeeperRecord>(&key) {
            let period = Self::current_period(&env);
            if record.last_period != period {
                record.period_successes = 0;
                record.last_period = period;
            }
            if success {
                record.successful_actions = record
                    .successful_actions
                    .checked_add(1)
                    .ok_or(KeeperRegistryError::ArithmeticOverflow)?;
                record.period_successes = record
                    .period_successes
                    .checked_add(1)
                    .ok_or(KeeperRegistryError::ArithmeticOverflow)?;
                let total: u64 = env
                    .storage()
                    .instance()
                    .get(&DataKey::PeriodSuccesses)
                    .unwrap();
                env.storage().instance().set(
                    &DataKey::PeriodSuccesses,
                    &total
                        .checked_add(1)
                        .ok_or(KeeperRegistryError::ArithmeticOverflow)?,
                );
            } else {
                record.failed_submissions = record
                    .failed_submissions
                    .checked_add(1)
                    .ok_or(KeeperRegistryError::ArithmeticOverflow)?;
            }
            env.storage().persistent().set(&key, &record);
        }
        Ok(())
    }

    /// Slashes up to a keeper's remaining bond for provable misbehaviour.
    /// Keepers whose remaining bond falls below the minimum are removed.
    pub fn slash_keeper(
        env: Env,
        governor: Address,
        keeper: Address,
        amount: i128,
        evidence_hash: BytesN<32>,
    ) -> Result<(), KeeperRegistryError> {
        Self::require_initialized(&env)?;
        governor.require_auth();
        let configured: Address = env.storage().instance().get(&DataKey::Governor).unwrap();
        if governor != configured {
            return Err(KeeperRegistryError::Unauthorized);
        }
        if amount <= 0 {
            return Err(KeeperRegistryError::InvalidAmount);
        }

        let key = DataKey::Keeper(keeper.clone());
        let mut record: KeeperRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(KeeperRegistryError::NotRegistered)?;
        let remaining = record
            .bond
            .checked_sub(record.slashed)
            .ok_or(KeeperRegistryError::ArithmeticOverflow)?;
        if amount > remaining {
            return Err(KeeperRegistryError::InsufficientBond);
        }
        record.slashed = record
            .slashed
            .checked_add(amount)
            .ok_or(KeeperRegistryError::ArithmeticOverflow)?;

        let min_bond: i128 = env.storage().instance().get(&DataKey::MinBond).unwrap();
        let removed = record.bond - record.slashed < min_bond;
        if removed && record.deregister_at.is_none() {
            record.deregister_at = Some(env.ledger().sequence());
        }
        env.storage().persistent().set(&key, &record);

        let token_address: Address = env.storage().instance().get(&DataKey::BondToken).unwrap();
        let treasury: Address = env.storage().instance().get(&DataKey::Treasury).unwrap();
        token::Client::new(&env, &token_address).transfer(
            &env.current_contract_address(),
            &treasury,
            &amount,
        );

        env.events().publish(
            (Symbol::new(&env, KEEPER_SLASHED_EVENT), keeper),
            KeeperSlashedEvent {
                amount,
                evidence_hash,
                removed,
            },
        );
        Ok(())
    }

    /// Adds protocol fees to the reward pool for the current period.
    pub fn fund_rewards(
        env: Env,
        funder: Address,
        amount: i128,
    ) -> Result<(), KeeperRegistryError> {
        Self::require_initialized(&env)?;
        funder.require_auth();
        if amount <= 0 {
            return Err(KeeperRegistryError::InvalidAmount);
        }
        let token_address: Address = env.storage().instance().get(&DataKey::BondToken).unwrap();
        token::Client::new(&env, &token_address).transfer(
            &funder,
            &env.current_contract_address(),
            &amount,
        );
        let pool: i128 = env.storage().instance().get(&DataKey::RewardPool).unwrap();
        env.storage()
            .instance()
            .set(&DataKey::RewardPool, &pool.checked_add(amount).unwrap());
        Ok(())
    }

    /// Advances the reward period; successful-action counters reset on the
    /// next action or distribution.
    pub fn advance_period(env: Env, admin: Address) -> Result<u64, KeeperRegistryError> {
        Self::require_initialized(&env)?;
        admin.require_auth();
        let configured: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != configured {
            return Err(KeeperRegistryError::Unauthorized);
        }
        let period = Self::current_period(&env)
            .checked_add(1)
            .ok_or(KeeperRegistryError::ArithmeticOverflow)?;
        env.storage()
            .instance()
            .set(&DataKey::RewardPeriod, &period);
        env.storage()
            .instance()
            .set(&DataKey::PeriodSuccesses, &0u64);
        Ok(period)
    }

    /// Distributes the current reward pool pro-rata to successful actions.
    /// Integer dust remains in the pool for the next period.
    pub fn distribute_rewards(env: Env) -> Result<i128, KeeperRegistryError> {
        Self::require_initialized(&env)?;
        let pool: i128 = env.storage().instance().get(&DataKey::RewardPool).unwrap();
        if pool <= 0 {
            return Err(KeeperRegistryError::NoRewardPool);
        }
        let total: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PeriodSuccesses)
            .unwrap();
        if total == 0 {
            return Err(KeeperRegistryError::NoSuccessfulActions);
        }
        let period = Self::current_period(&env);
        let keepers: Vec<Address> = env.storage().instance().get(&DataKey::KeeperList).unwrap();
        let token_address: Address = env.storage().instance().get(&DataKey::BondToken).unwrap();

        let mut distributed = 0i128;
        for keeper in keepers.iter() {
            let key = DataKey::Keeper(keeper.clone());
            let Some(mut record) = env.storage().persistent().get::<_, KeeperRecord>(&key) else {
                continue;
            };
            if record.last_period != period {
                record.period_successes = 0;
                record.last_period = period;
                env.storage().persistent().set(&key, &record);
            }
            if record.period_successes == 0
                || env
                    .storage()
                    .persistent()
                    .has(&DataKey::Rewarded(keeper.clone(), period))
            {
                continue;
            }
            let share = pool
                .checked_mul(record.period_successes as i128)
                .and_then(|v| v.checked_div(total as i128))
                .ok_or(KeeperRegistryError::ArithmeticOverflow)?;
            if share <= 0 {
                continue;
            }
            token::Client::new(&env, &token_address).transfer(
                &env.current_contract_address(),
                &keeper,
                &share,
            );
            env.storage()
                .persistent()
                .set(&DataKey::Rewarded(keeper.clone(), period), &true);
            distributed = distributed
                .checked_add(share)
                .ok_or(KeeperRegistryError::ArithmeticOverflow)?;
            env.events().publish(
                (Symbol::new(&env, KEEPER_REWARDED_EVENT), keeper),
                KeeperRewardedEvent {
                    period,
                    amount: share,
                    successful_actions: record.period_successes,
                },
            );
        }

        env.storage()
            .instance()
            .set(&DataKey::RewardPool, &(pool - distributed));
        Ok(distributed)
    }

    /// Enables or disables the keeper-only exclusive window.
    pub fn set_keeper_only(
        env: Env,
        admin: Address,
        enabled: bool,
        exclusive_delay: u32,
    ) -> Result<(), KeeperRegistryError> {
        Self::require_initialized(&env)?;
        admin.require_auth();
        let configured: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != configured {
            return Err(KeeperRegistryError::Unauthorized);
        }
        let until =
            env.ledger()
                .sequence()
                .saturating_add(if enabled { exclusive_delay } else { 0 });
        env.storage().instance().set(&DataKey::KeeperOnly, &enabled);
        env.storage()
            .instance()
            .set(&DataKey::ExclusiveUntil, &until);
        Ok(())
    }

    fn require_initialized(env: &Env) -> Result<(), KeeperRegistryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            Ok(())
        } else {
            Err(KeeperRegistryError::NotInitialized)
        }
    }

    fn current_period(env: &Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::RewardPeriod)
            .unwrap_or(0)
    }

    fn require_active_keeper(
        env: &Env,
        keeper: &Address,
    ) -> Result<KeeperRecord, KeeperRegistryError> {
        let record: KeeperRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Keeper(keeper.clone()))
            .ok_or(KeeperRegistryError::NotRegistered)?;
        let min_bond: i128 = env.storage().instance().get(&DataKey::MinBond).unwrap();
        if record.bond - record.slashed < min_bond {
            return Err(KeeperRegistryError::NotRegistered);
        }
        if let Some(ready_at) = record.deregister_at {
            if env.ledger().sequence() >= ready_at {
                return Err(KeeperRegistryError::NotRegistered);
            }
        }
        Ok(record)
    }
}
