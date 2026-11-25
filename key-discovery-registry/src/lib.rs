#![no_std]
//! On-chain key discovery registry (issue #1037).
//!
//! Tracks keys deployed through the creator-keys factory, records per-key trade
//! volume, and exposes sorted discovery views for "trending" (top keys by 24h
//! volume) and "new keys" (keys deployed since a timestamp).
//!
//! Integration points:
//! * `register_key` is called by the factory (or a relayer) whenever a
//!   `KeyDeployed` event is emitted for a newly deployed key.
//! * `record_volume` is called by the bonding curve on each trade to report the
//!   traded volume for a key.
//!
//! Both entrypoints are gated: only the admin, or an address the admin
//! explicitly authorised (`set_registrar` / `set_recorder`), may call them.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

#[cfg(test)]
mod test;

/// Number of seconds in an hour; used for volume-window math.
pub const SECONDS_PER_HOUR: u64 = 3_600;
/// Rolling window used by `get_trending` (24 hours).
pub const VOLUME_WINDOW_24H: u64 = 24 * SECONDS_PER_HOUR;
/// Volume records older than this are pruned on write (48 hours).
pub const VOLUME_RETENTION_48H: u64 = 48 * SECONDS_PER_HOUR;

pub const KEY_REGISTERED_EVENT_NAME: Symbol = symbol_short!("key_reg");
pub const VOLUME_RECORDED_EVENT_NAME: Symbol = symbol_short!("vol_rec");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DiscoveryError {
    AlreadyInitialised = 1,
    NotInitialised = 2,
    Unauthorized = 3,
    AlreadyRegistered = 4,
    KeyNotRegistered = 5,
    NotPositiveAmount = 6,
    ArithmeticOverflow = 7,
}

/// Registration metadata for a discovered key.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyInfo {
    pub key_id: Address,
    pub creator: Address,
    pub deploy_timestamp: u64,
}

/// A single volume observation recorded by the bonding curve.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VolumeEntry {
    pub timestamp: u64,
    pub amount: i128,
}

/// One row returned by `get_trending`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TrendingEntry {
    pub key_id: Address,
    pub volume_24h: i128,
    pub deploy_timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    Initialised,
    Registrar(Address),
    Recorder(Address),
    KeyInfo(Address),
    KeyIndex,
    Volume(Address),
}

#[contract]
pub struct KeyDiscoveryRegistry;

fn is_initialised(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Initialised)
        .unwrap_or(false)
}

fn read_admin(env: &Env) -> Result<Address, DiscoveryError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(DiscoveryError::NotInitialised)
}

fn read_key_index(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::KeyIndex)
        .unwrap_or_else(|| Vec::new(env))
}

fn write_key_index(env: &Env, index: &Vec<Address>) {
    env.storage().persistent().set(&DataKey::KeyIndex, index);
}

fn read_key_info(env: &Env, key_id: &Address) -> Option<KeyInfo> {
    env.storage()
        .persistent()
        .get(&DataKey::KeyInfo(key_id.clone()))
}

