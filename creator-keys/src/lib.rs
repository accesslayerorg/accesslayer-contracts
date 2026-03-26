#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Creator(Address),
}

#[derive(Clone)]
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
}

#[contract]
pub struct CreatorKeysContract;

#[contractimpl]
impl CreatorKeysContract {
    pub fn register_creator(env: Env, creator: Address, handle: String) {
        creator.require_auth();

        let key = DataKey::Creator(creator.clone());
        let profile = CreatorProfile {
            creator,
            handle,
            supply: 0,
        };

        env.storage().persistent().set(&key, &profile);
        env.events().publish((symbol_short!("register"),), key);
    }

    pub fn buy_key(env: Env, creator: Address, buyer: Address) -> u32 {
        buyer.require_auth();

        let key = DataKey::Creator(creator.clone());
        let mut profile: CreatorProfile = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("creator not registered"));

        profile.supply += 1;
        env.storage().persistent().set(&key, &profile);
        env.events()
            .publish((symbol_short!("buy"), creator, buyer), profile.supply);

        profile.supply
    }

    pub fn sell_key(env: Env, creator: Address, seller: Address) -> u32 {
        seller.require_auth();

        let key = DataKey::Creator(creator.clone());
        let mut profile: CreatorProfile = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("creator not registered"));

        if profile.supply == 0 {
            panic!("zero supply");
        }

        profile.supply -= 1;
        env.storage().persistent().set(&key, &profile);
        env.events()
            .publish((symbol_short!("sell"), creator, seller), profile.supply);

        profile.supply
    }

    pub fn get_creator(env: Env, creator: Address) -> Option<CreatorProfile> {
        let key = DataKey::Creator(creator);
        env.storage().persistent().get(&key)
    }
}
pub fn sell_key(env: Env, creator: Address, seller: Address) -> u32 {
    seller.require_auth();

    let key = DataKey::Creator(creator.clone());
    let mut profile: CreatorProfile = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| panic!("creator not registered"));

    if profile.supply == 0 {
        panic!("zero supply");
    }

    profile.supply -= 1;
    env.storage().persistent().set(&key, &profile);
    env.events()
        .publish((symbol_short!("sell"), creator, seller), profile.supply);

    profile.supply
}
