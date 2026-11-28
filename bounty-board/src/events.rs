//! Event definitions for the bounty board contract.
//!
//! Event field order is stable: append only, never reorder or remove fields.

use soroban_sdk::{contracttype, Address, BytesN, Env, Symbol};

pub const BOUNTY_POSTED_EVENT_NAME: &str = "bounty_posted";
pub const SUBMISSION_MADE_EVENT_NAME: &str = "submission_made";
pub const BOUNTY_AWARDED_EVENT_NAME: &str = "bounty_awarded";
pub const BOUNTY_RECLAIMED_EVENT_NAME: &str = "bounty_reclaimed";

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BountyPostedEvent {
    pub bounty_id: u64,
    pub creator: Address,
    pub key_id: Address,
    pub reward: i128,
    pub deadline_ledger: u32,
    pub max_winners: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SubmissionMadeEvent {
    pub bounty_id: u64,
    pub worker: Address,
    pub proof_hash: BytesN<32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BountyAwardedEvent {
    pub bounty_id: u64,
    pub worker: Address,
    pub amount: i128,
    pub winners: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BountyReclaimedEvent {
    pub bounty_id: u64,
    pub creator: Address,
    pub amount: i128,
}

pub fn bounty_posted_topics(env: &Env, bounty_id: u64) -> (Symbol, u64) {
    (Symbol::new(env, BOUNTY_POSTED_EVENT_NAME), bounty_id)
}

pub fn submission_made_topics(
    env: &Env,
    bounty_id: u64,
    worker: &Address,
) -> (Symbol, u64, Address) {
    (
        Symbol::new(env, SUBMISSION_MADE_EVENT_NAME),
        bounty_id,
        worker.clone(),
    )
}

pub fn bounty_awarded_topics(
    env: &Env,
    bounty_id: u64,
    worker: &Address,
) -> (Symbol, u64, Address) {
    (
        Symbol::new(env, BOUNTY_AWARDED_EVENT_NAME),
        bounty_id,
        worker.clone(),
    )
}

pub fn bounty_reclaimed_topics(env: &Env, bounty_id: u64) -> (Symbol, u64) {
    (Symbol::new(env, BOUNTY_RECLAIMED_EVENT_NAME), bounty_id)
}
