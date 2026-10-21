#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReserveError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NotPositiveAmount = 3,
    ArithmeticOverflow = 4,
    InsufficientReserve = 5,
    ExceedsDeploymentCap = 6,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    Reserve(Address),
}

#[contract]
pub struct ReserveContract;

#[contractimpl]
impl ReserveContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), ReserveError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ReserveError::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Accrues 5% of the swap fee amount into the reserve for `key_id`.
    pub fn accrue_reserve(
        env: Env,
        key_id: Address,
        fee_amount: i128,
    ) -> Result<i128, ReserveError> {
        if fee_amount <= 0 {
            return Err(ReserveError::NotPositiveAmount);
        }

        let accrued = fee_amount / 20;
        let balance = read_reserve_balance(&env, &key_id)
            .checked_add(accrued)
            .ok_or(ReserveError::ArithmeticOverflow)?;

        env.storage()
            .persistent()
            .set(&DataKey::Reserve(key_id.clone()), &balance);
        env.events()
            .publish((Symbol::new(&env, "ReserveAccrued"), key_id), accrued);

        Ok(accrued)
    }

    /// Deploys reserve to the bonding curve, up to 50% of its current reserve.
    pub fn deploy_reserve(env: Env, key_id: Address, amount: i128) -> Result<(), ReserveError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ReserveError::NotInitialized)?;
        stored_admin.require_auth();

        if amount <= 0 {
            return Err(ReserveError::NotPositiveAmount);
        }

        let balance = read_reserve_balance(&env, &key_id);
        if amount > balance {
            return Err(ReserveError::InsufficientReserve);
        }
        if amount > balance / 2 {
            return Err(ReserveError::ExceedsDeploymentCap);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Reserve(key_id.clone()), &(balance - amount));
        env.events()
            .publish((Symbol::new(&env, "ReserveDeployed"), key_id), amount);

        Ok(())
    }

    pub fn get_reserve_balance(env: Env, key_id: Address) -> i128 {
        read_reserve_balance(&env, &key_id)
    }
}

fn read_reserve_balance(env: &Env, key_id: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Reserve(key_id.clone()))
        .unwrap_or(0)
}

#[cfg(test)]
mod test;
