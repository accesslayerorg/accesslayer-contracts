#![no_std]

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, token::TokenClient,
    Address, Env, Vec,
};

pub mod events;

#[cfg(test)]
mod test;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum number of non-zero holder balances snapshotted per airdrop.
///
/// The holder registry is append-only and can grow without bound, so a single
/// `create_airdrop` call is capped the same way creator-keys bounds
/// `take_snapshot` and `airdrop_keys`. A key with more holders than this
/// needs its rewards split across separate airdrops.
pub const MAX_AIRDROP_HOLDERS: u32 = 100;

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
/// Contract error variants.
///
/// **IMPORTANT**: append only — never insert or reorder. Discriminants are ABI.
pub enum ContractError {
    /// Contract has already been initialized.
    AlreadyInitialized = 1,
    /// Contract has not been initialized.
    NotInitialized = 2,
    /// Caller is neither the key creator nor the admin.
    Unauthorized = 3,
    /// `total_amount` is not greater than zero.
    NotPositiveAmount = 4,
    /// No airdrop exists with the given id.
    AirdropNotFound = 5,
    /// The wallet has already claimed this airdrop.
    AlreadyClaimed = 6,
    /// The wallet held no keys at the snapshot block and can claim nothing.
    NoAllocation = 7,
    /// The key has more non-zero holders than [`MAX_AIRDROP_HOLDERS`].
    HolderLimitExceeded = 8,
    /// Arithmetic overflow.
    Overflow = 9,
    /// The key had no holders with a non-zero balance at the snapshot block.
    NoHolders = 10,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Protocol admin.
    Admin,
    /// Address of the creator-keys contract used to read holder balances.
    CreatorKeys,
    /// Monotonic airdrop id counter.
    NextAirdropId,
    /// Airdrop record: `DataKey::Airdrop(airdrop_id)`.
    Airdrop(u64),
    /// Snapshot key balance: `DataKey::SnapshotBalance(airdrop_id, wallet)`.
    SnapshotBalance(u64, Address),
    /// Whether a wallet has claimed an airdrop: `DataKey::Claimed(airdrop_id, wallet)`.
    Claimed(u64, Address),
}

// ── Data types ────────────────────────────────────────────────────────────────

/// Full record of a created airdrop, including its holder snapshot metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Airdrop {
    pub airdrop_id: u64,
    pub key_id: Address,
    /// Wallet (creator or admin) that funded the airdrop.
    pub funder: Address,
    /// SEP-41 token contract address paying out the rewards.
    pub reward_token: Address,
    /// Total reward tokens funded, distributed pro-rata across the snapshot.
    pub total_amount: i128,
    /// Sum of snapshot key balances across holders with a non-zero balance.
    pub total_snapshot_supply: u32,
    /// Ledger sequence number at which holder balances were snapshotted.
    pub snapshot_ledger: u32,
    /// Rewards claimed so far across all holders.
    pub total_claimed: i128,
}

/// Claimable and claimed amounts for a wallet on a given airdrop.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AirdropAllocation {
    /// Unclaimed pro-rata share (0 once fully claimed).
    pub claimable: i128,
    /// Amount the wallet has already claimed (0 before claiming).
    pub claimed: i128,
}

// ── Creator-keys read interface ───────────────────────────────────────────────

/// Read-only subset of the creator-keys contract this contract depends on.
///
/// Implemented by the deployed creator-keys contract; the address is stored
/// at initialization. Only view methods are used — no creator-keys state is
/// ever written from here.
#[contractclient(name = "CreatorKeysReader")]
pub trait CreatorKeysInterface {
    /// Every wallet that has ever held keys for `key_id` (append-only;
    /// zero-balance wallets are retained).
    fn get_holder_registry(env: Env, key_id: Address) -> Vec<Address>;
    /// Current key balance of `wallet` for `creator`.
    fn get_key_balance(env: Env, creator: Address, wallet: Address) -> u32;
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn read_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::Admin)
}

fn read_creator_keys(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::CreatorKeys)
}

fn read_airdrop(env: &Env, airdrop_id: u64) -> Option<Airdrop> {
    env.storage()
        .persistent()
        .get(&DataKey::Airdrop(airdrop_id))
}

fn write_airdrop(env: &Env, airdrop: &Airdrop) {
    env.storage()
        .persistent()
        .set(&DataKey::Airdrop(airdrop.airdrop_id), airdrop);
}

