//! #1098 — Key ownership handover with an acceptance window and timelock.
//!
//! Lets the wallet that currently manages a key hand that management right over
//! to a new wallet in three phases:
//!
//! 1. **Propose** — the current owner (initially the registered creator) raises
//!    a proposal with [`propose_transfer`]. The `own_prop` event is what holders
//!    observe during the notification period.
//! 2. **Accept** — the proposed new owner must accept with [`accept_transfer`]
//!    within [`ACCEPTANCE_WINDOW_SECS`] (48h). An unaccepted proposal expires
//!    at `expires_at` and can no longer be accepted.
//! 3. **Execute** — once accepted, [`execute_transfer`] finalises the handover
//!    only after [`EXECUTION_TIMELOCK_SECS`] (24h) has elapsed, so holders keep
//!    a full day of notice after the new owner has committed.
//!
//! The current owner may abandon a proposal at any point before execution with
//! [`cancel_transfer`]. At most one live proposal exists per key, so an
//! ownership record can never be overwritten by a stale proposal.
//!
//! The handover record itself is read with [`read_key_owner`] (exposed as the
//! `get_key_owner` entrypoint); it defaults to `key_id`, the registered creator
//! address, until a handover has been executed.

use crate::{
    events, extend_key_ttl_to_full_window, read_registered_creator_profile, ContractError,
};
use soroban_sdk::{contracttype, Address, Env};

/// Seconds the proposed new owner has to accept a handover before the proposal
/// expires (48 hours).
pub const ACCEPTANCE_WINDOW_SECS: u64 = 172_800;

/// Seconds that must elapse between acceptance and execution before the
/// handover may be finalised (24 hours).
pub const EXECUTION_TIMELOCK_SECS: u64 = 86_400;

/// Storage keys for the key-ownership handover record (#1098).
///
/// Kept out of [`crate::DataKey`] so the main key enum does not grow, following
/// the same convention as the other feature modules (`PriceAlertDataKey`,
/// `EmergencyPauseDataKey`, `AnnouncementKey`). All entries are persistent and
/// extended to the full TTL window on write.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum OwnershipDataKey {
    /// `key_id` -> wallet that currently manages the key. Absent means `key_id`
    /// itself, i.e. the registered creator has never handed the key over.
    Owner(Address),
    /// `proposal_id` -> the proposal record.
    Proposal(u32),
    /// `key_id` -> id of the single live proposal for that key.
    ActiveProposal(Address),
    /// Next proposal id to issue -> `u32`; ids start at `0`.
    NextProposalId,
}

/// A single key-ownership handover proposal.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct OwnershipTransferProposal {
    /// Sequential id of this proposal; also its storage key.
    pub proposal_id: u32,
    /// Key whose management rights are being handed over.
    pub key_id: Address,
    /// Owner that raised the proposal (the proposer).
    pub current_owner: Address,
    /// Wallet proposed to take over management of `key_id`.
    pub new_owner: Address,
    /// Ledger timestamp at which the proposal was raised.
    pub proposed_at: u64,
    /// `proposed_at + ACCEPTANCE_WINDOW_SECS`. Acceptance is rejected from this
    /// timestamp onwards.
    pub expires_at: u64,
    /// Ledger timestamp of acceptance; `None` until the new owner accepts.
    pub accepted_at: Option<u64>,
    /// `accepted_at + EXECUTION_TIMELOCK_SECS`; `None` until acceptance.
    pub executable_at: Option<u64>,
}

impl OwnershipTransferProposal {
    /// Whether the proposal can still be accepted at `now`: it has never been
    /// accepted and the 48h acceptance window is still open.
    pub fn is_acceptable_at(&self, now: u64) -> bool {
        self.accepted_at.is_none() && now < self.expires_at
    }

    /// Whether the proposal can still be cancelled at `now` — true until it is
    /// executed, including after acceptance.
    pub fn is_live_at(&self, now: u64) -> bool {
        self.accepted_at.is_some() || now < self.expires_at
    }
}

/// Read-only: the wallet that currently manages `key_id`.
///
/// Returns `key_id` itself — the registered creator address — when no handover
/// has been executed yet, so pre-existing callers that treat the creator as the
/// owner keep seeing the same value.
pub fn read_key_owner(env: &Env, key_id: &Address) -> Address {
    env.storage()
        .persistent()
        .get(&OwnershipDataKey::Owner(key_id.clone()))
        .unwrap_or_else(|| key_id.clone())
}

/// Reads a proposal by id, or [`ContractError::TransferProposalNotFound`].
fn read_proposal(env: &Env, proposal_id: u32) -> Result<OwnershipTransferProposal, ContractError> {
    env.storage()
        .persistent()
        .get(&OwnershipDataKey::Proposal(proposal_id))
        .ok_or(ContractError::TransferProposalNotFound)
}

