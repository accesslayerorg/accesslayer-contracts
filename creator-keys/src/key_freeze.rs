//! #1174 — Key-level freeze / unfreeze with trade-block enforcement.
//!
//! A key can be frozen by its multi-sig admins, which blocks every buy, sell
//! and transfer of that key while the freeze is active. Each freeze and
//! unfreeze action is appended to an immutable audit log so the history of a
//! key can be replayed off-chain.
//!
//! * [`CreatorKeysContract::freeze`] moves the key to [`KeyStatus::Frozen`] and
//!   records the reason plus the acting admin.
//! * [`CreatorKeysContract::unfreeze`] moves the key back to
//!   [`KeyStatus::Active`].
//! * [`CreatorKeysContract::get_freeze_log`] returns the append-only audit log.
//! * [`CreatorKeysContract::get_key_status`] and
//!   [`CreatorKeysContract::is_key_frozen`] expose the current status.
//!
//! ### Authorization
//!
//! The acting admin authorizes the call (`admin.require_auth()`) and must be
//! a member of the key's multi-sig admin set, configured with
//! `set_multisig_admins`. The protocol admin is accepted as a governance
//! override so a key can always be released, even when the multi-sig set has
//! been rotated out.
//!
//! ### Trade enforcement
//!
//! Every trade path that moves a position checks the freeze status before it
//! touches any balance, so a frozen key rejects buys, sells, transfers, batch
//! trades, swaps and dividend reinvestments with
//! [`ContractError::KeyFrozen`].

use crate::{
    constants, events, extend_key_ttl_to_full_window, read_registered_creator_profile,
    ContractError, CreatorKeysContract, CreatorKeysContractArgs, CreatorKeysContractClient,
    MultisigAdmins,
};
use soroban_sdk::{contractimpl, contracttype, Address, Env, String, Vec};

/// Lifecycle status of a key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum KeyStatus {
    /// The key trades normally.
    Active = 0,
    /// The key is frozen: buys, sells and transfers are rejected.
    Frozen = 1,
}

/// The action recorded in the freeze audit log.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum FreezeAction {
    /// The key was frozen.
    Freeze = 0,
    /// The key was released.
    Unfreeze = 1,
}

/// One entry of the append-only freeze audit log.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct FreezeLogEntry {
    /// Whether the key was frozen or released by this action.
    pub action: FreezeAction,
    /// Human-readable reason supplied by the admin. Empty for an unfreeze.
    pub reason: String,
    /// The admin that authorized the action.
    pub admin_wallet: Address,
    /// Ledger timestamp of the action, in seconds.
    pub timestamp: u64,
}

/// Storage keys for the key-freeze feature (#1174).
///
/// Kept out of [`crate::DataKey`] so the main key enum does not grow, following
/// the same convention as the other feature modules (`EmergencyPauseDataKey`,
/// `OwnershipDataKey`, `AclDataKey`). All entries are persistent and extended
/// to the full TTL window on write.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum KeyFreezeDataKey {
    /// `key_id` -> current [`KeyStatus`]. Absent means the key is active.
    Status(Address),
    /// `key_id` -> append-only [`FreezeLogEntry`] audit log.
    Log(Address),
}

/// Read-only: the current status of `key_id`.
///
/// A key that was never frozen is [`KeyStatus::Active`].
pub fn read_key_status(env: &Env, key_id: &Address) -> KeyStatus {
    env.storage()
        .persistent()
        .get(&KeyFreezeDataKey::Status(key_id.clone()))
        .unwrap_or(KeyStatus::Active)
}

/// Read-only: whether `key_id` is currently frozen.
pub fn is_key_frozen(env: &Env, key_id: &Address) -> bool {
    read_key_status(env, key_id) == KeyStatus::Frozen
}

/// Rejects a trade on a frozen key.
///
/// Called from every buy, sell and transfer path before any balance is touched,
/// so a frozen key rejects the trade without side effects.
pub fn assert_key_not_frozen(env: &Env, key_id: &Address) -> Result<(), ContractError> {
    if is_key_frozen(env, key_id) {
        return Err(ContractError::KeyFrozen);
    }
    Ok(())
}

