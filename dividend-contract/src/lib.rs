//! #1073 — Snapshot-based dividend distribution for creator revenue sharing.
//!
//! Lets a creator distribute revenue to their key holders pro-rata to holdings
//! using a snapshot-based flow that is immune to flash-purchase gaming:
//!
//! 1. **Snapshot** — the creator calls [`DividendDistributionContract::snapshot_holders`].
//!    The contract reads the key's holder registry and every holder's current
//!    balance from the `creator-keys` contract and freezes them in a
//!    `DividendSnapshot` record at the current ledger. Purchases made *after*
//!    this point cannot change anyone's entitlement in this snapshot.
//! 2. **Fund** — the creator deposits the distribution amount with
//!    [`DividendDistributionContract::fund_dividend`]. Funding may happen at
//!    any time after the snapshot; entitlements are locked by the snapshot
//!    balances, not by balances at funding time.
//! 3. **Claim** — each holder pulls their pro-rata share with
//!    [`DividendDistributionContract::claim_dividend`]:
//!    `share = funded_amount * holder_balance / total_supply` (floor division).
//!    A holder can claim each snapshot exactly once; a second attempt reverts
//!    with [`DividendError::AlreadyClaimed`].
//!
//! Views: [`DividendDistributionContract::get_unclaimed_dividend`] returns a
//! holder's claimable amount for a snapshot, and several `get_*` helpers expose
//! snapshot metadata, per-holder snapshot balances and claim state.
//!
//! Two events are emitted: `DividendSnapshotCreated` (snapshot id, key,
//! creator, total supply, holder count and snapshot ledger) when a snapshot is
//! taken, and `DividendClaimed` (snapshot id, holder, amount) when a holder
//! claims.
//!
//! Consistent with the rest of the protocol (see `distribute_dividend_claimable`
//! in `creator-keys`), dividend amounts are internal-ledger accounting: no
//! token moves inside this contract, and every paid amount is recorded in the
//! `PaidAmount` ledger and announced in the `DividendClaimed` event so
//! downstream settlement can be reconciled from events alone.
//!
//! Note on rounding: shares are computed with floor integer division, so a
//! sub-unit remainder ("dust") stays unallocated in the funded pool.

#![no_std]

use creator_keys::CREATOR_TTL_LEDGERS;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

#[cfg(test)]
mod test;

/// Event emitted when a creator snapshots their key holders (#1073).
pub const DIVIDEND_SNAPSHOT_CREATED_EVENT: Symbol = symbol_short!("div_snap");

/// Event emitted when a holder claims their pro-rata dividend (#1073).
pub const DIVIDEND_CLAIMED_EVENT: Symbol = symbol_short!("div_claim");

/// Event emitted when a creator funds a snapshot's distribution pool (#1073).
pub const DIVIDEND_FUNDED_EVENT: Symbol = symbol_short!("div_fund");

/// Upper bound on the number of holders a single snapshot can record.
///
/// Mirrors `creator-keys::MAX_SNAPSHOT_HOLDERS`: reading the holder registry
/// and one balance per holder happens in a single invocation, so the set is
/// bounded to keep the snapshot call safely inside the Soroban instruction
/// budget. Keys with more holders need an off-chain/indexer flow.
pub const MAX_SNAPSHOT_HOLDERS: u32 = 100;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DividendError {
    /// `initialize` was never called, or the snapshot id does not exist.
    NotInitialized = 1,
    /// `initialize` was called a second time.
    AlreadyInitialized = 2,
    /// The caller is not the creator owning the key (or not the snapshot's creator).
    Unauthorized = 3,
    /// A supplied amount was zero or negative where a positive value is required.
    NotPositiveAmount = 4,
    /// No snapshot with that id exists.
    SnapshotNotFound = 5,
    /// The holder already claimed this snapshot; each snapshot is claimable once.
    AlreadyClaimed = 6,
    /// The caller has nothing to claim from the snapshot (zero balance, no
    /// funding, or not part of the snapshot).
    NothingToClaim = 7,
    /// The key has no recorded holders, so a snapshot would be empty.
    NoKeyHolders = 8,
    /// The key's holder registry is larger than [`MAX_SNAPSHOT_HOLDERS`].
    HolderLimitExceeded = 9,
    /// Arithmetic overflow while accumulating balances or amounts.
    Overflow = 10,
}

