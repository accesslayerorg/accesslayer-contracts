//! Ledger-indexed balance and supply checkpoints (issue #1261).
//!
//! Governance, dividend distribution and gating need a holder's balance (or a
//! key's supply) *at a past ledger*, but persistent storage only keeps the
//! current value. This module records a compact `Checkpoint { ledger, balance }`
//! vector per `(key_id, holder)` and per `key_id`, so any historical value can
//! be recovered with a binary search.
//!
//! Invariants:
//!
//! * At most one checkpoint exists per ledger; a second change in the same
//!   ledger replaces the last entry instead of appending.
//! * Entries are strictly ascending by ledger.
//! * Lookups before the first checkpoint return `0`.
//! * [`compact`] drops history before a cutoff while retaining the latest
//!   preceding checkpoint, so lookups at or after the cutoff stay exact.

use crate::{
    assert_is_admin, extend_key_ttl_to_full_window, ContractError, CreatorKeysContract,
    CreatorKeysContractArgs, CreatorKeysContractClient,
};
use soroban_sdk::{contractimpl, contracttype, Address, Env, IntoVal, Val, Vec};

/// A single `(ledger, balance)` observation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Checkpoint {
    /// Ledger sequence the balance was in force from.
    pub ledger: u32,
    /// Balance recorded at `ledger`.
    pub balance: u32,
}

/// Storage keys for checkpoint vectors.
///
/// Kept in its own `#[contracttype]` enum, matching the convention used by the
/// other feature families (`RevenueKey`, `StakingKey`, ...) instead of growing
/// the main [`crate::DataKey`] enum.
#[derive(Clone)]
#[contracttype]
pub enum CheckpointKey {
    /// `(key_id, holder)` -> ascending `Vec<Checkpoint>` of holder balances.
    Balance(Address, Address),
    /// `key_id` -> ascending `Vec<Checkpoint>` of key supply.
    Supply(Address),
}

fn balance_key(key_id: &Address, holder: &Address) -> CheckpointKey {
    CheckpointKey::Balance(key_id.clone(), holder.clone())
}

fn supply_key(key_id: &Address) -> CheckpointKey {
    CheckpointKey::Supply(key_id.clone())
}

/// Appends `balance` at the current ledger, replacing the last entry when it
/// was already written in this ledger.
fn record<K>(env: &Env, key: &K, balance: u32)
where
    K: IntoVal<Env, Val> + Clone,
{
    let ledger = env.ledger().sequence();
    let mut checkpoints: Vec<Checkpoint> = env
        .storage()
        .persistent()
        .get(key)
        .unwrap_or_else(|| Vec::new(env));

    let last_index = checkpoints.len().checked_sub(1);
    match last_index {
        Some(index) if checkpoints.get(index).unwrap().ledger == ledger => {
            checkpoints.set(index, Checkpoint { ledger, balance });
        }
        _ => checkpoints.push_back(Checkpoint { ledger, balance }),
    }

    env.storage().persistent().set(key, &checkpoints);
    extend_key_ttl_to_full_window(env, key);
}

/// Records a holder balance checkpoint for `(key_id, holder)`.
pub fn record_balance_checkpoint(env: &Env, key_id: &Address, holder: &Address, balance: u32) {
    record(env, &balance_key(key_id, holder), balance);
}

/// Records a key supply checkpoint for `key_id`.
pub fn record_supply_checkpoint(env: &Env, key_id: &Address, supply: u32) {
    record(env, &supply_key(key_id), supply);
}

/// Returns the balance in force at `ledger`, or `0` when `ledger` precedes the
/// first recorded checkpoint.
pub fn balance_at(env: &Env, key_id: &Address, holder: &Address, ledger: u32) -> u32 {
    lookup(env, &balance_key(key_id, holder), ledger)
}

/// Returns the supply in force at `ledger`, or `0` when `ledger` precedes the
/// first recorded checkpoint.
pub fn supply_at(env: &Env, key_id: &Address, ledger: u32) -> u32 {
    lookup(env, &supply_key(key_id), ledger)
}

/// Snapshot read used by governance and dividend distribution.
///
/// When checkpoints exist this is exactly [`balance_at`]. Deployments that
/// predate checkpoints have no history for their holders, so the live entry is
/// returned instead: at the snapshot ledger it *is* the balance in force, and
/// without this fallback upgrading would zero out every pre-existing holder's
/// vote weight and dividend share.
pub fn balance_at_or_live(env: &Env, key_id: &Address, holder: &Address, ledger: u32) -> u32 {
    let key = balance_key(key_id, holder);
    if env.storage().persistent().has(&key) {
        lookup(env, &key, ledger)
    } else {
        env.storage()
            .persistent()
            .get(&crate::constants::storage::holder_balance_key(
                key_id, holder,
            ))
            .unwrap_or(0)
    }
}

