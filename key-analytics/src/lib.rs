#![no_std]

//! Key analytics snapshot contract for historical performance tracking.
//!
//! Records daily snapshots of creator key performance metrics (spot price,
//! cumulative volume, holder count, and circulating market capitalization)
//! to enable on-chain and off-chain historical charting, trend analysis,
//! and benchmarking.
//!
//! # Trust Model & Metric Sourcing
//!
//! - **Permissionless Daily Snapshots**: Any account or relayer may call
//!   [`KeyAnalyticsContract::take_daily_snapshot`] once per calendar day per key.
//!   The exact time of day at which the snapshot is recorded is caller-chosen,
//!   recording the live spot price and state in effect at the moment of the call.
//! - **Tamper-Resistance**: Sourcing does not accept caller-supplied metrics.
//!   All metrics are read on-chain via a cross-contract view call to the
//!   immutable `creator-keys` contract configured at deployment via
//!   [`CreatorKeysReader::try_get_key_market_stats`]. Using `try_*` ensures that
//!   external failures (e.g. unregistered keys) return a typed
//!   [`AnalyticsSnapshotError::MetricFetchFailed`] rather than trapping execution.
//! - **Spot Buy Price**: The recorded `price` is the bonding curve spot buy price
//!   at call time, expressed in XLM stroops (before fees).
//! - **Cumulative Volume**: The recorded `volume` is the total cumulative trade volume
//!   in XLM stroops for the creator's key up to the snapshot ledger.
//! - **Market Cap Formula**: Circulating market capitalization is calculated on-chain as:
//!   `market_cap = buy_price * circulating_supply`
//!   using checked integer arithmetic (`checked_mul`), returning
//!   [`AnalyticsSnapshotError::ArithmeticOverflow`] if multiplication overflows `i128`.
//! - **Deterministic Day Derivation**: The snapshot date index is deterministically
//!   derived from the ledger timestamp as `env.ledger().timestamp() / 86400`. Callers
//!   cannot select or manipulate the snapshot date.
//! - **Persistent Storage & TTL**: Snapshots are stored in persistent contract storage
//!   keyed by `(key_id, date)`. Every write and read extends the storage entry's TTL
//!   towards [`SNAPSHOT_TTL_EXTEND_TO`] (~30 days at 5 seconds per ledger) when below
//!   [`SNAPSHOT_TTL_THRESHOLD`].
//! - **No Admin Role**: The `creator-keys` address is fixed once at deploy via a single-use
//!   [`KeyAnalyticsContract::initialize`]. There are no privileged administrative roles,
//!   no administrative metric setters, and no address update mechanisms.

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, symbol_short, Address,
    Env, Symbol, Vec,
};

#[cfg(test)]
mod test;

/// Ledger threshold at which a snapshot entry's TTL is refreshed towards [`SNAPSHOT_TTL_EXTEND_TO`].
pub const SNAPSHOT_TTL_THRESHOLD: u32 = 100_000;

/// Ledger count to extend a snapshot entry's TTL to (~30 days at 5s ledgers).
pub const SNAPSHOT_TTL_EXTEND_TO: u32 = 518_400;

/// Maximum span of calendar days allowed in a single [`KeyAnalyticsContract::get_snapshot_range`] query.
pub const MAX_SNAPSHOT_RANGE_DAYS: u64 = 90;

/// Number of seconds in one standard calendar day (24 hours = 86,400 seconds).
pub const SECONDS_PER_DAY: u64 = 86_400;

/// Event name emitted when a daily analytics snapshot is recorded.
pub const SNAPSHOT_TAKEN_EVENT_NAME: Symbol = symbol_short!("snp_take");

/// Stable field order of the `SnapshotTaken` payload for downstream indexers.
pub const SNAPSHOT_TAKEN_DATA_FIELDS: [&str; 4] = ["price", "volume", "holder_count", "market_cap"];

/// Errors returned by the key analytics snapshot contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AnalyticsSnapshotError {
    /// A snapshot for this key and date already exists.
    SnapshotExists = 1,
    /// No snapshot was found for this key and date.
    SnapshotNotFound = 2,
    /// `from_date` is strictly greater than `to_date`.
    InvalidRange = 3,
    /// Requested date range span exceeds [`MAX_SNAPSHOT_RANGE_DAYS`].
    RangeTooLarge = 4,
    /// Contract is already initialized.
    AlreadyInitialized = 5,
    /// Contract has not been initialized with the creator-keys address.
    NotInitialized = 6,
    /// Cross-contract metric fetch failed or metric source returned an error.
    MetricFetchFailed = 7,
    /// Arithmetic overflow occurred while computing market capitalization.
    ArithmeticOverflow = 8,
}