/// Frozen record of one distribution round (#1073).
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendSnapshot {
    /// Globally unique snapshot id; also the storage key for its details.
    pub snapshot_id: u32,
    /// The creator key this snapshot belongs to.
    pub key_id: Address,
    /// The creator that took the snapshot (and may fund it).
    pub creator: Address,
    /// Ledger sequence number at which holder balances were recorded.
    pub snapshot_ledger: u32,
    /// Sum of the recorded holder balances (= circulating supply at snapshot time).
    pub total_supply: u32,
    /// Number of holder entries recorded in the snapshot.
    pub holder_count: u32,
    /// Total amount funded into this snapshot's distribution pool so far.
    pub funded_amount: i128,
    /// Total amount claimed by holders so far.
    pub total_claimed: i128,
}

/// Storage keys for the dividend distribution contract (#1073).
///
/// Kept out of `creator-keys::DataKey` because this is a standalone contract
/// with its own storage. All entries are persistent and extended to the full
/// protocol TTL window (`creator_keys::CREATOR_TTL_LEDGERS`) on write, so
/// snapshots stay claimable for the same horizon as creator state.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    /// Contract admin set at initialization.
    Admin,
    /// Address of the deployed `creator-keys` contract instance whose holder
    /// registry and balances this contract snapshots.
    CreatorKeysContract,
    /// Next snapshot id to issue -> `u32`; ids start at `1`.
    NextSnapshotId,
    /// `key_id` -> id of the most recent snapshot for that key (funding target).
    LatestSnapshot(Address),
    /// `snapshot_id` -> [`DividendSnapshot`] record.
    Snapshot(u32),
    /// `snapshot_id` -> `Vec<Address>` of the snapshotted holders.
    SnapshotHolders(u32),
    /// `(snapshot_id, holder)` -> balance recorded at snapshot time.
    HolderBalance(u32, Address),
    /// `(snapshot_id, holder)` -> whether the holder has claimed.
    Claimed(u32, Address),
    /// `(snapshot_id, holder)` -> amount settled to the holder on claim.
    PaidAmount(u32, Address),
    /// Global sum of funded amounts across all snapshots -> `i128`.
    TotalFunded,
    /// Global sum of claimed amounts across all snapshots -> `i128`.
    TotalClaimed,
}

/// Extends a persistent entry to the full protocol TTL window so snapshot
/// state stays live between rounds, mirroring the TTL policy of the
/// `creator-keys` contract. Extending a missing entry is a runtime no-op.
fn extend_ttl(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, CREATOR_TTL_LEDGERS, CREATOR_TTL_LEDGERS);
}

/// Reads the stored `creator-keys` contract address, or `NotInitialized`.
fn read_creator_keys_contract(env: &Env) -> Result<Address, DividendError> {
    env.storage()
        .persistent()
        .get(&DataKey::CreatorKeysContract)
        .ok_or(DividendError::NotInitialized)
}

/// Reads a snapshot record by id, or `SnapshotNotFound`.
fn read_snapshot(env: &Env, snapshot_id: u32) -> Result<DividendSnapshot, DividendError> {
    env.storage()
        .persistent()
        .get(&DataKey::Snapshot(snapshot_id))
        .ok_or(DividendError::SnapshotNotFound)
}

/// Persists a snapshot record and grants it the full TTL window.
fn write_snapshot(env: &Env, snapshot: &DividendSnapshot) {
    let key = DataKey::Snapshot(snapshot.snapshot_id);
    env.storage().persistent().set(&key, snapshot);
    extend_ttl(env, &key);
}

#[contract]
pub struct DividendDistributionContract;