fn lookup<K>(env: &Env, key: &K, ledger: u32) -> u32
where
    K: IntoVal<Env, Val> + Clone,
{
    let checkpoints: Vec<Checkpoint> = match env.storage().persistent().get(key) {
        Some(checkpoints) => checkpoints,
        None => return 0,
    };

    // Upper bound: first index whose ledger is strictly greater than `ledger`.
    let mut lo = 0u32;
    let mut hi = checkpoints.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if checkpoints.get(mid).unwrap().ledger <= ledger {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    if lo == 0 {
        0
    } else {
        checkpoints.get(lo - 1).unwrap().balance
    }
}

/// Drops checkpoints strictly before `before_ledger`, keeping the latest one
/// that precedes the cutoff so lookups between it and the cutoff stay exact.
///
/// Returns the number of removed entries.
fn compact<K>(env: &Env, key: &K, before_ledger: u32) -> u32
where
    K: IntoVal<Env, Val> + Clone,
{
    let checkpoints: Vec<Checkpoint> = match env.storage().persistent().get(key) {
        Some(checkpoints) => checkpoints,
        None => return 0,
    };

    // Index of the latest checkpoint strictly before the cutoff.
    let mut pivot: Option<u32> = None;
    for index in 0..checkpoints.len() {
        if checkpoints.get(index).unwrap().ledger < before_ledger {
            pivot = Some(index);
        } else {
            break;
        }
    }

    let mut retained: Vec<Checkpoint> = Vec::new(env);
    if let Some(pivot) = pivot {
        retained.push_back(checkpoints.get(pivot).unwrap());
        for index in (pivot + 1)..checkpoints.len() {
            retained.push_back(checkpoints.get(index).unwrap());
        }
    } else {
        // Nothing precedes the cutoff, so nothing is removed.
        return 0;
    }

    let removed = checkpoints.len() - retained.len();
    if removed == 0 {
        return 0;
    }

    if retained.is_empty() {
        env.storage().persistent().remove(key);
    } else {
        env.storage().persistent().set(key, &retained);
        extend_key_ttl_to_full_window(env, key);
    }
    removed
}

/// Persists `balance` for `(key_id, holder)` and records the checkpoint.
///
/// A zero balance removes the live entry (matching the pre-existing sparse
/// storage convention) but still records a checkpoint, so `balance_at` reports
/// the exit correctly for that ledger and later ones.
pub fn write_holder_balance(env: &Env, key_id: &Address, holder: &Address, balance: u32) {
    let key = crate::constants::storage::holder_balance_key(key_id, holder);
    if balance == 0 {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &balance);
        extend_key_ttl_to_full_window(env, &key);
    }
    record_balance_checkpoint(env, key_id, holder, balance);
}

#[contractimpl]
impl CreatorKeysContract {
    /// Returns `holder`'s balance of `key_id` in force at `ledger`.
    ///
    /// Returns `0` when `ledger` is before the first recorded checkpoint, which
    /// also covers holders that never held the key.
    pub fn balance_at(env: Env, holder: Address, key_id: Address, ledger: u32) -> u32 {
        balance_at(&env, &key_id, &holder, ledger)
    }

    /// Returns the supply of `key_id` in force at `ledger`, or `0` before the
    /// first recorded checkpoint.
    pub fn supply_at(env: Env, key_id: Address, ledger: u32) -> u32 {
        supply_at(&env, &key_id, ledger)
    }

    /// Compacts `holder`'s checkpoint history for `key_id`, removing entries
    /// strictly before `before_ledger` while keeping the latest preceding one.
    ///
    /// Admin-only: historical balances are shared governance and dividend input.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] when `caller` is not the protocol admin.
    pub fn compact(
        env: Env,
        caller: Address,
        holder: Address,
        key_id: Address,
        before_ledger: u32,
    ) -> Result<u32, ContractError> {
        caller.require_auth();
        assert_is_admin(&env, &caller)?;
        Ok(compact(&env, &balance_key(&key_id, &holder), before_ledger))
    }

    /// Compacts the supply checkpoint history of `key_id`.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] when `caller` is not the protocol admin.
    pub fn compact_supply(
        env: Env,
        caller: Address,
        key_id: Address,
        before_ledger: u32,
    ) -> Result<u32, ContractError> {
        caller.require_auth();
        assert_is_admin(&env, &caller)?;
        Ok(compact(&env, &supply_key(&key_id), before_ledger))
    }
}