/// A complete historical analytics snapshot record stored in persistent storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    /// Key identifier (creator address).
    pub key_id: Address,
    /// Calendar day index derived as `ledger_timestamp / 86400`.
    pub date: u64,
    /// Spot buy price at call time (in XLM stroops).
    pub price: i128,
    /// Cumulative gross trade volume up to this call (in XLM stroops).
    pub volume: i128,
    /// Distinct wallet holder count at call time.
    pub holder_count: u32,
    /// Circulating market capitalization (`price * circulating_supply`).
    pub market_cap: i128,
}

/// Event payload emitted when a daily analytics snapshot is recorded.
///
/// Topics emitted: `(Symbol("snp_take"), key_id: Address, date: u64)`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotTaken {
    /// Spot buy price at call time (in XLM stroops).
    pub price: i128,
    /// Cumulative gross trade volume up to this call (in XLM stroops).
    pub volume: i128,
    /// Distinct wallet holder count at call time.
    pub holder_count: u32,
    /// Circulating market capitalization (`price * circulating_supply`).
    pub market_cap: i128,
}

/// Live market stats view returned by `creator-keys::get_key_market_stats`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyMarketStatsView {
    pub creator: Address,
    pub supply: u32,
    pub buy_price: i128,
    pub sell_price: i128,
    pub holder_count: u32,
    pub volume: i128,
}

/// Minimal read-only interface for cross-contract querying of `creator-keys`.
#[contractclient(name = "CreatorKeysReader")]
pub trait CreatorKeysInterface {
    /// Returns live aggregated market stats for a creator key.
    fn get_key_market_stats(env: Env, creator: Address) -> KeyMarketStatsView;
}

/// Storage keys used by the key analytics contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Configured creator-keys contract address stored in instance storage.
    CreatorKeys,
    /// Historical snapshot entry in persistent storage: `(key_id, date)`.
    Snapshot(Address, u64),
}

fn snapshot_key(key_id: &Address, date: u64) -> DataKey {
    DataKey::Snapshot(key_id.clone(), date)
}

fn extend_snapshot_ttl(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, SNAPSHOT_TTL_THRESHOLD, SNAPSHOT_TTL_EXTEND_TO);
}

fn read_snapshot(env: &Env, key_id: &Address, date: u64) -> Option<Snapshot> {
    let key = snapshot_key(key_id, date);
    let snapshot: Option<Snapshot> = env.storage().persistent().get(&key);
    if snapshot.is_some() {
        extend_snapshot_ttl(env, &key);
    }
    snapshot
}

fn has_snapshot(env: &Env, key_id: &Address, date: u64) -> bool {
    let key = snapshot_key(key_id, date);
    env.storage().persistent().has(&key)
}

fn write_snapshot(env: &Env, snapshot: &Snapshot) {
    let key = snapshot_key(&snapshot.key_id, snapshot.date);
    env.storage().persistent().set(&key, snapshot);
    extend_snapshot_ttl(env, &key);
}

fn emit_snapshot_taken(env: &Env, snapshot: &Snapshot) {
    env.events().publish(
        (
            SNAPSHOT_TAKEN_EVENT_NAME,
            snapshot.key_id.clone(),
            snapshot.date,
        ),
        SnapshotTaken {
            price: snapshot.price,
            volume: snapshot.volume,
            holder_count: snapshot.holder_count,
            market_cap: snapshot.market_cap,
        },
    );
}

fn read_creator_keys(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::CreatorKeys)
}

fn derive_current_date(env: &Env) -> u64 {
    env.ledger().timestamp() / SECONDS_PER_DAY
}

#[contract]
pub struct KeyAnalyticsContract;

#[contractimpl]
impl KeyAnalyticsContract {
    /// Initializes the key analytics contract with the creator-keys address.
    ///
    /// Can only be called once at deployment. There are no subsequent setters
    /// and no administrative roles.
    ///
    /// # Errors
    /// - [`AnalyticsSnapshotError::AlreadyInitialized`] if already initialized.
    pub fn initialize(env: Env, creator_keys: Address) -> Result<(), AnalyticsSnapshotError> {
        if env.storage().instance().has(&DataKey::CreatorKeys) {
            return Err(AnalyticsSnapshotError::AlreadyInitialized);
        }

        env.storage()
            .instance()
            .set(&DataKey::CreatorKeys, &creator_keys);
        env.storage()
            .instance()
            .extend_ttl(SNAPSHOT_TTL_THRESHOLD, SNAPSHOT_TTL_EXTEND_TO);

        Ok(())
    }