/// Persists a proposal and marks it as the key's single live proposal.
fn store_proposal(env: &Env, proposal: &OwnershipTransferProposal) {
    let proposal_key = OwnershipDataKey::Proposal(proposal.proposal_id);
    let active_key = OwnershipDataKey::ActiveProposal(proposal.key_id.clone());
    let storage = env.storage().persistent();
    storage.set(&proposal_key, proposal);
    storage.set(&active_key, &proposal.proposal_id);
    extend_key_ttl_to_full_window(env, &proposal_key);
    extend_key_ttl_to_full_window(env, &active_key);
}

/// Removes a proposal and clears the key's live-proposal index.
fn clear_proposal(env: &Env, proposal: &OwnershipTransferProposal) {
    let storage = env.storage().persistent();
    storage.remove(&OwnershipDataKey::Proposal(proposal.proposal_id));
    storage.remove(&OwnershipDataKey::ActiveProposal(proposal.key_id.clone()));
}

/// Drops the key's live-proposal index when it points at nothing.
fn clear_dangling_active_index(env: &Env, key_id: &Address) {
    env.storage()
        .persistent()
        .remove(&OwnershipDataKey::ActiveProposal(key_id.clone()));
}

/// Raises an ownership-handover proposal for `key_id`.
///
/// # Access
///
/// The wallet that currently manages the key authorises the call
/// (`owner.require_auth()`), which for a key that has never been handed over is
/// the registered creator. The key must be registered.
///
/// # Validation
///
/// - `key_id` must be a registered key, otherwise [`ContractError::NotRegistered`].
/// - `new_owner` must differ from the current owner, otherwise
///   [`ContractError::SelfTransfer`].
/// - The key must have no live proposal, otherwise
///   [`ContractError::TransferAlreadyPending`]. A proposal whose 48h acceptance
///   window has passed and that was never accepted is *not* live: it is dropped
///   silently and replaced by the new one.
///
/// Returns the new `proposal_id`. The handover only takes effect once the new
/// owner accepts and [`execute_transfer`] runs.
pub fn propose_transfer(
    env: &Env,
    key_id: &Address,
    new_owner: &Address,
) -> Result<u32, ContractError> {
    read_registered_creator_profile(env, key_id)?;
    let owner = read_key_owner(env, key_id);
    owner.require_auth();

    if *new_owner == owner {
        return Err(ContractError::SelfTransfer);
    }

    let now = env.ledger().timestamp();

    // At most one live proposal per key: drop an expired, never-accepted one,
    // otherwise reject so two racing proposals can never both execute.
    let active_id: Option<u32> = env
        .storage()
        .persistent()
        .get(&OwnershipDataKey::ActiveProposal(key_id.clone()));
    if let Some(active_id) = active_id {
        let existing: Option<OwnershipTransferProposal> = env
            .storage()
            .persistent()
            .get(&OwnershipDataKey::Proposal(active_id));
        match existing {
            Some(active) if active.is_live_at(now) => {
                return Err(ContractError::TransferAlreadyPending);
            }
            Some(active) => clear_proposal(env, &active),
            None => clear_dangling_active_index(env, key_id),
        }
    }

    let proposal_id: u32 = env
        .storage()
        .persistent()
        .get(&OwnershipDataKey::NextProposalId)
        .unwrap_or(0);
    let following_id = proposal_id.checked_add(1).ok_or(ContractError::Overflow)?;
    env.storage()
        .persistent()
        .set(&OwnershipDataKey::NextProposalId, &following_id);
    extend_key_ttl_to_full_window(env, &OwnershipDataKey::NextProposalId);

    let expires_at = now
        .checked_add(ACCEPTANCE_WINDOW_SECS)
        .ok_or(ContractError::Overflow)?;

    let proposal = OwnershipTransferProposal {
        proposal_id,
        key_id: key_id.clone(),
        current_owner: owner,
        new_owner: new_owner.clone(),
        proposed_at: now,
        expires_at,
        accepted_at: None,
        executable_at: None,
    };
    store_proposal(env, &proposal);

    env.events().publish(
        events::transfer_proposed_topics(&proposal.key_id, &proposal.new_owner),
        events::TransferProposedEvent {
            proposal_id: proposal.proposal_id,
            key_id: proposal.key_id.clone(),
            current_owner: proposal.current_owner.clone(),
            new_owner: proposal.new_owner.clone(),
            proposed_at: proposal.proposed_at,
            expires_at: proposal.expires_at,
            ledger: env.ledger().sequence(),
        },
    );
    Ok(proposal_id)
}

