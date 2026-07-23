//! Centralized event names and helpers for consistent event emission.
//!
//! This module provides a single source of truth for event names used throughout
//! the contract, reducing string duplication and ensuring consistency across
//! event emission paths.
//!
//! ### Event Schema Stability
//!
//! Downstream indexers rely on the stable ordering of fields in event payloads.
//! When modifying event structures:
//! - **Do not reorder** existing fields.
//! - **Add new fields** only at the end of the structure to maintain compatibility.
//! - **Avoid removing fields**; if a field is deprecated, keep it with a default value.
//!
//! This approach ensures that indexers can reliably parse event data across
//! different contract versions.
//!
//! ### Quote-Related Event Field Semantics
//!
//! - `supply`: Number of keys in circulation after the trade (for buy/sell events)
//! - `payment`: Total amount paid by the buyer (for buy events, ≥ key price)

use crate::{
    constants, read_registered_creator_profile, CreatorKeysContract, CreatorKeysContractArgs,
    CreatorKeysContractClient,
};
use soroban_sdk::{
    contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

/// Event name for protocol pause.
pub const PAUSE_EVENT_NAME: Symbol = symbol_short!("pause");

/// Event name for protocol unpause.
pub const UNPAUSE_EVENT_NAME: Symbol = symbol_short!("unpause");

/// Event name for creator registration.
pub const REGISTER_EVENT_NAME: Symbol = symbol_short!("register");

/// Event name for key purchase.
pub const BUY_EVENT_NAME: Symbol = symbol_short!("buy");

/// Event name for key sale.
pub const SELL_EVENT_NAME: Symbol = symbol_short!("sell");

/// Event name for peer-to-peer key transfer.
pub const TRANSFER_EVENT_NAME: Symbol = symbol_short!("transfer");

/// Event name for creator key buyback.
pub const BUYBACK_EVENT_NAME: Symbol = symbol_short!("buyback");

/// Event name for governance poll creation.
pub const POLL_CREATED_EVENT_NAME: Symbol = symbol_short!("poll_new");

/// Event name for governance poll votes.
pub const POLL_VOTE_EVENT_NAME: Symbol = symbol_short!("poll_vote");

/// Topic index for the event name in common event topic tuples.
pub const TOPIC_EVENT_NAME_INDEX: u32 = 0;

/// Topic index for the creator address in common event topic tuples.
pub const TOPIC_CREATOR_INDEX: u32 = 1;

/// Topic index for the buyer/seller/actor address in common event topic tuples.
pub const TOPIC_BUYER_INDEX: u32 = 2;

/// Stable field order for registration event payloads.
pub const REGISTER_EVENT_DATA_FIELDS: [&str; 6] = [
    "creator",
    "handle",
    "supply",
    "holder_count",
    "creator_bps",
    "protocol_bps",
];

/// Stable field order for buy event payloads.
pub const BUY_EVENT_DATA_FIELDS: [&str; 2] = ["supply", "payment"];

/// Stable field order for sell event payloads.
pub const SELL_EVENT_DATA_FIELDS: [&str; 1] = ["supply"];

/// Stable field order for buyback event payloads.
pub const BUYBACK_EVENT_DATA_FIELDS: [&str; 5] =
    ["creator", "amount", "price_paid", "new_supply", "ledger"];

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
    pub creator_bps: u32,
    pub protocol_bps: u32,
}

/// Shared registration event topics tuple.
pub fn register_event_topics(creator: &Address) -> (Symbol, Address) {
    (REGISTER_EVENT_NAME, creator.clone())
}

/// Stable buyback event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(BUYBACK_EVENT_NAME, creator)`
/// - data: `KeysBoughtBackEvent`
///
/// # Creator Fee Waiver
/// On buybacks, the creator fee is explicitly waived because the creator cannot pay
/// themselves a fee. The protocol fee still applies.
///
/// # Indexer Note
/// This event represents a creator burning keys from their own held balance,
/// which is distinct from a regular buy event. Indexers should process this
/// event separately from `BUY_EVENT_NAME` events to correctly track supply
/// changes and fee accounting.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysBoughtBackEvent {
    /// Address of the creator performing the buyback.
    pub creator: Address,
    /// Number of keys being bought back and burned.
    pub amount: u32,
    /// Total amount paid by the creator, including protocol fee (but not creator fee).
    pub price_paid: i128,
    /// New total supply of keys for the creator after the buyback.
    pub new_supply: u32,
    /// Ledger sequence number at the time of the buyback.
    pub ledger: u32,
}

