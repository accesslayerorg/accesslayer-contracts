//! Centralized event names and helpers for consistent event emission.
//!
//! This module provides a single source of truth for event names used throughout
//! the contract, reducing string duplication and ensuring consistency across
//! event emission paths.

use crate::{constants, read_registered_creator_profile, CreatorKeysContract};
use soroban_sdk::{
    contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

/// Event name for creator registration.
pub const REGISTER_EVENT_NAME: Symbol = symbol_short!("register");

/// Event name for key purchase.
pub const BUY_EVENT_NAME: Symbol = symbol_short!("buy");

/// Event name for governance poll creation.
pub const POLL_CREATED_EVENT_NAME: Symbol = symbol_short!("poll_new");

/// Event name for governance poll votes.
pub const POLL_VOTE_EVENT_NAME: Symbol = symbol_short!("poll_vote");

const MIN_POLL_OPTIONS: u32 = 2;
const MAX_POLL_OPTIONS: u32 = 4;
const MAX_QUESTION_CHARS: u32 = 280;
const MAX_OPTION_CHARS: u32 = 100;

/// Stable registration event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(REGISTER_EVENT_NAME, creator)`
/// - data: `CreatorRegisteredEvent`
///
/// This keeps the creator address indexed in event topics while preserving
/// a predictable payload for off-chain consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CreatorRegisteredEvent {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
    pub holder_count: u32,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PollError {
    NotRegistered = 20,
    Overflow = 21,
    InvalidOptionCount = 22,
    QuestionTooLong = 23,
    OptionTooLong = 24,
    PollNotFound = 25,
    PollExpired = 26,
    NotAHolder = 27,
    InvalidOption = 28,
}

#[derive(Clone)]
#[contracttype]
enum PollDataKey {
    NextPollId(Address),
    Poll(Address, u32),
    Vote(Address, u32, Address),
}

#[derive(Clone)]
#[contracttype]
pub struct Poll {
    pub question: String,
    pub options: Vec<String>,
    pub vote_counts: Vec<u32>,
    pub total_weight: u32,
    pub expires_at: u32,
}

#[derive(Clone)]
#[contracttype]
pub struct PollVote {
    pub option_index: u32,
    pub weight: u32,
}

#[derive(Clone)]
#[contracttype]
pub struct PollResult {
    pub question: String,
    pub options: Vec<String>,
    pub vote_counts: Vec<u32>,
    pub total_weight: u32,
    pub expired: bool,
}

fn poll_storage_key(creator_id: &Address, poll_id: u32) -> PollDataKey {
    PollDataKey::Poll(creator_id.clone(), poll_id)
}

fn vote_storage_key(creator_id: &Address, poll_id: u32, voter: &Address) -> PollDataKey {
    PollDataKey::Vote(creator_id.clone(), poll_id, voter.clone())
}

fn read_poll(env: &Env, creator_id: &Address, poll_id: u32) -> Result<Poll, PollError> {
    env.storage()
        .persistent()
        .get(&poll_storage_key(creator_id, poll_id))
        .ok_or(PollError::PollNotFound)
}

fn is_poll_expired(env: &Env, poll: &Poll) -> bool {
    env.ledger().sequence() >= poll.expires_at
}

fn validate_poll_options(options: &Vec<String>) -> Result<(), PollError> {
    let option_count = options.len();
    if !(MIN_POLL_OPTIONS..=MAX_POLL_OPTIONS).contains(&option_count) {
        return Err(PollError::InvalidOptionCount);
    }

    let mut index = 0;
    while index < option_count {
        let option = options.get(index).ok_or(PollError::InvalidOption)?;
        if option.len() > MAX_OPTION_CHARS {
            return Err(PollError::OptionTooLong);
        }
        index += 1;
    }

    Ok(())
}

#[contractimpl]
impl CreatorKeysContract {
    /// Creates a creator-owned governance poll with two to four options.
    ///
    /// The creator address must authorize the call. Polls expire at the current ledger
    /// sequence plus `duration_ledgers`, and the returned `poll_id` is scoped to the creator.
    pub fn create_poll(
        env: Env,
        creator_id: Address,
        question: String,
        options: Vec<String>,
        duration_ledgers: u32,
    ) -> Result<u32, PollError> {
        creator_id.require_auth();
        read_registered_creator_profile(&env, &creator_id).map_err(|_| PollError::NotRegistered)?;

        if question.len() > MAX_QUESTION_CHARS {
            return Err(PollError::QuestionTooLong);
        }
        validate_poll_options(&options)?;

        let mut vote_counts = Vec::new(&env);
        let mut index = 0;
        while index < options.len() {
            vote_counts.push_back(0);
            index += 1;
        }

        let next_key = PollDataKey::NextPollId(creator_id.clone());
        let poll_id: u32 = env.storage().persistent().get(&next_key).unwrap_or(1);
        let next_poll_id = poll_id.checked_add(1).ok_or(PollError::Overflow)?;
        let expires_at = env
            .ledger()
            .sequence()
            .checked_add(duration_ledgers)
            .ok_or(PollError::Overflow)?;

        let poll = Poll {
            question,
            options,
            vote_counts,
            total_weight: 0,
            expires_at,
        };

        env.storage().persistent().set(&poll_storage_key(&creator_id, poll_id), &poll);
        env.storage().persistent().set(&next_key, &next_poll_id);
        env.events().publish(
            (POLL_CREATED_EVENT_NAME, creator_id.clone(), poll_id),
            poll.expires_at,
        );

        Ok(poll_id)
    }

    /// Casts or updates a weighted vote for a creator poll.
    ///
    /// The voter must authorize the call and must currently hold at least one liquid key for
    /// the creator. Re-voting before expiry removes the previous weight and adds the voter's
    /// current liquid key balance to the selected option.
    pub fn cast_vote(
        env: Env,
        creator_id: Address,
        voter: Address,
        poll_id: u32,
        option_index: u32,
    ) -> Result<(), PollError> {
        voter.require_auth();
        let mut poll = read_poll(&env, &creator_id, poll_id)?;

        if is_poll_expired(&env, &poll) {
            return Err(PollError::PollExpired);
        }
        if option_index >= poll.options.len() {
            return Err(PollError::InvalidOption);
        }

        let balance_key = constants::storage::key_balance(&creator_id, &voter);
        let weight: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if weight == 0 {
            return Err(PollError::NotAHolder);
        }

        let vote_key = vote_storage_key(&creator_id, poll_id, &voter);
        if let Some(previous_vote) = env
            .storage()
            .persistent()
            .get::<PollDataKey, PollVote>(&vote_key)
        {
            let previous_count = poll
                .vote_counts
                .get(previous_vote.option_index)
                .ok_or(PollError::InvalidOption)?;
            let updated_previous_count = previous_count
                .checked_sub(previous_vote.weight)
                .ok_or(PollError::Overflow)?;
            poll.vote_counts
                .set(previous_vote.option_index, updated_previous_count);
            poll.total_weight = poll
                .total_weight
                .checked_sub(previous_vote.weight)
                .ok_or(PollError::Overflow)?;
        }

        let selected_count = poll
            .vote_counts
            .get(option_index)
            .ok_or(PollError::InvalidOption)?;
        let updated_selected_count = selected_count.checked_add(weight).ok_or(PollError::Overflow)?;
        poll.vote_counts.set(option_index, updated_selected_count);
        poll.total_weight = poll.total_weight.checked_add(weight).ok_or(PollError::Overflow)?;

        env.storage().persistent().set(&poll_storage_key(&creator_id, poll_id), &poll);
        env.storage().persistent().set(
            &vote_key,
            &PollVote {
                option_index,
                weight,
            },
        );
        env.events()
            .publish((POLL_VOTE_EVENT_NAME, creator_id, poll_id, voter), (option_index, weight));

        Ok(())
    }

    /// Returns the current weighted result for a creator poll.
    pub fn get_poll_result(
        env: Env,
        creator_id: Address,
        poll_id: u32,
    ) -> Result<PollResult, PollError> {
        let poll = read_poll(&env, &creator_id, poll_id)?;
        let expired = is_poll_expired(&env, &poll);
        Ok(PollResult {
            question: poll.question,
            options: poll.options,
            vote_counts: poll.vote_counts,
            total_weight: poll.total_weight,
            expired,
        })
    }
}