    /// Returns the configured creator-keys contract address, if initialized.
    pub fn get_creator_keys(env: Env) -> Option<Address> {
        read_creator_keys(&env)
    }

    /// Records a daily analytics snapshot for `key_id` at the current calendar day.
    ///
    /// Permissionless: any caller may invoke this once per calendar day per key.
    /// The exact time of day is caller-chosen, recording the spot buy price and
    /// state in effect at that moment. Volume is cumulative gross trade volume.
    /// Market cap is derived on-chain as `buy_price * circulating_supply`.
    ///
    /// Sourced via a cross-contract view call to the immutable `creator-keys` contract.
    /// The date index is derived deterministically as `ledger_timestamp / 86400`.
    ///
    /// Emits a `SnapshotTaken` event with topics `(Symbol("snp_take"), key_id, date)`.
    ///
    /// # Errors
    /// - [`AnalyticsSnapshotError::SnapshotExists`] if a snapshot for `(key_id, date)` was already recorded.
    /// - [`AnalyticsSnapshotError::NotInitialized`] if the `creator-keys` address is not configured.
    /// - [`AnalyticsSnapshotError::MetricFetchFailed`] if the cross-contract query fails.
    /// - [`AnalyticsSnapshotError::ArithmeticOverflow`] if `buy_price * supply` overflows `i128`.
    pub fn take_daily_snapshot(
        env: Env,
        key_id: Address,
    ) -> Result<Snapshot, AnalyticsSnapshotError> {
        let date = derive_current_date(&env);
        if has_snapshot(&env, &key_id, date) {
            return Err(AnalyticsSnapshotError::SnapshotExists);
        }

        let creator_keys_addr =
            read_creator_keys(&env).ok_or(AnalyticsSnapshotError::NotInitialized)?;
        let reader = CreatorKeysReader::new(&env, &creator_keys_addr);

        let stats = match reader.try_get_key_market_stats(&key_id) {
            Ok(Ok(s)) => s,
            _ => return Err(AnalyticsSnapshotError::MetricFetchFailed),
        };

        let price = stats.buy_price;
        let volume = stats.volume;
        let holder_count = stats.holder_count;
        let market_cap = price
            .checked_mul(stats.supply as i128)
            .ok_or(AnalyticsSnapshotError::ArithmeticOverflow)?;

        let snapshot = Snapshot {
            key_id,
            date,
            price,
            volume,
            holder_count,
            market_cap,
        };

        write_snapshot(&env, &snapshot);
        emit_snapshot_taken(&env, &snapshot);

        Ok(snapshot)
    }

    /// Returns the analytics snapshot for `key_id` at a specific `date`.
    ///
    /// Extends the persistent storage TTL of the accessed entry (~30 days).
    ///
    /// # Errors
    /// - [`AnalyticsSnapshotError::SnapshotNotFound`] if no snapshot exists for `(key_id, date)`.
    pub fn get_snapshot(
        env: Env,
        key_id: Address,
        date: u64,
    ) -> Result<Snapshot, AnalyticsSnapshotError> {
        read_snapshot(&env, &key_id, date).ok_or(AnalyticsSnapshotError::SnapshotNotFound)
    }

    /// Returns all recorded snapshots for `key_id` across the `[from_date, to_date]` inclusive range.
    ///
    /// Missing days within the date range are omitted. Returned snapshots are strictly
    /// ordered by ascending date. Extends persistent storage TTL on read.
    ///
    /// # Errors
    /// - [`AnalyticsSnapshotError::InvalidRange`] if `from_date > to_date`.
    /// - [`AnalyticsSnapshotError::RangeTooLarge`] if `(to_date - from_date + 1) > MAX_SNAPSHOT_RANGE_DAYS`.
    pub fn get_snapshot_range(
        env: Env,
        key_id: Address,
        from_date: u64,
        to_date: u64,
    ) -> Result<Vec<Snapshot>, AnalyticsSnapshotError> {
        if from_date > to_date {
            return Err(AnalyticsSnapshotError::InvalidRange);
        }

        let span = match to_date.checked_sub(from_date) {
            Some(diff) => match diff.checked_add(1) {
                Some(s) => s,
                None => return Err(AnalyticsSnapshotError::RangeTooLarge),
            },
            None => return Err(AnalyticsSnapshotError::InvalidRange),
        };

        if span > MAX_SNAPSHOT_RANGE_DAYS {
            return Err(AnalyticsSnapshotError::RangeTooLarge);
        }

        let mut results = Vec::new(&env);
        for day in from_date..=to_date {
            if let Some(snapshot) = read_snapshot(&env, &key_id, day) {
                results.push_back(snapshot);
            }
        }

        Ok(results)
    }
}