#[contractimpl]
impl DividendDistributionContract {
    /// One-time setup: records the admin and the deployed `creator-keys`
    /// contract whose holder registry and balances are snapshotted.
    pub fn initialize(
        env: Env,
        admin: Address,
        creator_keys_contract: Address,
    ) -> Result<(), DividendError> {
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::Admin) {
            return Err(DividendError::AlreadyInitialized);
        }

        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::CreatorKeysContract, &creator_keys_contract);
        extend_ttl(&env, &DataKey::Admin);
        extend_ttl(&env, &DataKey::CreatorKeysContract);

        Ok(())
    }

    /// Records holder balances for `key_id` at the current ledger and returns
    /// the new snapshot id (#1073).
    ///
    /// Creator-only: `creator` must be the registered creator of `key_id` in
    /// the `creator-keys` contract. The holder registry and every holder's
    /// balance are read from `creator-keys` at call time and frozen into the
    /// snapshot, so key purchases made after this call cannot change any
    /// holder's entitlement in this snapshot — the flash-purchase vector the
    /// snapshot mechanism exists to close.
    ///
    /// # Errors
    /// - [`DividendError::NotInitialized`] if `initialize` was never called.
    /// - [`DividendError::Unauthorized`] if the caller is not the key's creator.
    /// - [`DividendError::NoKeyHolders`] if the key has no recorded holders.
    /// - [`DividendError::HolderLimitExceeded`] if the holder registry exceeds
    ///   [`MAX_SNAPSHOT_HOLDERS`].
    /// - [`DividendError::Overflow`] on balance accumulation overflow.
    pub fn snapshot_holders(
        env: Env,
        creator: Address,
        key_id: Address,
    ) -> Result<u32, DividendError> {
        creator.require_auth();

        let creator_keys_address = read_creator_keys_contract(&env)?;
        let keys_client = creator_keys::CreatorKeysContractClient::new(&env, &creator_keys_address);

        // The registry is the canonical holder set maintained by `creator-keys`
        // (every wallet that has ever held keys for this creator).
        let holders: Vec<Address> = keys_client.get_holder_registry(&key_id);
        if holders.is_empty() {
            return Err(DividendError::NoKeyHolders);
        }
        if holders.len() > MAX_SNAPSHOT_HOLDERS {
            return Err(DividendError::HolderLimitExceeded);
        }

        let next_id: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::NextSnapshotId)
            .unwrap_or(1);

        let snapshot_ledger = env.ledger().sequence();
        let mut total_supply: u32 = 0;
        for holder in holders.iter() {
            let balance: u32 = keys_client.get_balance(&key_id, &holder);
            total_supply = total_supply
                .checked_add(balance)
                .ok_or(DividendError::Overflow)?;

            let balance_key = DataKey::HolderBalance(next_id, holder.clone());
            env.storage().persistent().set(&balance_key, &balance);
            extend_ttl(&env, &balance_key);
        }

        if total_supply == 0 {
            return Err(DividendError::NoKeyHolders);
        }

        let holder_count = holders.len();
        let snapshot = DividendSnapshot {
            snapshot_id: next_id,
            key_id: key_id.clone(),
            creator: creator.clone(),
            snapshot_ledger,
            total_supply,
            holder_count,
            funded_amount: 0,
            total_claimed: 0,
        };

        let holders_key = DataKey::SnapshotHolders(next_id);
        env.storage().persistent().set(&holders_key, &holders);
        extend_ttl(&env, &holders_key);

        write_snapshot(&env, &snapshot);

        let latest_key = DataKey::LatestSnapshot(key_id.clone());
        env.storage().persistent().set(&latest_key, &next_id);
        extend_ttl(&env, &latest_key);

        let next_id_key = DataKey::NextSnapshotId;
        env.storage().persistent().set(&next_id_key, &(next_id + 1));
        extend_ttl(&env, &next_id_key);

        env.events().publish(
            (DIVIDEND_SNAPSHOT_CREATED_EVENT, key_id.clone()),
            DividendSnapshotCreatedEvent {
                snapshot_id: next_id,
                key_id,
                creator,
                total_supply,
                holder_count,
                snapshot_ledger,
            },
        );

        Ok(next_id)
    }

    /// Funds the most recent snapshot of `key_id` with `amount` (#1073).
    ///
    /// Creator-only. Funding accumulates across calls, so a creator can top up
    /// a distribution round incrementally. Entitlements are fixed by the
    /// snapshot balances, so funding later never dilutes or re-prorates
    /// holders. Amounts are internal-ledger accounting, consistent with the
    /// rest of the protocol's dividend flows.
    ///
    /// Returns the snapshot's new cumulative funded amount.
    ///
    /// # Errors
    /// - [`DividendError::NotInitialized`] if `initialize` was never called.
    /// - [`DividendError::SnapshotNotFound`] if the key has no snapshot yet.
    /// - [`DividendError::Unauthorized`] if the caller is not the snapshot's creator.
    /// - [`DividendError::NotPositiveAmount`] if `amount <= 0`.
    /// - [`DividendError::Overflow`] on accumulation overflow.
    pub fn fund_dividend(
        env: Env,
        creator: Address,
        key_id: Address,
        amount: i128,
    ) -> Result<i128, DividendError> {
        creator.require_auth();

        if amount <= 0 {
            return Err(DividendError::NotPositiveAmount);
        }

        let snapshot_id: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::LatestSnapshot(key_id.clone()))
            .ok_or(DividendError::SnapshotNotFound)?;

        let mut snapshot = read_snapshot(&env, snapshot_id)?;
        if snapshot.creator != creator || snapshot.key_id != key_id {
            return Err(DividendError::Unauthorized);
        }

        let funded = snapshot
            .funded_amount
            .checked_add(amount)
            .ok_or(DividendError::Overflow)?;
        snapshot.funded_amount = funded;
        write_snapshot(&env, &snapshot);

        let total_key = DataKey::TotalFunded;
        let total: i128 = env.storage().persistent().get(&total_key).unwrap_or(0);
        let new_total = total.checked_add(amount).ok_or(DividendError::Overflow)?;
        env.storage().persistent().set(&total_key, &new_total);
        extend_ttl(&env, &total_key);

        env.events().publish(
            (DIVIDEND_FUNDED_EVENT, key_id.clone()),
            DividendFundedEvent {
                snapshot_id,
                key_id,
                amount,
                funded_amount: funded,
            },
        );

        Ok(funded)
    }

    /// Claims `holder`'s pro-rata share of snapshot `snapshot_id` (#1073).
    ///
    /// The share is `funded_amount * snapshot_balance / total_supply` (floor
    /// division), using the balances frozen when the snapshot was taken — so
    /// key purchases made after the snapshot never increase the claim. Each
    /// holder can claim a snapshot exactly once; a second call reverts with
    /// [`DividendError::AlreadyClaimed`].
    ///
    /// Returns the claimed amount.
    ///
    /// # Errors
    /// - [`DividendError::SnapshotNotFound`] if the snapshot does not exist.
    /// - [`DividendError::AlreadyClaimed`] on a second claim of the same snapshot.
    /// - [`DividendError::NothingToClaim`] if the holder's snapshot balance is
    ///   zero or the snapshot has not been funded.
    /// - [`DividendError::Overflow`] on amount overflow.
    pub fn claim_dividend(
        env: Env,
        holder: Address,
        snapshot_id: u32,
    ) -> Result<i128, DividendError> {
        holder.require_auth();

        let mut snapshot = read_snapshot(&env, snapshot_id)?;

        let claimed_key = DataKey::Claimed(snapshot_id, holder.clone());
        let already_claimed: bool = env
            .storage()
            .persistent()
            .get(&claimed_key)
            .unwrap_or(false);
        if already_claimed {
            return Err(DividendError::AlreadyClaimed);
        }

        let balance_key = DataKey::HolderBalance(snapshot_id, holder.clone());
        let holder_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if holder_balance == 0 || snapshot.total_supply == 0 || snapshot.funded_amount == 0 {
            return Err(DividendError::NothingToClaim);
        }

        let share = snapshot
            .funded_amount
            .checked_mul(holder_balance as i128)
            .ok_or(DividendError::Overflow)?
            / snapshot.total_supply as i128;
        if share == 0 {
            return Err(DividendError::NothingToClaim);
        }

        env.storage().persistent().set(&claimed_key, &true);
        extend_ttl(&env, &claimed_key);

        let paid_key = DataKey::PaidAmount(snapshot_id, holder.clone());
        env.storage().persistent().set(&paid_key, &share);
        extend_ttl(&env, &paid_key);

        snapshot.total_claimed = snapshot
            .total_claimed
            .checked_add(share)
            .ok_or(DividendError::Overflow)?;
        write_snapshot(&env, &snapshot);

        let total_key = DataKey::TotalClaimed;
        let total: i128 = env.storage().persistent().get(&total_key).unwrap_or(0);
        let new_total = total.checked_add(share).ok_or(DividendError::Overflow)?;
        env.storage().persistent().set(&total_key, &new_total);
        extend_ttl(&env, &total_key);

        env.events().publish(
            (DIVIDEND_CLAIMED_EVENT, holder.clone()),
            DividendClaimedEvent {
                snapshot_id,
                holder,
                amount: share,
            },
        );

        Ok(share)
    }

    /// Read-only view: the amount `wallet` can still claim from snapshot
    /// `snapshot_id` (#1073).
    ///
    /// Returns `0` when the snapshot does not exist, the wallet holds no keys
    /// in the snapshot, the snapshot is unfunded, or the wallet has already
    /// claimed.
    pub fn get_unclaimed_dividend(env: Env, wallet: Address, snapshot_id: u32) -> i128 {
        let snapshot: DividendSnapshot = match env
            .storage()
            .persistent()
            .get(&DataKey::Snapshot(snapshot_id))
        {
            Some(snapshot) => snapshot,
            None => return 0,
        };

        let claimed_key = DataKey::Claimed(snapshot_id, wallet.clone());
        let already_claimed: bool = env
            .storage()
            .persistent()
            .get(&claimed_key)
            .unwrap_or(false);
        if already_claimed {
            return 0;
        }

        let balance_key = DataKey::HolderBalance(snapshot_id, wallet);
        let holder_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if holder_balance == 0 || snapshot.total_supply == 0 {
            return 0;
        }

        snapshot.funded_amount * holder_balance as i128 / snapshot.total_supply as i128
    }

    /// Read-only view: the snapshot record for `snapshot_id`.
    pub fn get_snapshot(env: Env, snapshot_id: u32) -> Result<DividendSnapshot, DividendError> {
        read_snapshot(&env, snapshot_id)
    }

    /// Read-only view: the holders recorded in snapshot `snapshot_id`.
    pub fn get_snapshot_holders(env: Env, snapshot_id: u32) -> Result<Vec<Address>, DividendError> {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Snapshot(snapshot_id))
        {
            return Err(DividendError::SnapshotNotFound);
        }
        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::SnapshotHolders(snapshot_id))
            .unwrap_or_else(|| Vec::new(&env)))
    }

    /// Read-only view: `holder`'s balance as recorded in snapshot `snapshot_id`
    /// (0 when the holder is not part of the snapshot).
    pub fn get_snapshot_holder_balance(env: Env, snapshot_id: u32, holder: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::HolderBalance(snapshot_id, holder))
            .unwrap_or(0)
    }

    /// Read-only view: whether `holder` has already claimed snapshot `snapshot_id`.
    pub fn has_claimed(env: Env, snapshot_id: u32, holder: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Claimed(snapshot_id, holder))
            .unwrap_or(false)
    }

    /// Read-only view: the amount already settled to `holder` for snapshot
    /// `snapshot_id` (0 when unclaimed).
    pub fn get_paid_amount(env: Env, snapshot_id: u32, holder: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::PaidAmount(snapshot_id, holder))
            .unwrap_or(0)
    }

    /// Read-only view: id of the most recent snapshot for `key_id` (0 when the
    /// key has never been snapshotted).
    pub fn get_latest_snapshot_id(env: Env, key_id: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::LatestSnapshot(key_id))
            .unwrap_or(0)
    }

    /// Read-only view: total amount funded across all snapshots.
    pub fn get_total_funded(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalFunded)
            .unwrap_or(0)
    }

    /// Read-only view: total amount claimed across all snapshots.
    pub fn get_total_claimed(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalClaimed)
            .unwrap_or(0)
    }

    /// Read-only view: the contract admin.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }

    /// Read-only view: the `creator-keys` contract this contract snapshots.
    pub fn get_creator_keys_contract(env: Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::CreatorKeysContract)
    }
}