fn read_snapshot_balance(env: &Env, airdrop_id: u64, wallet: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::SnapshotBalance(airdrop_id, wallet.clone()))
        .unwrap_or(0)
}

fn write_snapshot_balance(env: &Env, airdrop_id: u64, wallet: &Address, balance: u32) {
    env.storage().persistent().set(
        &DataKey::SnapshotBalance(airdrop_id, wallet.clone()),
        &balance,
    );
}

fn has_claimed(env: &Env, airdrop_id: u64, wallet: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Claimed(airdrop_id, wallet.clone()))
        .unwrap_or(false)
}

fn mark_claimed(env: &Env, airdrop_id: u64, wallet: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::Claimed(airdrop_id, wallet.clone()), &true);
}

/// Pro-rata share of `airdrop` for `wallet`, computed exclusively from the
/// balances frozen in the snapshot at airdrop creation. Post-snapshot balance
/// changes in the creator-keys contract do not affect the result.
fn allocation_for(env: &Env, airdrop: &Airdrop, wallet: &Address) -> Result<i128, ContractError> {
    let balance = read_snapshot_balance(env, airdrop.airdrop_id, wallet);
    if balance == 0 {
        return Ok(0);
    }
    let numerator = airdrop
        .total_amount
        .checked_mul(balance as i128)
        .ok_or(ContractError::Overflow)?;
    let denominator = airdrop.total_snapshot_supply as i128;
    Ok(numerator / denominator)
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct HolderRewardsContract;

#[contractimpl]
impl HolderRewardsContract {
    // ── Bootstrap ─────────────────────────────────────────────────────────────

    /// Initializes the contract with an admin and the creator-keys contract
    /// address used to snapshot holder balances.
    pub fn init(
        env: Env,
        admin: Address,
        creator_keys_contract: Address,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::CreatorKeys, &creator_keys_contract);
        env.storage().instance().set(&DataKey::NextAirdropId, &1u64);

        Ok(())
    }

    /// Returns the protocol admin address, if initialized.
    pub fn get_admin(env: Env) -> Option<Address> {
        read_admin(&env)
    }

    /// Updates the admin address. Requires authorization from the current admin.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        let admin = read_admin(&env).ok_or(ContractError::NotInitialized)?;
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }

    // ── Airdrop lifecycle ─────────────────────────────────────────────────────

    /// Creates an airdrop for `key_id` funded by `caller`, snapshotting every
    /// current holder's key balance at the creation ledger.
    ///
    /// `caller` must be the key's creator (`key_id` itself, matching the
    /// creator-keys convention) or the protocol admin. `total_amount` of
    /// `reward_token` is transferred from `caller` into this contract and paid
    /// out to holders pro-rata to their snapshot balances when they claim.
    ///
    /// Returns the unique `airdrop_id`.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotInitialized`] if `init` has not been called.
    /// - [`ContractError::Unauthorized`] if `caller` is neither the creator
    ///   nor the admin.
    /// - [`ContractError::NotPositiveAmount`] if `total_amount <= 0`.
    /// - [`ContractError::HolderLimitExceeded`] if the key has more than
    ///   [`MAX_AIRDROP_HOLDERS`] non-zero holder balances.
    /// - [`ContractError::NoHolders`] if no holder has a non-zero balance.
    pub fn create_airdrop(
        env: Env,
        key_id: Address,
        caller: Address,
        reward_token: Address,
        total_amount: i128,
    ) -> Result<u64, ContractError> {
        caller.require_auth();

        let admin = read_admin(&env).ok_or(ContractError::NotInitialized)?;
        if caller != key_id && caller != admin {
            return Err(ContractError::Unauthorized);
        }
        if total_amount <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let creator_keys_addr = read_creator_keys(&env).ok_or(ContractError::NotInitialized)?;
        let reader = CreatorKeysReader::new(&env, &creator_keys_addr);
        let holders = reader.get_holder_registry(&key_id);

        let airdrop_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextAirdropId)
            .unwrap_or(1);

        // Snapshot holder balances at this ledger: only non-zero balances are
        // written, and the total supply they sum to is frozen in the airdrop
        // record so later balance changes cannot shift any allocation.
        let mut total_snapshot_supply: u32 = 0;
        let mut holders_snapshotted: u32 = 0;
        for wallet in holders.iter() {
            let balance = reader.get_key_balance(&key_id, &wallet);
            if balance == 0 {
                continue;
            }
            holders_snapshotted = holders_snapshotted
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
            if holders_snapshotted > MAX_AIRDROP_HOLDERS {
                return Err(ContractError::HolderLimitExceeded);
            }
            total_snapshot_supply = total_snapshot_supply
                .checked_add(balance)
                .ok_or(ContractError::Overflow)?;
            write_snapshot_balance(&env, airdrop_id, &wallet, balance);
        }
        if total_snapshot_supply == 0 {
            return Err(ContractError::NoHolders);
        }

        // Fund the airdrop: pull the reward tokens from the funder into this
        // contract so claims can be paid out from the escrowed balance.
        TokenClient::new(&env, &reward_token).transfer(
            &caller,
            &env.current_contract_address(),
            &total_amount,
        );

        let next_id = airdrop_id.checked_add(1).ok_or(ContractError::Overflow)?;
        env.storage()
            .instance()
            .set(&DataKey::NextAirdropId, &next_id);

        let airdrop = Airdrop {
            airdrop_id,
            key_id: key_id.clone(),
            funder: caller.clone(),
            reward_token: reward_token.clone(),
            total_amount,
            total_snapshot_supply,
            snapshot_ledger: env.ledger().sequence(),
            total_claimed: 0,
        };
        write_airdrop(&env, &airdrop);

        env.events().publish(
            events::airdrop_created_topics(&env, &key_id),
            events::AirdropCreatedEvent {
                airdrop_id,
                key_id,
                funder: caller,
                reward_token,
                total_amount,
                total_snapshot_supply,
                snapshot_ledger: airdrop.snapshot_ledger,
            },
        );

        Ok(airdrop_id)
    }

    /// Claims the caller's pro-rata share of an airdrop and transfers the
    /// reward tokens. Each wallet may claim an airdrop at most once.
    ///
    /// # Errors
    ///
    /// - [`ContractError::AirdropNotFound`] if the airdrop does not exist.
    /// - [`ContractError::AlreadyClaimed`] on a second claim by the same wallet.
    /// - [`ContractError::NoAllocation`] if the wallet held no keys at the
    ///   snapshot block.
    pub fn claim_airdrop(
        env: Env,
        airdrop_id: u64,
        caller: Address,
    ) -> Result<i128, ContractError> {
        caller.require_auth();

        let mut airdrop = read_airdrop(&env, airdrop_id).ok_or(ContractError::AirdropNotFound)?;
        if has_claimed(&env, airdrop_id, &caller) {
            return Err(ContractError::AlreadyClaimed);
        }

        let amount = allocation_for(&env, &airdrop, &caller)?;
        if amount <= 0 {
            return Err(ContractError::NoAllocation);
        }

        mark_claimed(&env, airdrop_id, &caller);
        airdrop.total_claimed = airdrop
            .total_claimed
            .checked_add(amount)
            .ok_or(ContractError::Overflow)?;
        write_airdrop(&env, &airdrop);

        TokenClient::new(&env, &airdrop.reward_token).transfer(
            &env.current_contract_address(),
            &caller,
            &amount,
        );

        env.events().publish(
            events::airdrop_claimed_topics(&env, &airdrop.key_id, &caller),
            events::AirdropClaimedEvent {
                airdrop_id,
                key_id: airdrop.key_id.clone(),
                wallet: caller,
                amount,
            },
        );

        Ok(amount)
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    /// Returns the claimable and claimed amounts for `wallet` on `airdrop_id`.
    ///
    /// Allocations are derived from the snapshot balances frozen at airdrop
    /// creation, so the result is independent of current key balances.
    pub fn get_airdrop_allocation(
        env: Env,
        airdrop_id: u64,
        wallet: Address,
    ) -> Result<AirdropAllocation, ContractError> {
        let airdrop = read_airdrop(&env, airdrop_id).ok_or(ContractError::AirdropNotFound)?;
        let allocation = allocation_for(&env, &airdrop, &wallet)?;
        let claimed = if has_claimed(&env, airdrop_id, &wallet) {
            allocation
        } else {
            0
        };

        Ok(AirdropAllocation {
            claimable: allocation.saturating_sub(claimed),
            claimed,
        })
    }

    /// Returns the full airdrop record, including snapshot metadata.
    pub fn get_airdrop(env: Env, airdrop_id: u64) -> Result<Airdrop, ContractError> {
        read_airdrop(&env, airdrop_id).ok_or(ContractError::AirdropNotFound)
    }
}