/// Shared buy event topics tuple.
pub fn buy_event_topics(creator: &Address, buyer: &Address) -> (Symbol, Address, Address) {
    (BUY_EVENT_NAME, creator.clone(), buyer.clone())
}

/// Shared peer-to-peer transfer event topics tuple.
pub fn transfer_event_topics(creator: &Address, from: &Address) -> (Symbol, Address, Address) {
    (TRANSFER_EVENT_NAME, creator.clone(), from.clone())
}

/// Shared buyback event topics tuple.
pub fn buyback_event_topics(creator: &Address) -> (Symbol, Address) {
    (BUYBACK_EVENT_NAME, creator.clone())
}

/// Event name for dividend distribution.
pub const DIVIDEND_DISTRIBUTED_EVENT_NAME: Symbol = symbol_short!("div_dist");

/// Event name for dividend claim.
pub const DIVIDEND_CLAIMED_EVENT_NAME: Symbol = symbol_short!("div_claim");

/// Event name for allocation locked.
pub const ALLOCATION_LOCKED_EVENT_NAME: Symbol = symbol_short!("alloc_lck");

/// Event name for allocation claimed.
pub const ALLOCATION_CLAIMED_EVENT_NAME: Symbol = symbol_short!("alloc_clm");

/// Event name for protocol fee recipient updated.
pub const PROTOCOL_FEE_RECIPIENT_UPDATED_EVENT_NAME: Symbol = symbol_short!("p_fee_upd");

/// Event name for creator fee recipient updated.
pub const CREATOR_FEE_RECIPIENT_UPDATED_EVENT_NAME: Symbol = symbol_short!("c_fee_upd");

/// Event name for co-creator fee accrual.
pub const CO_CREATOR_FEE_EARNED_EVENT_NAME: Symbol = symbol_short!("co_fee");

/// Event name for creator key TTL extension.
pub const TTL_EXTENDED_EVENT_NAME: Symbol = symbol_short!("ttl_ext");

/// Stable field order for dividend distributed event payloads.
pub const DIVIDEND_DISTRIBUTED_DATA_FIELDS: [&str; 4] =
    ["creator", "total_amount", "snapshot_supply", "ledger"];

/// Stable field order for dividend claimed event payloads.
pub const DIVIDEND_CLAIMED_DATA_FIELDS: [&str; 3] = ["creator", "claimant", "amount"];

/// Stable field order for co-creator fee earned event payloads.
pub const CO_CREATOR_FEE_EARNED_DATA_FIELDS: [&str; 4] =
    ["creator_id", "co_creator", "amount", "ledger"];

