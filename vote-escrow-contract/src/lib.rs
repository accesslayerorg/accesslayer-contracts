#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

pub const MIN_LOCK: u64 = 1;
pub const MAX_LOCK: u64 = 4 * 365 * 24 * 60 * 60;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    InvalidAmount = 1,
    InvalidUnlock = 2,
    LockNotFound = 3,
    LockActive = 4,
    CannotShorten = 5,
    Overflow = 6,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lock {
    pub id: u64,
    pub holder: Address,
    pub key_id: Address,
    pub amount: i128,
    pub created_at: u64,
    pub unlock_ledger: u64,
}

#[contracttype]
#[derive(Clone)]
enum Key {
    NextId,
    Lock(u64),
    HolderLock(Address, u64),
}

#[contract]
pub struct VoteEscrowContract;

fn lock_key(id: u64) -> Key {
    Key::Lock(id)
}

#[contractimpl]
impl VoteEscrowContract {
    pub fn lock(
        e: Env,
        holder: Address,
        key_id: Address,
        amount: i128,
        unlock_ledger: u64,
    ) -> Result<u64, Error> {
        holder.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let now = e.ledger().sequence() as u64;
        if unlock_ledger <= now || unlock_ledger - now < MIN_LOCK || unlock_ledger - now > MAX_LOCK
        {
            return Err(Error::InvalidUnlock);
        }
        let id = e
            .storage()
            .persistent()
            .get::<Key, u64>(&Key::NextId)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        e.storage().persistent().set(&Key::NextId, &id);
        let value = Lock {
            id,
            holder: holder.clone(),
            key_id,
            amount,
            created_at: now,
            unlock_ledger,
        };
        e.storage().persistent().set(&lock_key(id), &value);
        e.storage()
            .persistent()
            .set(&Key::HolderLock(holder, id), &true);
        e.events()
            .publish((soroban_sdk::symbol_short!("locked"), id), value);
        Ok(id)
    }

    pub fn extend_lock(
        e: Env,
        holder: Address,
        id: u64,
        new_unlock_ledger: u64,
    ) -> Result<(), Error> {
        holder.require_auth();
        let mut lock: Lock = e
            .storage()
            .persistent()
            .get(&lock_key(id))
            .ok_or(Error::LockNotFound)?;
        if lock.holder != holder {
            return Err(Error::LockNotFound);
        }
        if new_unlock_ledger <= lock.unlock_ledger || new_unlock_ledger - lock.created_at > MAX_LOCK
        {
            return Err(Error::CannotShorten);
        }
        lock.unlock_ledger = new_unlock_ledger;
        e.storage().persistent().set(&lock_key(id), &lock);
        e.events().publish(
            (soroban_sdk::symbol_short!("extended"), id),
            new_unlock_ledger,
        );
        Ok(())
    }

    pub fn increase_lock(e: Env, holder: Address, id: u64, amount: i128) -> Result<(), Error> {
        holder.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let mut lock: Lock = e
            .storage()
            .persistent()
            .get(&lock_key(id))
            .ok_or(Error::LockNotFound)?;
        if lock.holder != holder {
            return Err(Error::LockNotFound);
        }
        lock.amount = lock.amount.checked_add(amount).ok_or(Error::Overflow)?;
        e.storage().persistent().set(&lock_key(id), &lock);
        Ok(())
    }

    pub fn voting_power(e: Env, id: u64, at_ledger: u64) -> Result<i128, Error> {
        let lock: Lock = e
            .storage()
            .persistent()
            .get(&lock_key(id))
            .ok_or(Error::LockNotFound)?;
        if at_ledger >= lock.unlock_ledger {
            return Ok(0);
        }
        let remaining = lock.unlock_ledger - at_ledger;
        let duration = lock.unlock_ledger - lock.created_at;
        lock.amount
            .checked_mul(remaining as i128)
            .ok_or(Error::Overflow)
            .map(|v| v / duration as i128)
    }

    pub fn withdraw(e: Env, holder: Address, id: u64) -> Result<i128, Error> {
        holder.require_auth();
        let lock: Lock = e
            .storage()
            .persistent()
            .get(&lock_key(id))
            .ok_or(Error::LockNotFound)?;
        if lock.holder != holder {
            return Err(Error::LockNotFound);
        }
        if (e.ledger().sequence() as u64) < lock.unlock_ledger {
            return Err(Error::LockActive);
        }
        e.storage().persistent().remove(&lock_key(id));
        e.storage()
            .persistent()
            .remove(&Key::HolderLock(holder, id));
        e.events()
            .publish((soroban_sdk::symbol_short!("withdraw"), id), lock.amount);
        Ok(lock.amount)
    }

    pub fn get_lock(e: Env, id: u64) -> Option<Lock> {
        e.storage().persistent().get(&lock_key(id))
    }
}
