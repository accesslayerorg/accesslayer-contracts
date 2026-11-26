use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::ContractError;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct JointKey {
    pub key_id: Address,
    pub creators: Vec<Address>,
    pub weights_bps: Vec<u32>,
    pub threshold_bps: u32,
    pub notice_ledgers: u32,
}

#[derive(Clone)]
#[contracttype]
enum JointKeyDataKey {
    Joint(Address),
    Proposal(u32),
    NextProposal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ChangeProposal {
    pub key_id: Address,
    pub owner: Address,
    pub change: u32,
    pub proposed_at: u32,
    pub execution_not_before: u32,
    pub approvals_bps: u32,
}

pub fn create_joint_key(
    env: &Env,
    key_id: Address,
    creators: Vec<Address>,
    weights_bps: Vec<u32>,
    threshold_bps: u32,
) -> Result<(), ContractError> {
    if creators.is_empty()
        || creators.len() != weights_bps.len()
        || threshold_bps == 0
        || threshold_bps > 10_000
    {
        return Err(ContractError::InvalidWeights);
    }
    let mut sum = 0u32;
    for w in weights_bps.iter() {
        sum = sum.saturating_add(w);
    }
    if sum != 10_000 {
        return Err(ContractError::InvalidWeights);
    }
    let joint = JointKey {
        key_id: key_id.clone(),
        creators,
        weights_bps,
        threshold_bps,
        notice_ledgers: 17_280,
    };
    env.storage()
        .instance()
        .set(&JointKeyDataKey::Joint(key_id.clone()), &joint);
    env.events().publish(
        (soroban_sdk::Symbol::new(env, "joint_key_created"), key_id),
        (),
    );
    Ok(())
}

pub fn propose_change(
    env: &Env,
    owner: Address,
    key_id: Address,
    change: u32,
) -> Result<u32, ContractError> {
    owner.require_auth();
    let joint: JointKey = env
        .storage()
        .instance()
        .get(&JointKeyDataKey::Joint(key_id.clone()))
        .ok_or(ContractError::JointKeyNotFound)?;
    if !joint.creators.iter().any(|c| c == owner) {
        return Err(ContractError::NotJointOwner);
    }
    let id: u32 = env
        .storage()
        .instance()
        .get(&JointKeyDataKey::NextProposal)
        .unwrap_or(1);
    let proposal = ChangeProposal {
        key_id,
        owner,
        change,
        proposed_at: env.ledger().sequence(),
        execution_not_before: env.ledger().sequence().saturating_add(17_280),
        approvals_bps: 0,
    };
    env.storage()
        .instance()
        .set(&JointKeyDataKey::Proposal(id), &proposal);
    env.storage()
        .instance()
        .set(&JointKeyDataKey::NextProposal, &(id + 1));
    env.events()
        .publish((soroban_sdk::Symbol::new(env, "change_proposed"), id), ());
    Ok(id)
}

pub fn approve_change(env: &Env, owner: Address, proposal_id: u32) -> Result<(), ContractError> {
    owner.require_auth();
    let mut proposal: ChangeProposal = env
        .storage()
        .instance()
        .get(&JointKeyDataKey::Proposal(proposal_id))
        .ok_or(ContractError::JointProposalNotFound)?;
    let joint: JointKey = env
        .storage()
        .instance()
        .get(&JointKeyDataKey::Joint(proposal.key_id.clone()))
        .ok_or(ContractError::JointKeyNotFound)?;
    let mut weight = 0u32;
    for i in 0..joint.creators.len() {
        if joint.creators.get(i).unwrap() == owner {
            weight = joint.weights_bps.get(i).unwrap();
            break;
        }
    }
    if weight == 0 {
        return Err(ContractError::NotJointOwner);
    }
    proposal.approvals_bps = proposal.approvals_bps.saturating_add(weight);
    env.storage()
        .instance()
        .set(&JointKeyDataKey::Proposal(proposal_id), &proposal);
    env.events().publish(
        (
            soroban_sdk::Symbol::new(env, "change_approved"),
            proposal_id,
        ),
        (),
    );
    Ok(())
}

pub fn execute_change(env: &Env, proposal_id: u32) -> Result<(), ContractError> {
    let proposal: ChangeProposal = env
        .storage()
        .instance()
        .get(&JointKeyDataKey::Proposal(proposal_id))
        .ok_or(ContractError::JointProposalNotFound)?;
    let joint: JointKey = env
        .storage()
        .instance()
        .get(&JointKeyDataKey::Joint(proposal.key_id.clone()))
        .ok_or(ContractError::JointKeyNotFound)?;
    if proposal.approvals_bps < joint.threshold_bps {
        return Err(ContractError::ThresholdNotMet);
    }
    if env.ledger().sequence() < proposal.execution_not_before {
        return Err(ContractError::TimelockNotElapsed);
    }
    env.events().publish(
        (
            soroban_sdk::Symbol::new(env, "change_executed"),
            proposal_id,
        ),
        (),
    );
    Ok(())
}
