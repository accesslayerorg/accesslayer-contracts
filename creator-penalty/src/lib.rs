#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    /// Contract has already been initialized.
    AlreadyInitialized = 1,
    /// Caller is not authorized to perform this action.
    Unauthorized = 2,
    /// Penalty amount must be greater than zero.
    InvalidAmount = 3,
    /// Creator or slash record not found.
    NotFound = 4,
    /// Creator has already been slashed for this violation.
    AlreadySlashed = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub creator: Address,
    pub amount: i128,
    pub reason: String,
    pub resolved: bool,
}

#[contracttype]
enum Key {
    Admin,
    Violation(Address),
}

#[contract]
pub struct CreatorPenaltyContract;

#[contractimpl]
impl CreatorPenaltyContract {
    pub fn initialize(e: Env, admin: Address) -> Result<(), Error> {
        if e.storage().instance().has(&Key::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        e.storage().instance().set(&Key::Admin, &admin);
        Ok(())
    }
    pub fn slash(e: Env, creator: Address, amount: i128, reason: String) -> Result<(), Error> {
        let admin: Address = e
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::Unauthorized)?;
        admin.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let v = Violation {
            creator: creator.clone(),
            amount,
            reason,
            resolved: false,
        };
        if e.storage()
            .persistent()
            .has(&Key::Violation(creator.clone()))
        {
            return Err(Error::AlreadySlashed);
        }
        e.storage()
            .persistent()
            .set(&Key::Violation(creator.clone()), &v);
        e.events()
            .publish((soroban_sdk::symbol_short!("slash"), creator), v.clone());
        Ok(())
    }
    pub fn resolve(e: Env, creator: Address) -> Result<(), Error> {
        let admin: Address = e
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::Unauthorized)?;
        admin.require_auth();
        let mut v: Violation = e
            .storage()
            .persistent()
            .get(&Key::Violation(creator))
            .ok_or(Error::NotFound)?;
        v.resolved = true;
        e.storage()
            .persistent()
            .set(&Key::Violation(v.creator.clone()), &v);
        Ok(())
    }
    pub fn get_violation(e: Env, creator: Address) -> Result<Violation, Error> {
        e.storage()
            .persistent()
            .get(&Key::Violation(creator))
            .ok_or(Error::NotFound)
    }
}
