#![no_std]

use common::guard::ReentrancyGuard;
use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, token::TokenClient,
    Address, BytesN, Env,
};

pub mod events;

#[cfg(test)]
mod test;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
/// Contract error variants. Append only — discriminants are ABI.
pub enum ContractError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidReward = 4,
    InvalidMaxWinners = 5,
    InvalidDeadline = 6,
    BountyNotFound = 7,
    DeadlinePassed = 8,
    DeadlineNotReached = 9,
    NotAHolder = 10,
    AlreadyActiveSubmission = 11,
    SubmissionNotFound = 12,
    NoActiveSubmission = 13,
    MaxWinnersReached = 14,
    AlreadyReclaimed = 15,
    Overflow = 16,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Config,
    NextBountyId,
    Bounty(u64),
    Submission(u64, Address),
}

/// Contract-wide configuration. A zero `min_holder_balance` disables holder
/// gating; a positive value requires every worker to hold at least that many
/// `key_id` keys when submitting.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Config {
    pub reward_token: Address,
    pub creator_keys: Address,
    pub min_holder_balance: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Bounty {
    pub bounty_id: u64,
    pub creator: Address,
    pub key_id: Address,
    pub reward_token: Address,
    pub reward: i128,
    pub deadline_ledger: u32,
    pub max_winners: u32,
    pub winners: u32,
    pub paid: i128,
    pub reclaimed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Submission {
    pub worker: Address,
    pub proof_hash: BytesN<32>,
    pub active: bool,
    pub awarded: bool,
}

/// Read-only subset of the creator-keys contract used for holder gating.
#[contractclient(name = "CreatorKeysReader")]
pub trait CreatorKeysInterface {
    fn get_balance(env: Env, creator: Address, wallet: Address) -> u32;
}

fn read_config(env: &Env) -> Result<Config, ContractError> {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .ok_or(ContractError::NotInitialized)
}

fn read_bounty(env: &Env, bounty_id: u64) -> Result<Bounty, ContractError> {
    env.storage()
        .persistent()
        .get(&DataKey::Bounty(bounty_id))
        .ok_or(ContractError::BountyNotFound)
}

fn write_bounty(env: &Env, bounty: &Bounty) {
    env.storage()
        .persistent()
        .set(&DataKey::Bounty(bounty.bounty_id), bounty);
}

fn read_submission(
    env: &Env,
    bounty_id: u64,
    worker: &Address,
) -> Result<Submission, ContractError> {
    env.storage()
        .persistent()
        .get(&DataKey::Submission(bounty_id, worker.clone()))
        .ok_or(ContractError::SubmissionNotFound)
}

fn write_submission(env: &Env, bounty_id: u64, submission: &Submission) {
    env.storage().persistent().set(
        &DataKey::Submission(bounty_id, submission.worker.clone()),
        submission,
    );
}

fn remaining_reward(bounty: &Bounty) -> Result<i128, ContractError> {
    bounty
        .reward
        .checked_sub(bounty.paid)
        .ok_or(ContractError::Overflow)
}

#[contract]
pub struct BountyBoard;

#[contractimpl]
impl BountyBoard {
    /// Sets the reward token, creator-keys address and holder threshold.
    /// Requires a positive `max_winners` and a future `deadline_ledger` at
    /// posting time. A zero holder threshold disables gating.
    pub fn init(
        env: Env,
        reward_token: Address,
        creator_keys: Address,
        min_holder_balance: u32,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(ContractError::AlreadyInitialized);
        }
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                reward_token,
                creator_keys,
                min_holder_balance,
            },
        );
        env.storage().instance().set(&DataKey::NextBountyId, &1u64);
        Ok(())
    }

    /// Creates a bounty and escrows the entire `reward` from `creator`.
    pub fn post_bounty(
        env: Env,
        creator: Address,
        key_id: Address,
        reward: i128,
        deadline_ledger: u32,
        max_winners: u32,
    ) -> Result<u64, ContractError> {
        creator.require_auth();
        let _guard = ReentrancyGuard::enter(&env);
        let config = read_config(&env)?;

        if reward <= 0 {
            return Err(ContractError::InvalidReward);
        }
        if max_winners == 0 {
            return Err(ContractError::InvalidMaxWinners);
        }
        if deadline_ledger <= env.ledger().sequence() {
            return Err(ContractError::InvalidDeadline);
        }

        let bounty_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextBountyId)
            .unwrap_or(1);
        let next_id = bounty_id.checked_add(1).ok_or(ContractError::Overflow)?;
        env.storage()
            .instance()
            .set(&DataKey::NextBountyId, &next_id);

        TokenClient::new(&env, &config.reward_token).transfer(
            &creator,
            &env.current_contract_address(),
            &reward,
        );

        let bounty = Bounty {
            bounty_id,
            creator: creator.clone(),
            key_id: key_id.clone(),
            reward_token: config.reward_token,
            reward,
            deadline_ledger,
            max_winners,
            winners: 0,
            paid: 0,
            reclaimed: false,
        };
        write_bounty(&env, &bounty);

        env.events().publish(
            events::bounty_posted_topics(&env, bounty_id),
            events::BountyPostedEvent {
                bounty_id,
                creator,
                key_id,
                reward,
                deadline_ledger,
                max_winners,
            },
        );
        Ok(bounty_id)
    }

    /// Registers an active submission for `worker`. Each worker may hold only
    /// one active submission per bounty, and re-submission after rejection is
    /// allowed. Holder-gated bounties require the configured key balance.
    pub fn submit(
        env: Env,
        worker: Address,
        bounty_id: u64,
        proof_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        worker.require_auth();
        let bounty = read_bounty(&env, bounty_id)?;
        if env.ledger().sequence() >= bounty.deadline_ledger {
            return Err(ContractError::DeadlinePassed);
        }

        if let Ok(existing) = read_submission(&env, bounty_id, &worker) {
            if existing.active {
                return Err(ContractError::AlreadyActiveSubmission);
            }
        }

        let config = read_config(&env)?;
        if config.min_holder_balance > 0
            && CreatorKeysReader::new(&env, &config.creator_keys)
                .get_balance(&bounty.key_id, &worker)
                < config.min_holder_balance
        {
            return Err(ContractError::NotAHolder);
        }

        write_submission(
            &env,
            bounty_id,
            &Submission {
                worker: worker.clone(),
                proof_hash: proof_hash.clone(),
                active: true,
                awarded: false,
            },
        );
        env.events().publish(
            events::submission_made_topics(&env, bounty_id, &worker),
            events::SubmissionMadeEvent {
                bounty_id,
                worker,
                proof_hash,
            },
        );
        Ok(())
    }

    /// Awards `reward / max_winners` to an active submission. Only the bounty
    /// creator may award, and no more than `max_winners` workers can be paid.
    pub fn award(
        env: Env,
        creator: Address,
        bounty_id: u64,
        worker: Address,
    ) -> Result<i128, ContractError> {
        creator.require_auth();
        let _guard = ReentrancyGuard::enter(&env);
        let mut bounty = read_bounty(&env, bounty_id)?;
        if bounty.creator != creator {
            return Err(ContractError::Unauthorized);
        }
        if bounty.reclaimed {
            return Err(ContractError::AlreadyReclaimed);
        }
        if env.ledger().sequence() >= bounty.deadline_ledger {
            return Err(ContractError::DeadlinePassed);
        }
        if bounty.winners >= bounty.max_winners {
            return Err(ContractError::MaxWinnersReached);
        }

        let mut submission = read_submission(&env, bounty_id, &worker)?;
        if !submission.active {
            return Err(ContractError::NoActiveSubmission);
        }

        let amount = bounty.reward / bounty.max_winners as i128;
        let paid = bounty
            .paid
            .checked_add(amount)
            .ok_or(ContractError::Overflow)?;
        bounty.paid = paid;
        bounty.winners = bounty
            .winners
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;
        write_bounty(&env, &bounty);

        submission.active = false;
        submission.awarded = true;
        write_submission(&env, bounty_id, &submission);

        if amount > 0 {
            TokenClient::new(&env, &bounty.reward_token).transfer(
                &env.current_contract_address(),
                &worker,
                &amount,
            );
        }
        env.events().publish(
            events::bounty_awarded_topics(&env, bounty_id, &worker),
            events::BountyAwardedEvent {
                bounty_id,
                worker: worker.clone(),
                amount,
                winners: bounty.winners,
            },
        );
        Ok(amount)
    }

    /// Deactivates a submission without paying it, freeing the worker to
    /// submit again. Only the bounty creator may reject.
    pub fn reject_submission(
        env: Env,
        creator: Address,
        bounty_id: u64,
        worker: Address,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        let bounty = read_bounty(&env, bounty_id)?;
        if bounty.creator != creator {
            return Err(ContractError::Unauthorized);
        }
        let mut submission = read_submission(&env, bounty_id, &worker)?;
        if !submission.active {
            return Err(ContractError::NoActiveSubmission);
        }
        submission.active = false;
        write_submission(&env, bounty_id, &submission);
        Ok(())
    }

    /// Returns the still-unpaid portion of the escrow to the creator once the
    /// deadline has been reached. Already-awarded rewards are never reclaimed.
    pub fn reclaim_unawarded(
        env: Env,
        creator: Address,
        bounty_id: u64,
    ) -> Result<i128, ContractError> {
        creator.require_auth();
        let _guard = ReentrancyGuard::enter(&env);
        let mut bounty = read_bounty(&env, bounty_id)?;
        if bounty.creator != creator {
            return Err(ContractError::Unauthorized);
        }
        if env.ledger().sequence() < bounty.deadline_ledger {
            return Err(ContractError::DeadlineNotReached);
        }
        if bounty.reclaimed {
            return Err(ContractError::AlreadyReclaimed);
        }

        let amount = remaining_reward(&bounty)?;
        bounty.reclaimed = true;
        write_bounty(&env, &bounty);

        if amount > 0 {
            TokenClient::new(&env, &bounty.reward_token).transfer(
                &env.current_contract_address(),
                &creator,
                &amount,
            );
        }
        env.events().publish(
            events::bounty_reclaimed_topics(&env, bounty_id),
            events::BountyReclaimedEvent {
                bounty_id,
                creator: creator.clone(),
                amount,
            },
        );
        Ok(amount)
    }

    pub fn get_bounty(env: Env, bounty_id: u64) -> Result<Bounty, ContractError> {
        read_bounty(&env, bounty_id)
    }

    pub fn get_submission(
        env: Env,
        bounty_id: u64,
        worker: Address,
    ) -> Result<Submission, ContractError> {
        read_submission(&env, bounty_id, &worker)
    }
}
