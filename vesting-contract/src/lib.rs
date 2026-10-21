#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

#[cfg(test)]
mod test;

/// Event name for vesting schedule creation.
pub const VEST_CREATED_EVENT_NAME: Symbol = symbol_short!("vest_new");

/// Event name for claiming vested tokens.
pub const VEST_CLAIMED_EVENT_NAME: Symbol = symbol_short!("vest_clm");

/// Default persistent storage TTL thresholds (~30 days).
pub const VEST_TTL_THRESHOLD: u32 = 100_000;
pub const VEST_TTL_EXTEND_TO: u32 = 518_400;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    VestNotFound = 4,
    CliffNotReached = 5,
    NotPositiveAmount = 6,
    InvalidDuration = 7,
    InvalidCliff = 8,
    NoVestedTokens = 9,
    ArithmeticOverflow = 10,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestSchedule {
    pub vest_id: u64,
    pub wallet: Address,
    pub key_id: Address,
    pub amount: i128,
    pub claimed_amount: i128,
    pub start_timestamp: u64,
    pub cliff_seconds: u64,
    pub duration_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestedAmount {
    pub total: i128,
    pub claimed: i128,
    pub claimable: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestCreated {
    pub vest_id: u64,
    pub wallet: Address,
    pub key_id: Address,
    pub amount: i128,
    pub cliff_seconds: u64,
    pub duration_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestClaimed {
    pub vest_id: u64,
    pub wallet: Address,
    pub key_id: Address,
    pub amount: i128,
    pub remaining_locked: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    Admin,
    NextVestId,
    Vest(u64),
}

#[contract]
pub struct VestingContract;

fn calculate_unlocked_amount(
    schedule: &VestSchedule,
    current_timestamp: u64,
) -> Result<i128, ContractError> {
    let cliff_time = schedule
        .start_timestamp
        .checked_add(schedule.cliff_seconds)
        .ok_or(ContractError::ArithmeticOverflow)?;

    if current_timestamp < cliff_time {
        return Ok(0);
    }

    let end_time = schedule
        .start_timestamp
        .checked_add(schedule.duration_seconds)
        .ok_or(ContractError::ArithmeticOverflow)?;

    if current_timestamp >= end_time {
        return Ok(schedule.amount);
    }

    let elapsed = current_timestamp.saturating_sub(schedule.start_timestamp);
    let unlocked = (schedule
        .amount
        .checked_mul(elapsed as i128)
        .ok_or(ContractError::ArithmeticOverflow)?)
        / (schedule.duration_seconds as i128);

    Ok(unlocked)
}

#[contractimpl]
impl VestingContract {
    /// Initializes the vesting contract with an admin.
    pub fn init(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextVestId, &1u64);
        env.storage()
            .instance()
            .extend_ttl(VEST_TTL_THRESHOLD, VEST_TTL_EXTEND_TO);

        Ok(())
    }

    /// Returns the contract admin address, if initialized.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    /// Updates the admin address. Requires authorization from current admin.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)?;
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.storage()
            .instance()
            .extend_ttl(VEST_TTL_THRESHOLD, VEST_TTL_EXTEND_TO);

        Ok(())
    }

    /// Creates a new vesting schedule for a creator key allocation. Admin-only.
    pub fn create_vest(
        env: Env,
        wallet: Address,
        key_id: Address,
        amount: i128,
        cliff_seconds: u64,
        duration_seconds: u64,
    ) -> Result<u64, ContractError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)?;
        admin.require_auth();

        if amount <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        if duration_seconds == 0 {
            return Err(ContractError::InvalidDuration);
        }
        if cliff_seconds > duration_seconds {
            return Err(ContractError::InvalidCliff);
        }

        let next_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextVestId)
            .unwrap_or(1);

        let new_next_id = next_id
            .checked_add(1)
            .ok_or(ContractError::ArithmeticOverflow)?;
        env.storage()
            .instance()
            .set(&DataKey::NextVestId, &new_next_id);
        env.storage()
            .instance()
            .extend_ttl(VEST_TTL_THRESHOLD, VEST_TTL_EXTEND_TO);

        let now = env.ledger().timestamp();
        let schedule = VestSchedule {
            vest_id: next_id,
            wallet: wallet.clone(),
            key_id: key_id.clone(),
            amount,
            claimed_amount: 0,
            start_timestamp: now,
            cliff_seconds,
            duration_seconds,
        };

        let key = DataKey::Vest(next_id);
        env.storage().persistent().set(&key, &schedule);
        env.storage()
            .persistent()
            .extend_ttl(&key, VEST_TTL_THRESHOLD, VEST_TTL_EXTEND_TO);

        env.events().publish(
            (VEST_CREATED_EVENT_NAME, wallet.clone()),
            VestCreated {
                vest_id: next_id,
                wallet,
                key_id,
                amount,
                cliff_seconds,
                duration_seconds,
            },
        );

        Ok(next_id)
    }

    /// Returns the full vesting schedule for a vest ID.
    pub fn get_vest(env: Env, vest_id: u64) -> Result<VestSchedule, ContractError> {
        let key = DataKey::Vest(vest_id);
        let schedule: VestSchedule = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::VestNotFound)?;

        env.storage()
            .persistent()
            .extend_ttl(&key, VEST_TTL_THRESHOLD, VEST_TTL_EXTEND_TO);

        Ok(schedule)
    }

    /// Returns total, claimed, and currently claimable amounts for a vest ID.
    pub fn get_vested_amount(env: Env, vest_id: u64) -> Result<VestedAmount, ContractError> {
        let schedule = Self::get_vest(env.clone(), vest_id)?;
        let now = env.ledger().timestamp();
        let unlocked = calculate_unlocked_amount(&schedule, now)?;
        let claimable = unlocked.saturating_sub(schedule.claimed_amount);

        Ok(VestedAmount {
            total: schedule.amount,
            claimed: schedule.claimed_amount,
            claimable,
        })
    }

    /// Claims unlocked vested tokens for a vest schedule. Callable by beneficiary.
    pub fn claim_vested(env: Env, vest_id: u64) -> Result<i128, ContractError> {
        let key = DataKey::Vest(vest_id);
        let mut schedule: VestSchedule = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::VestNotFound)?;

        schedule.wallet.require_auth();

        let now = env.ledger().timestamp();
        let cliff_time = schedule
            .start_timestamp
            .checked_add(schedule.cliff_seconds)
            .ok_or(ContractError::ArithmeticOverflow)?;

        if now < cliff_time {
            return Err(ContractError::CliffNotReached);
        }

        let unlocked = calculate_unlocked_amount(&schedule, now)?;
        let claimable = unlocked.saturating_sub(schedule.claimed_amount);

        if claimable <= 0 {
            return Err(ContractError::NoVestedTokens);
        }

        schedule.claimed_amount = schedule
            .claimed_amount
            .checked_add(claimable)
            .ok_or(ContractError::ArithmeticOverflow)?;

        let remaining_locked = schedule.amount.saturating_sub(schedule.claimed_amount);

        env.storage().persistent().set(&key, &schedule);
        env.storage()
            .persistent()
            .extend_ttl(&key, VEST_TTL_THRESHOLD, VEST_TTL_EXTEND_TO);

        env.events().publish(
            (VEST_CLAIMED_EVENT_NAME, schedule.wallet.clone()),
            VestClaimed {
                vest_id,
                wallet: schedule.wallet,
                key_id: schedule.key_id,
                amount: claimable,
                remaining_locked,
            },
        );

        Ok(claimable)
    }
}
