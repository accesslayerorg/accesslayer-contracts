#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String};
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    NameTooLong = 3,
    MissingKey = 4,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SocialMetadata {
    pub display_name: String,
    pub bio: String,
    pub avatar_cid: String,
    pub twitter: String,
    pub website: String,
    pub discord: String,
}
#[contracttype]
enum Key {
    Admin,
    Creator(Address),
    Metadata(Address),
}
#[contract]
pub struct SocialMetadataContract;
#[contractimpl]
impl SocialMetadataContract {
    pub fn initialize(e: Env, admin: Address) -> Result<(), Error> {
        if e.storage().instance().has(&Key::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        e.storage().instance().set(&Key::Admin, &admin);
        Ok(())
    }
    pub fn register_key(
        e: Env,
        admin: Address,
        key: Address,
        creator: Address,
    ) -> Result<(), Error> {
        let a: Address = e
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::Unauthorized)?;
        if a != admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        e.storage().persistent().set(&Key::Creator(key), &creator);
        Ok(())
    }
    pub fn set_social_metadata(e: Env, key: Address, data: SocialMetadata) -> Result<(), Error> {
        let c: Address = e
            .storage()
            .persistent()
            .get(&Key::Creator(key.clone()))
            .ok_or(Error::MissingKey)?;
        c.require_auth();
        if data.display_name.len() > 50 {
            return Err(Error::NameTooLong);
        }
        e.storage()
            .persistent()
            .set(&Key::Metadata(key.clone()), &data);
        e.events()
            .publish((soroban_sdk::symbol_short!("metadata"), key), data);
        Ok(())
    }
    pub fn get_social_metadata(e: Env, key: Address) -> Result<SocialMetadata, Error> {
        e.storage()
            .persistent()
            .get(&Key::Metadata(key))
            .ok_or(Error::MissingKey)
    }
}