/// Accepts a pending ownership-handover proposal as the proposed new owner.
///
/// # Access
///
/// Only the wallet named in the proposal authorises the call
/// (`new_owner.require_auth()`).
///
/// # Validation
///
/// - The proposal must exist, otherwise [`ContractError::TransferProposalNotFound`].
/// - It must not be accepted already, otherwise
///   [`ContractError::TransferAlreadyAccepted`].
/// - It must be accepted strictly before `expires_at` (48h after the proposal),
///   otherwise [`ContractError::TransferWindowExpired`]. An expired proposal
///   stays in storage until the current owner cancels it or raises a new one
///   for the same key, but it can never be accepted or executed.
///
/// On success, records the acceptance and returns the timestamp at which
/// [`execute_transfer`] becomes callable (24h later).
pub fn accept_transfer(env: &Env, proposal_id: u32) -> Result<u64, ContractError> {
    let mut proposal = read_proposal(env, proposal_id)?;
    proposal.new_owner.require_auth();

    let now = env.ledger().timestamp();
    if !proposal.is_acceptable_at(now) {
        if proposal.accepted_at.is_some() {
            return Err(ContractError::TransferAlreadyAccepted);
        }
        return Err(ContractError::TransferWindowExpired);
    }

    let executable_at = now
        .checked_add(EXECUTION_TIMELOCK_SECS)
        .ok_or(ContractError::Overflow)?;

    proposal.accepted_at = Some(now);
    proposal.executable_at = Some(executable_at);
    store_proposal(env, &proposal);

    env.events().publish(
        events::transfer_accepted_topics(&proposal.key_id, &proposal.new_owner),
        events::TransferAcceptedEvent {
            proposal_id: proposal.proposal_id,
            key_id: proposal.key_id.clone(),
            current_owner: proposal.current_owner.clone(),
            new_owner: proposal.new_owner.clone(),
            accepted_at: now,
            executable_at,
            ledger: env.ledger().sequence(),
        },
    );
    Ok(executable_at)
}

/// Finalises an accepted ownership handover once its timelock has elapsed.
///
/// Permissionless: the proposal already carries the current owner's proposal and
/// the new owner's acceptance, so anyone may push it through once the 24h
/// timelock has expired — including the new owner, the current owner, or a
/// holder keeping the record honest.
///
/// # Validation
///
/// - The proposal must exist, otherwise [`ContractError::TransferProposalNotFound`].
/// - It must have been accepted, otherwise [`ContractError::TransferNotAccepted`]
///   (this is also what an unaccepted, expired proposal reports).
/// - `now >= executable_at`, otherwise [`ContractError::TimelockNotElapsed`].
/// - The owner recorded in the proposal must still own the key, otherwise
///   [`ContractError::Unauthorized`].
///
/// On success the key's owner becomes the proposal's `new_owner` and the
/// proposal is deleted.
pub fn execute_transfer(env: &Env, proposal_id: u32) -> Result<(), ContractError> {
    let proposal = read_proposal(env, proposal_id)?;
    let executable_at = proposal
        .executable_at
        .ok_or(ContractError::TransferNotAccepted)?;

    let now = env.ledger().timestamp();
    if now < executable_at {
        return Err(ContractError::TimelockNotElapsed);
    }

    if read_key_owner(env, &proposal.key_id) != proposal.current_owner {
        return Err(ContractError::Unauthorized);
    }

    let owner_key = OwnershipDataKey::Owner(proposal.key_id.clone());
    env.storage()
        .persistent()
        .set(&owner_key, &proposal.new_owner);
    extend_key_ttl_to_full_window(env, &owner_key);
    clear_proposal(env, &proposal);

    env.events().publish(
        events::transfer_executed_topics(&proposal.key_id, &proposal.new_owner),
        events::TransferExecutedEvent {
            proposal_id: proposal.proposal_id,
            key_id: proposal.key_id.clone(),
            previous_owner: proposal.current_owner.clone(),
            new_owner: proposal.new_owner.clone(),
            executed_at: now,
            ledger: env.ledger().sequence(),
        },
    );
    Ok(())
}

/// Abandons a pending ownership-handover proposal before it executes.
///
/// # Access
///
/// The wallet that currently manages the key authorises the call
/// (`current_owner.require_auth()`), which for a proposal raised by the
/// registered creator is that creator.
///
/// # Validation
///
/// - The proposal must exist, otherwise [`ContractError::TransferProposalNotFound`].
/// - The current owner must be the owner that raised it, otherwise
///   [`ContractError::Unauthorized`].
///
/// Cancellation is allowed at any point before execution, including after the
/// new owner has accepted — the accepting wallet cannot force the handover.
pub fn cancel_transfer(env: &Env, proposal_id: u32) -> Result<(), ContractError> {
    let proposal = read_proposal(env, proposal_id)?;
    let owner = read_key_owner(env, &proposal.key_id);
    owner.require_auth();

    if owner != proposal.current_owner {
        return Err(ContractError::Unauthorized);
    }

    clear_proposal(env, &proposal);
    Ok(())
}

/// Read-only view: the proposal recorded under `proposal_id`.
///
/// Returns [`ContractError::TransferProposalNotFound`] once the proposal has
/// been executed or cancelled, or if it never existed.
pub fn get_transfer_proposal(
    env: &Env,
    proposal_id: u32,
) -> Result<OwnershipTransferProposal, ContractError> {
    read_proposal(env, proposal_id)
}

/// Read-only view: the wallet that currently manages `key_id`, defaulting to
/// `key_id` itself when no handover has been executed.
pub fn get_key_owner(env: &Env, key_id: &Address) -> Address {
    read_key_owner(env, key_id)
}