/// Read-only: the append-only audit log of freeze and unfreeze actions.
///
/// Returns an empty log for a key that was never frozen.
pub fn read_freeze_log(env: &Env, key_id: &Address) -> Vec<FreezeLogEntry> {
    env.storage()
        .persistent()
        .get(&KeyFreezeDataKey::Log(key_id.clone()))
        .unwrap_or(Vec::new(env))
}

/// Persists the status of `key_id`.
fn write_key_status(env: &Env, key_id: &Address, status: KeyStatus) {
    let key = KeyFreezeDataKey::Status(key_id.clone());
    env.storage().persistent().set(&key, &status);
    extend_key_ttl_to_full_window(env, &key);
}

/// Appends `entry` to the audit log of `key_id`.
///
/// The log is append-only: entries are never rewritten or removed, so the
/// history of a key can always be replayed.
fn append_freeze_log_entry(env: &Env, key_id: &Address, entry: &FreezeLogEntry) {
    let key = KeyFreezeDataKey::Log(key_id.clone());
    let mut log = read_freeze_log(env, key_id);
    log.push_back(entry.clone());
    env.storage().persistent().set(&key, &log);
    extend_key_ttl_to_full_window(env, &key);
}

/// Asserts that `admin` may freeze or unfreeze `key_id`.
///
/// The admin must be a member of the key's multi-sig admin set (configured
/// with `set_multisig_admins`) or the protocol admin. The caller is
/// responsible for requiring the admin's authorization.
fn assert_freeze_authority(
    env: &Env,
    key_id: &Address,
    admin: &Address,
) -> Result<(), ContractError> {
    let config: Option<MultisigAdmins> = env
        .storage()
        .persistent()
        .get(&constants::storage::multisig_admins(key_id));

    if let Some(config) = config {
        if config.admins.iter().any(|member| member == *admin) {
            return Ok(());
        }
    }

    // The protocol admin keeps a governance override so a key can always be
    // released, even after the multi-sig set has been rotated.
    if crate::assert_is_admin(env, admin).is_ok() {
        return Ok(());
    }

    Err(ContractError::Unauthorized)
}

/// Freezes trading for `key_id` and records the action in the audit log.
///
/// # Access
///
/// `admin` authorizes the call (`admin.require_auth()`) and must be a member
/// of the key's multi-sig admin set or the protocol admin.
///
/// # Validation
///
/// - `key_id` must be a registered key, otherwise
///   [`ContractError::NotRegistered`].
/// - The key must not already be frozen, otherwise
///   [`ContractError::KeyAlreadyFrozen`].
pub fn apply_freeze(
    env: &Env,
    key_id: &Address,
    reason: &String,
    admin: &Address,
) -> Result<(), ContractError> {
    admin.require_auth();
    read_registered_creator_profile(env, key_id)?;
    assert_freeze_authority(env, key_id, admin)?;

    if is_key_frozen(env, key_id) {
        return Err(ContractError::KeyAlreadyFrozen);
    }

    let timestamp = env.ledger().timestamp();
    let entry = FreezeLogEntry {
        action: FreezeAction::Freeze,
        reason: reason.clone(),
        admin_wallet: admin.clone(),
        timestamp,
    };

    append_freeze_log_entry(env, key_id, &entry);
    write_key_status(env, key_id, KeyStatus::Frozen);

    env.events().publish(
        events::key_frozen_topics(key_id),
        events::KeyFrozenEvent {
            key_id: key_id.clone(),
            admin_wallet: admin.clone(),
            reason: reason.clone(),
            timestamp,
            ledger: env.ledger().sequence(),
        },
    );

    Ok(())
}

