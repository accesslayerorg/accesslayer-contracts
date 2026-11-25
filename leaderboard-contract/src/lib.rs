#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

pub const LEADERBOARD_UPDATED_EVENT_NAME: Symbol = symbol_short!("lb_upd");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidLimit = 3,
    InvalidScore = 4,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MetricType {
    Volume = 0,
    Pnl = 1,
    HoldingDays = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaderboardEntry {
    pub wallet: Address,
    pub score: i128,
}

#[contracttype]
#[derive(Clone)]
enum Key {
    Admin,
    Authorized(Address),
    Scores(Address, MetricType),
}

#[contract]
pub struct LeaderboardContract;

#[contractimpl]
impl LeaderboardContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&Key::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&Key::Admin, &admin);
        Ok(())
    }

    pub fn set_authorized(
        env: Env,
        admin: Address,
        contract: Address,
        authorized: bool,
    ) -> Result<(), Error> {
        let current: Address = env
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::Unauthorized)?;
        admin.require_auth();
        if admin != current {
            return Err(Error::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&Key::Authorized(contract), &authorized);
        Ok(())
    }

    /// Sets a wallet's absolute score. Only configured integration contracts may call this.
    pub fn update_leaderboard(
        env: Env,
        caller: Address,
        key_id: Address,
        wallet: Address,
        metric_type: MetricType,
        value: i128,
    ) -> Result<(), Error> {
        caller.require_auth();
        if !env
            .storage()
            .instance()
            .get(&Key::Authorized(caller))
            .unwrap_or(false)
        {
            return Err(Error::Unauthorized);
        }
        if value < 0 {
            return Err(Error::InvalidScore);
        }
        let storage_key = Key::Scores(key_id.clone(), metric_type);
        let mut entries: Vec<LeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&storage_key)
            .unwrap_or(Vec::new(&env));
        let mut found = false;
        for i in 0..entries.len() {
            let mut entry = entries.get(i).unwrap();
            if entry.wallet == wallet {
                entry.score = value;
                entries.set(i, entry);
                found = true;
                break;
            }
        }
        if !found {
            entries.push_back(LeaderboardEntry {
                wallet,
                score: value,
            });
        }
        // Stable deterministic ordering: descending score, then ascending address XDR order.
        for i in 1..entries.len() {
            let mut j = i;
            while j > 0 {
                let left = entries.get(j - 1).unwrap();
                let right = entries.get(j).unwrap();
                if left.score >= right.score {
                    break;
                }
                entries.set(j - 1, right);
                entries.set(j, left);
                j -= 1;
            }
        }
        let mut old_top: Vec<Address> = Vec::new(&env);
        let old: Vec<LeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&storage_key)
            .unwrap_or(Vec::new(&env));
        for i in 0..old.len().min(10) {
            old_top.push_back(old.get(i).unwrap().wallet);
        }
        let mut new_top: Vec<Address> = Vec::new(&env);
        for i in 0..entries.len().min(10) {
            new_top.push_back(entries.get(i).unwrap().wallet);
        }
        env.storage().persistent().set(&storage_key, &entries);
        if old_top != new_top {
            env.events().publish(
                (LEADERBOARD_UPDATED_EVENT_NAME, key_id, metric_type),
                new_top,
            );
        }
        Ok(())
    }

    pub fn get_leaderboard(
        env: Env,
        key_id: Address,
        metric_type: MetricType,
        limit: u32,
    ) -> Result<Vec<LeaderboardEntry>, Error> {
        if limit > 100 {
            return Err(Error::InvalidLimit);
        }
        let entries: Vec<LeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&Key::Scores(key_id, metric_type))
            .unwrap_or(Vec::new(&env));
        let mut result = Vec::new(&env);
        for i in 0..entries.len().min(limit) {
            result.push_back(entries.get(i).unwrap());
        }
        Ok(result)
    }
}
