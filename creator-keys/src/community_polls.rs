//! Snapshot-weighted community polls for a creator's key holders.
//!
//! This is distinct from the live-weight, quorum-escalating governance polls in
//! [`crate::events`] (`create_poll` / `cast_vote` / `close_poll`): here, every
//! holder's vote weight is fixed from their balance at poll-creation time, a
//! wallet may vote exactly once, and the poll simply expires after
//! `duration_seconds` rather than requiring quorum. Entry points and types are
//! named distinctly (`*_snapshot_poll`, [`SnapshotPoll`]) to avoid colliding
//! with that existing feature.
use crate::{
    extend_key_ttl_to_full_window, read_registered_creator_profile, ContractError,
    CreatorKeysContract, CreatorKeysContractArgs, CreatorKeysContractClient,
};
use soroban_sdk::{contractimpl, contracttype, symbol_short, Address, Env, String, Vec};

/// Minimum number of options a poll must offer.
pub const MIN_POLL_OPTIONS: u32 = 2;
/// Maximum number of options a poll may offer.
pub const MAX_POLL_OPTIONS: u32 = 4;
/// Maximum number of holders whose balances can be snapshotted for a single poll.
pub const MAX_POLL_HOLDERS: u32 = 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SnapshotPoll {
    pub id: u32,
    pub creator: Address,
    pub key_id: Address,
    pub question: String,
    pub options: Vec<String>,
    /// Per-option cumulative vote weight, indexed the same as `options`.
    pub option_weights: Vec<u64>,
    /// Per-option number of distinct voters, indexed the same as `options`.
    pub option_counts: Vec<u32>,
    pub snapshot_supply: u32,
    pub created_at: u64,
    pub expires_at: u64,
    pub closed: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum SnapshotPollKey {
    NextId,
    Poll(u32),
    /// (poll_id, holder) -> snapshot balance taken at `create_snapshot_poll` time.
    Weight(u32, Address),
    /// (poll_id, voter) -> true once the voter has cast a vote.
    Voted(u32, Address),
}

fn read_poll(env: &Env, poll_id: u32) -> Result<SnapshotPoll, ContractError> {
    let key = SnapshotPollKey::Poll(poll_id);
    let poll = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContractError::PollNotFound)?;
    extend_key_ttl_to_full_window(env, &key);
    Ok(poll)
}

fn write_poll(env: &Env, poll_id: u32, poll: &SnapshotPoll) {
    let key = SnapshotPollKey::Poll(poll_id);
    env.storage().persistent().set(&key, poll);
    extend_key_ttl_to_full_window(env, &key);
}

#[contractimpl]
impl CreatorKeysContract {
    /// Creates a snapshot-weighted poll for `key_id`'s holders.
    ///
    /// `creator`-only; snapshots every current holder's balance at creation time so
    /// later trades cannot change already-cast or future vote weight for this poll.
    pub fn create_snapshot_poll(
        env: Env,
        creator: Address,
        key_id: Address,
        question: String,
        options: Vec<String>,
        duration_seconds: u64,
    ) -> Result<u32, ContractError> {
        creator.require_auth();
        let profile = read_registered_creator_profile(&env, &key_id)?;
        if profile.creator != creator {
            return Err(ContractError::Unauthorized);
        }
        if question.is_empty() || duration_seconds == 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        if options.len() < MIN_POLL_OPTIONS || options.len() > MAX_POLL_OPTIONS {
            return Err(ContractError::InvalidPollOption);
        }
        for option in options.iter() {
            if option.is_empty() {
                return Err(ContractError::InvalidPollOption);
            }
        }
        if profile.supply == 0 {
            return Err(ContractError::InsufficientSupply);
        }
        let holders = Self::get_holder_registry(env.clone(), key_id.clone());
        if holders.len() > MAX_POLL_HOLDERS {
            return Err(ContractError::SnapshotHolderLimitExceeded);
        }

        let id: u32 = env
            .storage()
            .persistent()
            .get(&SnapshotPollKey::NextId)
            .unwrap_or(0);
        let next_id = id.checked_add(1).ok_or(ContractError::Overflow)?;

        let created_at = env.ledger().timestamp();
        let expires_at = created_at
            .checked_add(duration_seconds)
            .ok_or(ContractError::Overflow)?;

        for holder in holders.iter() {
            let balance = Self::get_key_balance(env.clone(), key_id.clone(), holder.clone());
            if balance > 0 {
                let key = SnapshotPollKey::Weight(id, holder);
                env.storage().persistent().set(&key, &balance);
                extend_key_ttl_to_full_window(&env, &key);
            }
        }

        let mut option_weights: Vec<u64> = Vec::new(&env);
        let mut option_counts: Vec<u32> = Vec::new(&env);
        for _ in 0..options.len() {
            option_weights.push_back(0);
            option_counts.push_back(0);
        }

        let poll = SnapshotPoll {
            id,
            creator: creator.clone(),
            key_id: key_id.clone(),
            question,
            options,
            option_weights,
            option_counts,
            snapshot_supply: profile.supply,
            created_at,
            expires_at,
            closed: false,
        };
        write_poll(&env, id, &poll);
        env.storage()
            .persistent()
            .set(&SnapshotPollKey::NextId, &next_id);
        extend_key_ttl_to_full_window(&env, &SnapshotPollKey::NextId);

        env.events()
            .publish((symbol_short!("poll_new"), key_id, creator), poll);
        Ok(id)
    }