/// Emitted with [`DIVIDEND_SNAPSHOT_CREATED_EVENT`] when a snapshot is taken.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendSnapshotCreatedEvent {
    /// Id of the newly created snapshot.
    pub snapshot_id: u32,
    /// The creator key whose holders were snapshotted.
    pub key_id: Address,
    /// The creator that took the snapshot.
    pub creator: Address,
    /// Sum of snapshotted holder balances.
    pub total_supply: u32,
    /// Number of holders recorded.
    pub holder_count: u32,
    /// Ledger at which balances were recorded.
    pub snapshot_ledger: u32,
}

/// Emitted with [`DIVIDEND_CLAIMED_EVENT`] when a holder claims.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendClaimedEvent {
    /// Snapshot that was claimed.
    pub snapshot_id: u32,
    /// Holder that claimed.
    pub holder: Address,
    /// Amount settled to the holder.
    pub amount: i128,
}

/// Emitted with [`DIVIDEND_FUNDED_EVENT`] when a creator funds a snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendFundedEvent {
    /// Snapshot that was funded.
    pub snapshot_id: u32,
    /// The creator key the snapshot belongs to.
    pub key_id: Address,
    /// Amount added in this call.
    pub amount: i128,
    /// The snapshot's cumulative funded amount after this call.
    pub funded_amount: i128,
}