#[cfg(test)]
mod tests {
    use super::{balance_at, record_balance_checkpoint, record_supply_checkpoint, supply_at};
    use crate::CreatorKeysContract;
    use soroban_sdk::{
        testutils::{Address as _, Ledger as _},
        Address, Env,
    };

    fn setup() -> (Env, Address, Address, Address) {
        let env = Env::default();
        let contract_id = env.register(CreatorKeysContract, ());
        let key_id = Address::generate(&env);
        let holder = Address::generate(&env);
        (env, contract_id, key_id, holder)
    }

    #[test]
    fn lookups_at_between_and_before_checkpoints() {
        let (env, contract_id, key_id, holder) = setup();
        env.as_contract(&contract_id, || {
            env.ledger().set_sequence_number(100);
            record_balance_checkpoint(&env, &key_id, &holder, 10);
            env.ledger().set_sequence_number(200);
            record_balance_checkpoint(&env, &key_id, &holder, 25);
            env.ledger().set_sequence_number(300);
            record_balance_checkpoint(&env, &key_id, &holder, 5);

            // Before the first checkpoint.
            assert_eq!(balance_at(&env, &key_id, &holder, 0), 0);
            assert_eq!(balance_at(&env, &key_id, &holder, 99), 0);
            // Exactly at a checkpoint.
            assert_eq!(balance_at(&env, &key_id, &holder, 100), 10);
            assert_eq!(balance_at(&env, &key_id, &holder, 200), 25);
            assert_eq!(balance_at(&env, &key_id, &holder, 300), 5);
            // Between checkpoints and after the last one.
            assert_eq!(balance_at(&env, &key_id, &holder, 150), 10);
            assert_eq!(balance_at(&env, &key_id, &holder, 299), 25);
            assert_eq!(balance_at(&env, &key_id, &holder, 1_000), 5);
        });
    }

    #[test]
    fn multiple_changes_in_one_ledger_store_one_checkpoint() {
        let (env, contract_id, key_id, holder) = setup();
        env.as_contract(&contract_id, || {
            env.ledger().set_sequence_number(42);
            record_balance_checkpoint(&env, &key_id, &holder, 1);
            record_balance_checkpoint(&env, &key_id, &holder, 2);
            record_balance_checkpoint(&env, &key_id, &holder, 3);

            use super::CheckpointKey;
            let stored: soroban_sdk::Vec<super::Checkpoint> = env
                .storage()
                .persistent()
                .get(&CheckpointKey::Balance(key_id.clone(), holder.clone()))
                .unwrap();
            assert_eq!(stored.len(), 1);
            assert_eq!(stored.get(0).unwrap().balance, 3);
            assert_eq!(balance_at(&env, &key_id, &holder, 42), 3);
        });
    }

    #[test]
    fn supply_checkpoints_track_supply_changes() {
        let (env, contract_id, key_id, _holder) = setup();
        env.as_contract(&contract_id, || {
            env.ledger().set_sequence_number(10);
            record_supply_checkpoint(&env, &key_id, 0);
            env.ledger().set_sequence_number(20);
            record_supply_checkpoint(&env, &key_id, 4);

            assert_eq!(supply_at(&env, &key_id, 5), 0);
            assert_eq!(supply_at(&env, &key_id, 10), 0);
            assert_eq!(supply_at(&env, &key_id, 19), 0);
            assert_eq!(supply_at(&env, &key_id, 20), 4);
            assert_eq!(supply_at(&env, &key_id, 21), 4);
        });
    }

    #[test]
    fn compaction_keeps_latest_preceding_checkpoint() {
        let (env, contract_id, key_id, holder) = setup();
        env.as_contract(&contract_id, || {
            env.ledger().set_sequence_number(100);
            record_balance_checkpoint(&env, &key_id, &holder, 10);
            env.ledger().set_sequence_number(200);
            record_balance_checkpoint(&env, &key_id, &holder, 25);
            env.ledger().set_sequence_number(300);
            record_balance_checkpoint(&env, &key_id, &holder, 5);
            env.ledger().set_sequence_number(400);
            record_balance_checkpoint(&env, &key_id, &holder, 8);

            // Remove everything before 300 but keep the 200 checkpoint.
            let removed = super::compact(&env, &super::balance_key(&key_id, &holder), 300);
            assert_eq!(removed, 1);
            assert_eq!(balance_at(&env, &key_id, &holder, 250), 25);
            assert_eq!(balance_at(&env, &key_id, &holder, 300), 5);
            assert_eq!(balance_at(&env, &key_id, &holder, 400), 8);
            // The dropped 100 checkpoint no longer answers.
            assert_eq!(balance_at(&env, &key_id, &holder, 100), 0);
        });
    }
}