/// Stable field order for TTL extended event payloads.
pub const TTL_EXTENDED_DATA_FIELDS: [&str; 3] = ["creator", "extend_to", "ledger"];

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendDistributedEvent {
    pub creator: Address,
    pub total_amount: i128,
    pub snapshot_supply: u32,
    pub ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendClaimedEvent {
    pub creator: Address,
    pub claimant: Address,
    pub amount: i128,
}

pub fn dividend_distributed_topics(creator: &Address) -> (Symbol, Address) {
    (DIVIDEND_DISTRIBUTED_EVENT_NAME, creator.clone())
}

pub fn dividend_claimed_topics(
    creator: &Address,
    claimant: &Address,
) -> (Symbol, Address, Address) {
    (
        DIVIDEND_CLAIMED_EVENT_NAME,
        creator.clone(),
        claimant.clone(),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AllocationLockedEvent {
    pub creator_id: Address,
    pub amount: u32,
    pub unlock_ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AllocationClaimedEvent {
    pub creator_id: Address,
    pub amount: u32,
    pub ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ProtocolFeeRecipientUpdatedEvent {
    pub old_recipient: Address,
    pub new_recipient: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CreatorFeeRecipientUpdatedEvent {
    pub creator_id: Address,
    pub old_recipient: Address,
    pub new_recipient: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CoCreatorFeeEarned {
    pub creator_id: Address,
    pub co_creator: Address,
    pub amount: i128,
    pub ledger: u32,
}

/// Stable TTL extended event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(TTL_EXTENDED_EVENT_NAME, creator)`
/// - data: `TtlExtendedEvent`
///
/// This event is emitted when a creator's storage entries have their TTL extended
/// after a successful buy, sell, or other operation that keeps the creator active.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TtlExtendedEvent {
    pub creator: Address,
    pub extend_to: u32,
    pub ledger: u32,
}

pub fn co_creator_fee_earned_topics(
    creator_id: &Address,
    co_creator: &Address,
) -> (Symbol, Address, Address) {
    (
        CO_CREATOR_FEE_EARNED_EVENT_NAME,
        creator_id.clone(),
        co_creator.clone(),
    )
}

/// Shared TTL extended event topics tuple.
pub fn ttl_extended_topics(creator: &Address) -> (Symbol, Address) {
    (TTL_EXTENDED_EVENT_NAME, creator.clone())
}

/// Event name for key transfer.
pub const KEYS_TRANSFERRED_EVENT_NAME: Symbol = symbol_short!("xfer");

/// Stable field order for key transfer event payloads.
pub const KEYS_TRANSFERRED_DATA_FIELDS: [&str; 5] =
    ["creator_id", "from", "to", "amount", "ledger"];

/// Stable key transfer event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(KEYS_TRANSFERRED_EVENT_NAME, creator_id, from)`
/// - data: `KeysTransferredEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysTransferredEvent {
    pub creator_id: Address,
    pub from: Address,
    pub to: Address,
    pub amount: u32,
    pub ledger: u32,
}

/// Event name for creator key airdrops.
pub const KEYS_AIRDROPPED_EVENT_NAME: Symbol = symbol_short!("airdrop");

/// Stable field order for airdrop event payloads.
pub const KEYS_AIRDROPPED_DATA_FIELDS: [&str; 6] = [
    "creator_id",
    "total_keys",
    "total_cost",
    "recipient_count",
    "skipped_count",
    "ledger",
];

/// Stable airdrop event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(KEYS_AIRDROPPED_EVENT_NAME, creator_id)`
/// - data: `KeysAirdroppedEvent`
///
/// `total_cost` is the full amount charged to the creator (curve cost plus
/// protocol fee), `skipped_count` is the number of recipients skipped due to
/// per-wallet cap, and `ledger` is the Soroban ledger sequence number at airdrop
/// time so off-chain indexers can reconstruct the timeline.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysAirdroppedEvent {
    pub creator_id: Address,
    pub total_keys: u32,
    pub total_cost: i128,
    pub recipient_count: u32,
    pub skipped_count: u32,
    pub ledger: u32,
}

/// Shared airdrop event topics tuple.
pub fn keys_airdropped_topics(creator: &Address) -> (Symbol, Address) {
    (KEYS_AIRDROPPED_EVENT_NAME, creator.clone())
}

/// Event name for treasury withdrawal by the protocol admin.
pub const TREASURY_WITHDRAWAL_EVENT_NAME: Symbol = symbol_short!("treas_out");

/// Stable field order for treasury withdrawal event payloads.
pub const TREASURY_WITHDRAWAL_DATA_FIELDS: [&str; 4] =
    ["amount", "recipient", "remaining_balance", "ledger"];

/// Stable treasury withdrawal event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(TREASURY_WITHDRAWAL_EVENT_NAME, recipient)`
/// - data: `TreasuryWithdrawalEvent`
///
/// `ledger` is the Soroban ledger sequence number at the time of withdrawal so
/// off-chain indexers can reconstruct the timeline without replaying all events.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TreasuryWithdrawalEvent {
    pub amount: i128,
    pub recipient: Address,
    pub remaining_balance: i128,
    pub ledger: u32,
}

/// Shared treasury withdrawal event topics tuple.
pub fn treasury_withdrawal_event_topics(recipient: &Address) -> (Symbol, Address) {
    (TREASURY_WITHDRAWAL_EVENT_NAME, recipient.clone())
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

        env.storage()
            .persistent()
            .set(&poll_storage_key(&creator_id, poll_id), &poll);
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
        let updated_selected_count = selected_count
            .checked_add(weight)
            .ok_or(PollError::Overflow)?;
        poll.vote_counts.set(option_index, updated_selected_count);
        poll.total_weight = poll
            .total_weight
            .checked_add(weight)
            .ok_or(PollError::Overflow)?;

        env.storage()
            .persistent()
            .set(&poll_storage_key(&creator_id, poll_id), &poll);
        env.storage().persistent().set(
            &vote_key,
            &PollVote {
                option_index,
                weight,
            },
        );
        env.events().publish(
            (POLL_VOTE_EVENT_NAME, creator_id, poll_id, voter),
            (option_index, weight),
        );

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
