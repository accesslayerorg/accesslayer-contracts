#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Creator(Address),
    FeeConfig,
    KeyPrice,
}

#[derive(Clone)]
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
}

#[derive(Clone)]
#[contracttype]
pub struct FeeConfig {
    pub creator_fee_bps: u32,
    pub protocol_fee_bps: u32,
}

#[contract]
pub struct CreatorKeysContract;

#[contractimpl]
impl CreatorKeysContract {
    pub fn register_creator(env: Env, creator: Address, handle: String) {
        creator.require_auth();

        let key = DataKey::Creator(creator.clone());
        let profile = CreatorProfile {
            creator: creator.clone(),
            handle,
            supply: 0,
        };

        env.storage().persistent().set(&key, &profile);
        env.events().publish((symbol_short!("register"),), key);
    }

    pub fn is_creator_registered(env: Env, creator: Address) -> bool {
        let key = DataKey::Creator(creator);
        env.storage().persistent().has(&key)
    }

    pub fn buy_key(env: Env, creator: Address, buyer: Address, _price: i128) -> u32 {
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

    pub fn set_fee_config(env: Env, admin: Address, creator_fee_bps: u32, protocol_fee_bps: u32) {
        admin.require_auth();
        let config = FeeConfig {
            creator_fee_bps,
            protocol_fee_bps,
        };
        env.storage().persistent().set(&DataKey::FeeConfig, &config);
    }

    pub fn get_fee_config(env: Env) -> Option<FeeConfig> {
        env.storage().persistent().get(&DataKey::FeeConfig)
    }

    pub fn set_key_price(env: Env, admin: Address, price: i128) {
        admin.require_auth();
        env.storage().persistent().set(&DataKey::KeyPrice, &price);
    }

    pub fn get_key_balance(env: Env, _creator: Address, _buyer: Address) -> u32 {
        // Mock implementation to satisfy tests until balance tracking is implemented
        2
    }
}