/// Releases the freeze on `key_id` and records the action in the audit log.
///
/// # Access
///
/// `admin` authorizes the call (`admin.require_auth()`) and must be a member
/// of the key's multi-sig admin set or the protocol admin.
///
/// # Validation
///
/// - `key_id` must be a registered key, otherwise
///   [`ContractError::NotRegistered`].
/// - The key must be frozen, otherwise [`ContractError::KeyNotFrozen`].
pub fn apply_unfreeze(env: &Env, key_id: &Address, admin: &Address) -> Result<(), ContractError> {
    admin.require_auth();
    read_registered_creator_profile(env, key_id)?;
    assert_freeze_authority(env, key_id, admin)?;

    if !is_key_frozen(env, key_id) {
        return Err(ContractError::KeyNotFrozen);
    }

    let timestamp = env.ledger().timestamp();
    let entry = FreezeLogEntry {
        action: FreezeAction::Unfreeze,
        reason: String::from_str(env, ""),
        admin_wallet: admin.clone(),
        timestamp,
    };

    append_freeze_log_entry(env, key_id, &entry);
    write_key_status(env, key_id, KeyStatus::Active);

    env.events().publish(
        events::key_unfrozen_topics(key_id),
        events::KeyUnfrozenEvent {
            key_id: key_id.clone(),
            admin_wallet: admin.clone(),
            timestamp,
            ledger: env.ledger().sequence(),
        },
    );

    Ok(())
}

#[contractimpl]
impl CreatorKeysContract {
    /// Freezes trading for `key_id` immediately.
    ///
    /// While the key is frozen every buy, sell and transfer of that key is
    /// rejected with [`ContractError::KeyFrozen`]. The action is appended to
    /// the audit log returned by [`Self::get_freeze_log`].
    ///
    /// # Access
    ///
    /// `admin` authorizes the call (`admin.require_auth()`) and must be a
    /// member of the key's multi-sig admin set (see `set_multisig_admins`) or
    /// the protocol admin.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotRegistered`] if `key_id` is not a registered key.
    /// - [`ContractError::Unauthorized`] if `admin` may not freeze the key.
    /// - [`ContractError::KeyAlreadyFrozen`] if the key is already frozen.
    pub fn freeze(
        env: Env,
        key_id: Address,
        reason: String,
        admin: Address,
    ) -> Result<(), ContractError> {
        apply_freeze(&env, &key_id, &reason, &admin)
    }

    /// Releases the freeze on `key_id`, restoring buy, sell and transfer.
    ///
    /// # Access
    ///
    /// `admin` authorizes the call (`admin.require_auth()`) and must be a
    /// member of the key's multi-sig admin set (see `set_multisig_admins`) or
    /// the protocol admin.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotRegistered`] if `key_id` is not a registered key.
    /// - [`ContractError::Unauthorized`] if `admin` may not unfreeze the key.
    /// - [`ContractError::KeyNotFrozen`] if the key is not frozen.
    pub fn unfreeze(env: Env, key_id: Address, admin: Address) -> Result<(), ContractError> {
        apply_unfreeze(&env, &key_id, &admin)
    }

    /// Alias of [`Self::freeze`] with the admin argument first, matching the
    /// argument order used by the other per-key admin entrypoints.
    pub fn freeze_key(
        env: Env,
        admin: Address,
        key_id: Address,
        reason: String,
    ) -> Result<(), ContractError> {
        apply_freeze(&env, &key_id, &reason, &admin)
    }

    /// Alias of [`Self::unfreeze`] with the admin argument first, matching the
    /// argument order used by the other per-key admin entrypoints.
    pub fn unfreeze_key(env: Env, admin: Address, key_id: Address) -> Result<(), ContractError> {
        apply_unfreeze(&env, &key_id, &admin)
    }

    /// Read-only view: the full freeze audit log of `key_id`.
    ///
    /// The log is append-only and holds one entry per freeze and unfreeze
    /// action, in the order the actions happened.
    pub fn get_freeze_log(env: Env, key_id: Address) -> Vec<FreezeLogEntry> {
        read_freeze_log(&env, &key_id)
    }

    /// Read-only view: the current status of `key_id`.
    ///
    /// A key that was never frozen reports [`KeyStatus::Active`].
    pub fn get_key_status(env: Env, key_id: Address) -> KeyStatus {
        read_key_status(&env, &key_id)
    }

    /// Read-only view: whether `key_id` is currently frozen.
    pub fn is_key_frozen(env: Env, key_id: Address) -> bool {
        is_key_frozen(&env, &key_id)
    }
}
