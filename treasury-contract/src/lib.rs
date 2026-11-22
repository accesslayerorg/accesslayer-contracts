#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, BytesN, Env,
    String, Symbol, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TreasuryError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidMilestoneAmount = 4,
    MilestoneAmountMismatch = 5,
    GrantNotFound = 6,
    MilestoneNotFound = 7,
    MilestoneNotSubmitted = 8,
    MilestoneAlreadyApproved = 9,
    MilestoneAlreadyRejected = 10,
    InvalidMilestoneIndex = 11,
    OutOfOrderSubmission = 12,
    InsufficientBalance = 13,
    NothingToCancel = 14,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Balance,
    Epoch,
    EpochHistory(u32),
    GrantCounter,
    Grant(u32),
    MilestoneSubmitted(u32, u32),
    MilestoneApproved(u32, u32),
    ReviewerRole,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochDistribution {
    pub epoch: u32,
    pub recipients: Vec<Address>,
    pub amounts: Vec<i128>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    pub amount: i128,
    pub description_hash: BytesN<32>,
    pub submitted: bool,
    pub approved: bool,
    pub rejected: bool,
    pub evidence_hash: Option<BytesN<32>>,
    pub reject_reason_hash: Option<BytesN<32>>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grant {
    pub id: u32,
    pub admin: Address,
    pub recipient: Address,
    pub total_amount: i128,
    pub released_amount: i128,
    pub milestones: Vec<Milestone>,
    pub cancelled: bool,
    pub require_sequential: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MilestoneState {
    Pending,
    Submitted,
    Approved,
    Rejected,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantStatus {
    pub id: u32,
    pub admin: Address,
    pub recipient: Address,
    pub total_amount: i128,
    pub released_amount: i128,
    pub milestones: Vec<MilestoneStatus>,
    pub cancelled: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneStatus {
    pub index: u32,
    pub amount: i128,
    pub description_hash: BytesN<32>,
    pub state: MilestoneState,
    pub evidence_hash: Option<BytesN<32>>,
    pub reject_reason_hash: Option<BytesN<32>>,
}

#[contract]
pub struct TreasuryContract;

#[contractimpl]
impl TreasuryContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, TreasuryError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Balance, &0i128);
        env.storage().instance().set(&DataKey::Epoch, &0u32);
        env.storage().instance().set(&DataKey::GrantCounter, &0u32);
    }

    pub fn set_reviewer_role(env: Env, reviewer: Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::ReviewerRole, &reviewer);
    }

    pub fn get_reviewer_role(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::ReviewerRole)
    }

    pub fn collect_fee(env: Env, key_id: String, amount: i128) {
        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let mut balance: i128 = env.storage().instance().get(&DataKey::Balance).unwrap_or(0);
        balance += amount;
        env.storage().instance().set(&DataKey::Balance, &balance);

        env.events()
            .publish((Symbol::new(&env, "FeeCollected"), key_id), amount);
    }

    pub fn get_treasury_balance(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::Balance).unwrap_or(0)
    }

    pub fn distribute(env: Env, recipients: Vec<Address>, amounts: Vec<i128>) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        admin.require_auth();

        if recipients.len() != amounts.len() {
            panic!("Recipients and amounts length mismatch");
        }

        let mut total_amount: i128 = 0;
        for amount in amounts.iter() {
            if amount <= 0 {
                panic!("Amount must be positive");
            }
            total_amount += amount;
        }

        let mut balance: i128 = env.storage().instance().get(&DataKey::Balance).unwrap_or(0);
        if total_amount > balance {
            panic!("Insufficient balance");
        }

        balance -= total_amount;
        env.storage().instance().set(&DataKey::Balance, &balance);

        let epoch: u32 = env.storage().instance().get(&DataKey::Epoch).unwrap_or(0) + 1;
        let distribution = EpochDistribution {
            epoch,
            recipients: recipients.clone(),
            amounts: amounts.clone(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::EpochHistory(epoch), &distribution);
        env.storage().instance().set(&DataKey::Epoch, &(epoch + 1));

        env.events()
            .publish((Symbol::new(&env, "FeeDistributed"), epoch), distribution);
    }

    pub fn get_distribution_history(env: Env, epoch: u32) -> Option<EpochDistribution> {
        env.storage()
            .persistent()
            .get(&DataKey::EpochHistory(epoch))
    }

    pub fn create_grant(
        env: Env,
        admin: Address,
        recipient: Address,
        total: i128,
        milestones: Vec<MilestoneInput>,
        require_sequential: bool,
    ) -> u32 {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        if admin != stored_admin {
            panic_with_error!(&env, TreasuryError::Unauthorized);
        }

        if total <= 0 {
            panic_with_error!(&env, TreasuryError::InvalidMilestoneAmount);
        }

        if milestones.is_empty() {
            panic_with_error!(&env, TreasuryError::InvalidMilestoneAmount);
        }

        let mut milestone_sum: i128 = 0;
        let mut grant_milestones: Vec<Milestone> = Vec::new(&env);

        for input in milestones.iter() {
            if input.amount <= 0 {
                panic_with_error!(&env, TreasuryError::InvalidMilestoneAmount);
            }
            milestone_sum += input.amount;

            grant_milestones.push_back(Milestone {
                amount: input.amount,
                description_hash: input.description_hash,
                submitted: false,
                approved: false,
                rejected: false,
                evidence_hash: None,
                reject_reason_hash: None,
            });
        }

        if milestone_sum != total {
            panic_with_error!(&env, TreasuryError::MilestoneAmountMismatch);
        }

        let mut balance: i128 = env.storage().instance().get(&DataKey::Balance).unwrap_or(0);
        if total > balance {
            panic_with_error!(&env, TreasuryError::InsufficientBalance);
        }

        balance -= total;
        env.storage().instance().set(&DataKey::Balance, &balance);

        let grant_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::GrantCounter)
            .unwrap_or(0);
        let next_id = grant_id + 1;
        env.storage()
            .instance()
            .set(&DataKey::GrantCounter, &next_id);

        let grant = Grant {
            id: grant_id,
            admin: admin.clone(),
            recipient: recipient.clone(),
            total_amount: total,
            released_amount: 0,
            milestones: grant_milestones,
            cancelled: false,
            require_sequential,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Grant(grant_id), &grant);

        env.events().publish(
            (Symbol::new(&env, "grant_created"), grant_id),
            (admin, recipient, total, require_sequential),
        );

        grant_id
    }

    pub fn submit_milestone(
        env: Env,
        recipient: Address,
        grant_id: u32,
        index: u32,
        evidence_hash: BytesN<32>,
    ) {
        recipient.require_auth();

        let mut grant: Grant = env
            .storage()
            .persistent()
            .get(&DataKey::Grant(grant_id))
            .ok_or(TreasuryError::GrantNotFound)
            .unwrap();

        if grant.cancelled {
            panic!("Grant is cancelled");
        }

        if grant.recipient != recipient {
            panic_with_error!(&env, TreasuryError::Unauthorized);
        }

        if index >= grant.milestones.len() {
            panic_with_error!(&env, TreasuryError::InvalidMilestoneIndex);
        }

        let mut milestone = grant.milestones.get(index).unwrap();

        if milestone.submitted {
            panic!("Milestone already submitted");
        }

        if milestone.approved || milestone.rejected {
            panic!("Milestone already finalized");
        }

        if grant.require_sequential && index > 0 {
            let prev_milestone = grant.milestones.get(index - 1).unwrap();
            if !prev_milestone.approved {
                panic_with_error!(&env, TreasuryError::OutOfOrderSubmission);
            }
        }

        milestone.submitted = true;
        milestone.evidence_hash = Some(evidence_hash.clone());

        grant.milestones.set(index, milestone);
        env.storage()
            .persistent()
            .set(&DataKey::Grant(grant_id), &grant);

        env.storage()
            .persistent()
            .set(&DataKey::MilestoneSubmitted(grant_id, index), &true);

        env.events().publish(
            (Symbol::new(&env, "milestone_submitted"), grant_id),
            (recipient, index, evidence_hash),
        );
    }

    pub fn approve_milestone(env: Env, reviewer: Address, grant_id: u32, index: u32) {
        let stored_reviewer: Option<Address> = env.storage().instance().get(&DataKey::ReviewerRole);
        if stored_reviewer.is_none() || stored_reviewer.unwrap() != reviewer {
            panic_with_error!(&env, TreasuryError::Unauthorized);
        }
        reviewer.require_auth();

        let mut grant: Grant = env
            .storage()
            .persistent()
            .get(&DataKey::Grant(grant_id))
            .ok_or(TreasuryError::GrantNotFound)
            .unwrap();

        if grant.cancelled {
            panic!("Grant is cancelled");
        }

        if index >= grant.milestones.len() {
            panic_with_error!(&env, TreasuryError::InvalidMilestoneIndex);
        }

        let mut milestone = grant.milestones.get(index).unwrap();

        if !milestone.submitted {
            panic_with_error!(&env, TreasuryError::MilestoneNotSubmitted);
        }

        if milestone.approved {
            panic_with_error!(&env, TreasuryError::MilestoneAlreadyApproved);
        }

        if milestone.rejected {
            panic_with_error!(&env, TreasuryError::MilestoneAlreadyRejected);
        }

        milestone.approved = true;
        grant.released_amount += milestone.amount;

        let amount = milestone.amount;
        grant.milestones.set(index, milestone);
        env.storage()
            .persistent()
            .set(&DataKey::Grant(grant_id), &grant);

        env.storage()
            .persistent()
            .set(&DataKey::MilestoneApproved(grant_id, index), &true);

        env.events().publish(
            (Symbol::new(&env, "milestone_approved"), grant_id),
            (reviewer, index, amount),
        );
    }

    pub fn reject_milestone(
        env: Env,
        reviewer: Address,
        grant_id: u32,
        index: u32,
        reason_hash: BytesN<32>,
    ) {
        let stored_reviewer: Option<Address> = env.storage().instance().get(&DataKey::ReviewerRole);
        if stored_reviewer.is_none() || stored_reviewer.unwrap() != reviewer {
            panic_with_error!(&env, TreasuryError::Unauthorized);
        }
        reviewer.require_auth();

        let mut grant: Grant = env
            .storage()
            .persistent()
            .get(&DataKey::Grant(grant_id))
            .ok_or(TreasuryError::GrantNotFound)
            .unwrap();

        if grant.cancelled {
            panic!("Grant is cancelled");
        }

        if index >= grant.milestones.len() {
            panic_with_error!(&env, TreasuryError::InvalidMilestoneIndex);
        }

        let mut milestone = grant.milestones.get(index).unwrap();

        if !milestone.submitted {
            panic_with_error!(&env, TreasuryError::MilestoneNotSubmitted);
        }

        if milestone.approved {
            panic_with_error!(&env, TreasuryError::MilestoneAlreadyApproved);
        }

        if milestone.rejected {
            panic_with_error!(&env, TreasuryError::MilestoneAlreadyRejected);
        }

        milestone.rejected = true;
        milestone.reject_reason_hash = Some(reason_hash.clone());

        grant.milestones.set(index, milestone);
        env.storage()
            .persistent()
            .set(&DataKey::Grant(grant_id), &grant);

        env.events().publish(
            (Symbol::new(&env, "milestone_rejected"), grant_id),
            (reviewer, index, reason_hash),
        );
    }

    pub fn cancel_grant(env: Env, admin: Address, grant_id: u32) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        if admin != stored_admin {
            panic_with_error!(&env, TreasuryError::Unauthorized);
        }

        let mut grant: Grant = env
            .storage()
            .persistent()
            .get(&DataKey::Grant(grant_id))
            .ok_or(TreasuryError::GrantNotFound)
            .unwrap();

        if grant.cancelled {
            panic!("Grant already cancelled");
        }

        let unreleased = grant.total_amount - grant.released_amount;
        if unreleased <= 0 {
            panic_with_error!(&env, TreasuryError::NothingToCancel);
        }

        let mut balance: i128 = env.storage().instance().get(&DataKey::Balance).unwrap_or(0);
        balance += unreleased;
        env.storage().instance().set(&DataKey::Balance, &balance);

        grant.cancelled = true;
        env.storage()
            .persistent()
            .set(&DataKey::Grant(grant_id), &grant);

        env.events().publish(
            (Symbol::new(&env, "grant_cancelled"), grant_id),
            (admin, unreleased),
        );
    }

    pub fn grant_status(env: Env, grant_id: u32) -> GrantStatus {
        let grant: Grant = env
            .storage()
            .persistent()
            .get(&DataKey::Grant(grant_id))
            .ok_or(TreasuryError::GrantNotFound)
            .unwrap();

        let mut milestone_statuses: Vec<MilestoneStatus> = Vec::new(&env);

        for (i, milestone) in grant.milestones.iter().enumerate() {
            let state = if milestone.approved {
                MilestoneState::Approved
            } else if milestone.rejected {
                MilestoneState::Rejected
            } else if milestone.submitted {
                MilestoneState::Submitted
            } else {
                MilestoneState::Pending
            };

            milestone_statuses.push_back(MilestoneStatus {
                index: i as u32,
                amount: milestone.amount,
                description_hash: milestone.description_hash,
                state,
                evidence_hash: milestone.evidence_hash,
                reject_reason_hash: milestone.reject_reason_hash,
            });
        }

        GrantStatus {
            id: grant.id,
            admin: grant.admin,
            recipient: grant.recipient,
            total_amount: grant.total_amount,
            released_amount: grant.released_amount,
            milestones: milestone_statuses,
            cancelled: grant.cancelled,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneInput {
    pub amount: i128,
    pub description_hash: BytesN<32>,
}

#[cfg(test)]
mod test;
