#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    /// The contract admin has already been initialized.
    /// Action: Do not initialize again; use the existing admin configuration.
    AlreadyInitialized = 1,
    /// Reserved for admin-authorized operations when the caller is not the configured admin.
    /// Action: Invoke admin-only operations with the configured admin address and its authorization.
    Unauthorized = 2,
    /// The user nonce has already been recorded for a sponsored authorization.
    /// Action: Generate a fresh unused nonce for the user and retry the authorization.
    NonceUsed = 3,
    /// The sponsored authorization has passed its expiry ledger timestamp.
    /// Action: Create a new authorization with an unexpired `expires_at` timestamp and retry.
    Expired = 4,
    /// The provided maximum relayer fee is negative.
    /// Action: Provide a `max_fee` of zero or a positive value within the caller's intended limit.
    InvalidFee = 5,
}
#[contracttype]
enum Key {
    Admin,
    Nonce(Address, u64),
}

#[contract]
pub struct SponsoredTransactions;
#[contractimpl]
impl SponsoredTransactions {
    pub fn initialize(e: Env, admin: Address) -> Result<(), Error> {
        if e.storage().instance().has(&Key::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        e.storage().instance().set(&Key::Admin, &admin);
        Ok(())
    }
    /// Records a relayed invocation after the user has authenticated its nonce and expiry.
    /// Execution of the target call remains atomic in the caller; this contract prevents replay.
    pub fn authorize(
        e: Env,
        user: Address,
        relayer: Address,
        nonce: u64,
        expires_at: u64,
        max_fee: i128,
    ) -> Result<(), Error> {
        user.require_auth();
        if e.ledger().timestamp() > expires_at {
            return Err(Error::Expired);
        }
        if max_fee < 0 {
            return Err(Error::InvalidFee);
        }
        if e.storage()
            .persistent()
            .has(&Key::Nonce(user.clone(), nonce))
        {
            return Err(Error::NonceUsed);
        }
        e.storage()
            .persistent()
            .set(&Key::Nonce(user.clone(), nonce), &relayer);
        e.events().publish(
            (soroban_sdk::symbol_short!("sponsor"), user),
            (relayer, nonce, max_fee),
        );
        Ok(())
    }
    pub fn is_used(e: Env, user: Address, nonce: u64) -> bool {
        e.storage().persistent().has(&Key::Nonce(user, nonce))
    }
}