    /// Casts a vote on `poll_id` for `option_index`, weighted by the voter's
    /// snapshot balance taken when the poll was created.
    pub fn cast_poll_vote(
        env: Env,
        voter: Address,
        poll_id: u32,
        option_index: u32,
    ) -> Result<(), ContractError> {
        voter.require_auth();
        let mut poll = read_poll(&env, poll_id)?;
        if poll.closed || env.ledger().timestamp() >= poll.expires_at {
            return Err(ContractError::PollClosed);
        }
        if option_index >= poll.options.len() {
            return Err(ContractError::InvalidPollOption);
        }

        let voted_key = SnapshotPollKey::Voted(poll_id, voter.clone());
        if env.storage().persistent().has(&voted_key) {
            return Err(ContractError::AlreadyVoted);
        }

        let weight_key = SnapshotPollKey::Weight(poll_id, voter.clone());
        let weight: u32 = env.storage().persistent().get(&weight_key).unwrap_or(0);
        if weight == 0 {
            return Err(ContractError::NotAHolder);
        }
        extend_key_ttl_to_full_window(&env, &weight_key);

        let current_weight = poll.option_weights.get(option_index).unwrap();
        let updated_weight = current_weight
            .checked_add(u64::from(weight))
            .ok_or(ContractError::Overflow)?;
        poll.option_weights.set(option_index, updated_weight);

        let current_count = poll.option_counts.get(option_index).unwrap();
        let updated_count = current_count
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;
        poll.option_counts.set(option_index, updated_count);

        write_poll(&env, poll_id, &poll);
        env.storage().persistent().set(&voted_key, &true);
        extend_key_ttl_to_full_window(&env, &voted_key);

        env.events().publish(
            (symbol_short!("poll_vote"), poll_id, voter),
            (option_index, weight),
        );
        Ok(())
    }

    /// Permissionlessly closes `poll_id` once its voting window has elapsed,
    /// locking in the final tally and emitting a `poll_end` event with the
    /// final poll data.
    pub fn close_snapshot_poll(env: Env, poll_id: u32) -> Result<SnapshotPoll, ContractError> {
        let mut poll = read_poll(&env, poll_id)?;
        if poll.closed {
            return Err(ContractError::ActionNotPending);
        }
        if env.ledger().timestamp() < poll.expires_at {
            return Err(ContractError::DeadlinePassed);
        }
        poll.closed = true;
        write_poll(&env, poll_id, &poll);
        env.events()
            .publish((symbol_short!("poll_end"), poll_id), poll.clone());
        Ok(poll)
    }

    /// Returns the current per-option vote counts and weights for `poll_id`.
    ///
    /// Available both before and after expiry; once `close_snapshot_poll` has
    /// been called the returned tally is final and will not change.
    pub fn get_poll_results(env: Env, poll_id: u32) -> Result<SnapshotPoll, ContractError> {
        read_poll(&env, poll_id)
    }
}