fn load_volume_entries(env: &Env, key_id: &Address) -> Vec<VolumeEntry> {
    env.storage()
        .persistent()
        .get(&DataKey::Volume(key_id.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

fn write_volume_entries(env: &Env, key_id: &Address, entries: &Vec<VolumeEntry>) {
    env.storage()
        .persistent()
        .set(&DataKey::Volume(key_id.clone()), entries);
}

/// Drops every volume record strictly older than the 48-hour retention window
/// relative to `now`. Returns `true` when at least one record was removed.
fn prune_volume(env: &Env, key_id: &Address, now: u64) -> bool {
    let entries = load_volume_entries(env, key_id);
    if entries.is_empty() {
        return false;
    }
    let cutoff = now.saturating_sub(VOLUME_RETENTION_48H);
    let mut kept: Vec<VolumeEntry> = Vec::new(env);
    let mut removed = false;
    for entry in entries.iter() {
        if entry.timestamp >= cutoff {
            kept.push_back(entry);
        } else {
            removed = true;
        }
    }
    if removed {
        write_volume_entries(env, key_id, &kept);
    }
    removed
}

/// Sums the volume recorded for `key_id` within `[now - 24h, now]`.
fn volume_24h(env: &Env, key_id: &Address, now: u64) -> i128 {
    let entries = load_volume_entries(env, key_id);
    let window_start = now.saturating_sub(VOLUME_WINDOW_24H);
    let mut total: i128 = 0;
    for entry in entries.iter() {
        if entry.timestamp >= window_start {
            total = total.saturating_add(entry.amount);
        }
    }
    total
}

/// Returns `true` when `a` should be ordered ahead of `b` in the trending list:
/// higher 24h volume first, ties broken by earlier deployment.
fn trending_ranks_higher(a: &TrendingEntry, b: &TrendingEntry) -> bool {
    if a.volume_24h != b.volume_24h {
        a.volume_24h > b.volume_24h
    } else {
        a.deploy_timestamp < b.deploy_timestamp
    }
}

/// Stable insertion sort over the trending rows (descending by volume).
fn sort_trending(entries: &mut Vec<TrendingEntry>) {
    let n = entries.len();
    let mut i: u32 = 1;
    while i < n {
        let mut j = i;
        while j > 0 {
            let cur = entries.get(j).unwrap();
            let prev = entries.get(j - 1).unwrap();
            if trending_ranks_higher(&cur, &prev) {
                entries.set(j, prev);
                entries.set(j - 1, cur);
                j -= 1;
            } else {
                break;
            }
        }
        i += 1;
    }
}

#[contractimpl]
impl KeyDiscoveryRegistry {
    /// One-time setup: records the admin allowed to manage authorisations.
    pub fn initialise(env: Env, admin: Address) -> Result<(), DiscoveryError> {
        if is_initialised(&env) {
            return Err(DiscoveryError::AlreadyInitialised);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Initialised, &true);
        Ok(())
    }

    /// Admin allows `registrar` to register newly deployed keys.
    pub fn set_registrar(
        env: Env,
        registrar: Address,
        allowed: bool,
    ) -> Result<(), DiscoveryError> {
        let admin = read_admin(&env)?;
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::Registrar(registrar), &allowed);
        Ok(())
    }

    /// Admin allows `recorder` to report trade volume.
    pub fn set_recorder(env: Env, recorder: Address, allowed: bool) -> Result<(), DiscoveryError> {
        let admin = read_admin(&env)?;
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::Recorder(recorder), &allowed);
        Ok(())
    }

    /// Registers a newly deployed key. Called by the factory (or a relayer)
    /// when a `KeyDeployed` event fires.
    pub fn register_key(
        env: Env,
        caller: Address,
        key_id: Address,
        creator: Address,
    ) -> Result<(), DiscoveryError> {
        caller.require_auth();
        if !is_initialised(&env) {
            return Err(DiscoveryError::NotInitialised);
        }
        let admin = read_admin(&env)?;
        let authorised = caller == admin
            || env
                .storage()
                .instance()
                .get(&DataKey::Registrar(caller.clone()))
                .unwrap_or(false);
        if !authorised {
            return Err(DiscoveryError::Unauthorized);
        }
        if read_key_info(&env, &key_id).is_some() {
            return Err(DiscoveryError::AlreadyRegistered);
        }

        let deploy_timestamp = env.ledger().timestamp();
        let info = KeyInfo {
            key_id: key_id.clone(),
            creator: creator.clone(),
            deploy_timestamp,
        };
        env.storage()
            .persistent()
            .set(&DataKey::KeyInfo(key_id.clone()), &info);

        let mut index = read_key_index(&env);
        index.push_back(key_id.clone());
        write_key_index(&env, &index);

        env.events().publish(
            (KEY_REGISTERED_EVENT_NAME, key_id.clone()),
            KeyInfo {
                key_id: key_id.clone(),
                creator,
                deploy_timestamp,
            },
        );
        Ok(())
    }

    /// Records trade volume for `key_id`. Called by the bonding curve on each
    /// trade. Prunes records older than 48 hours, then appends the new one.
    pub fn record_volume(
        env: Env,
        caller: Address,
        key_id: Address,
        amount: i128,
    ) -> Result<(), DiscoveryError> {
        caller.require_auth();
        if !is_initialised(&env) {
            return Err(DiscoveryError::NotInitialised);
        }
        let admin = read_admin(&env)?;
        let authorised = caller == admin
            || env
                .storage()
                .instance()
                .get(&DataKey::Recorder(caller.clone()))
                .unwrap_or(false);
        if !authorised {
            return Err(DiscoveryError::Unauthorized);
        }
        if amount <= 0 {
            return Err(DiscoveryError::NotPositiveAmount);
        }
        if read_key_info(&env, &key_id).is_none() {
            return Err(DiscoveryError::KeyNotRegistered);
        }

        let now = env.ledger().timestamp();
        prune_volume(&env, &key_id, now);

        let mut entries = load_volume_entries(&env, &key_id);
        entries.push_back(VolumeEntry {
            timestamp: now,
            amount,
        });
        write_volume_entries(&env, &key_id, &entries);

        env.events().publish(
            (VOLUME_RECORDED_EVENT_NAME, key_id.clone()),
            VolumeEntry {
                timestamp: now,
                amount,
            },
        );
        Ok(())
    }

    /// Removes volume records older than 48 hours for `key_id`. Callable by
    /// anyone; it is a pure housekeeping operation.
    pub fn prune(env: Env, key_id: Address) -> bool {
        let now = env.ledger().timestamp();
        prune_volume(&env, &key_id, now)
    }

    /// Read-only view: 24h rolling volume for a single key.
    pub fn get_volume_24h(env: Env, key_id: Address) -> i128 {
        let now = env.ledger().timestamp();
        volume_24h(&env, &key_id, now)
    }

    /// Read-only view: all retained volume records for a key (newest last).
    pub fn get_volume_entries(env: Env, key_id: Address) -> Vec<VolumeEntry> {
        load_volume_entries(&env, &key_id)
    }

    /// Read-only view: registration metadata for a key, if registered.
    pub fn get_key_info(env: Env, key_id: Address) -> Option<KeyInfo> {
        read_key_info(&env, &key_id)
    }

    /// Read-only view: number of registered keys.
    pub fn get_registered_count(env: Env) -> u32 {
        read_key_index(&env).len()
    }

    /// Read-only view: the top `limit` keys by 24h volume, descending. Keys with
    /// zero 24h volume are omitted. Ties are broken by earlier deployment.
    pub fn get_trending(env: Env, limit: u32) -> Vec<TrendingEntry> {
        if limit == 0 {
            return Vec::new(&env);
        }
        let now = env.ledger().timestamp();
        let index = read_key_index(&env);
        let mut rows: Vec<TrendingEntry> = Vec::new(&env);
        for key_id in index.iter() {
            let vol = volume_24h(&env, &key_id, now);
            if vol > 0 {
                let deploy_timestamp = read_key_info(&env, &key_id)
                    .map(|info| info.deploy_timestamp)
                    .unwrap_or(0);
                rows.push_back(TrendingEntry {
                    key_id,
                    volume_24h: vol,
                    deploy_timestamp,
                });
            }
        }
        sort_trending(&mut rows);

        let mut out: Vec<TrendingEntry> = Vec::new(&env);
        let mut i: u32 = 0;
        while i < limit && i < rows.len() {
            out.push_back(rows.get(i).unwrap());
            i += 1;
        }
        out
    }

    /// Read-only view: up to `limit` keys deployed at or after `since_timestamp`,
    /// newest first.
    pub fn get_new_keys(env: Env, since_timestamp: u64, limit: u32) -> Vec<Address> {
        if limit == 0 {
            return Vec::new(&env);
        }
        let index = read_key_index(&env);
        let total = index.len();
        let mut out: Vec<Address> = Vec::new(&env);
        // Walk the registration index newest-first.
        let mut i = total;
        while i > 0 && out.len() < limit {
            i -= 1;
            let key_id = index.get(i).unwrap();
            let deployed_at = read_key_info(&env, &key_id)
                .map(|info| info.deploy_timestamp)
                .unwrap_or(0);
            if deployed_at >= since_timestamp {
                out.push_back(key_id);
            }
        }
        out
    }
}
