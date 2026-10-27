//! Holder governance with a fixed electorate, snapshot weights and delayed execution.
use crate::{
    assert_is_admin, constants, extend_key_ttl_to_full_window, read_registered_creator_profile,
    ContractError, CreatorKeysContract, CreatorKeysContractArgs, CreatorKeysContractClient,
};
use soroban_sdk::{contractimpl, contracttype, symbol_short, Address, Env, String};

pub const VOTING_SECONDS: u64 = 72 * 60 * 60;
pub const EXECUTION_DELAY_SECONDS: u64 = 24 * 60 * 60;
pub const MAX_GOVERNANCE_HOLDERS: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum ProtocolParameter {
    KeyPrice,
    CurveSlope,
    BondSlash,
}

#[derive(Clone)]
#[contracttype]
pub struct GovernanceConfig {
    pub key_id: Address,
    pub minimum_balance: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ProtocolProposal {
    pub id: u32,
    pub proposer: Address,
    pub key_id: Address,
    pub parameter: ProtocolParameter,
    pub new_value: i128,
    pub description: String,
    pub snapshot_supply: u32,
    pub voting_ends_at: u64,
    pub executable_at: u64,
    pub votes_for: u64,
    pub votes_against: u64,
    pub executed: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum GovernanceKey {
    ProtocolGovConfig,
    ProtocolGovNextId,
    ProtocolGovProposal(u32),
    ProtocolGovWeight(u32, Address),
    ProtocolGovVoted(u32, Address),
}

fn read_proposal(env: &Env, id: u32) -> Result<ProtocolProposal, ContractError> {
    let key = GovernanceKey::ProtocolGovProposal(id);
    let proposal = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContractError::ProposalNotFound)?;
    extend_key_ttl_to_full_window(env, &key);
    Ok(proposal)
}

#[contractimpl]
impl CreatorKeysContract {
    /// Admin selects the protocol's electorate once. Proposers cannot choose
    /// their own quorum, eligibility, voting window or executable parameter set.
    pub fn configure_protocol_governance(
        env: Env,
        admin: Address,
        key_id: Address,
        minimum_balance: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        read_registered_creator_profile(&env, &key_id)?;
        if minimum_balance == 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        if env
            .storage()
            .persistent()
            .has(&GovernanceKey::ProtocolGovConfig)
        {
            return Err(ContractError::AlreadyInitialized);
        }
        env.storage().persistent().set(
            &GovernanceKey::ProtocolGovConfig,
            &GovernanceConfig {
                key_id,
                minimum_balance,
            },
        );
        extend_key_ttl_to_full_window(&env, &GovernanceKey::ProtocolGovConfig);
        Ok(())
    }

    pub fn create_proposal(
        env: Env,
        proposer: Address,
        parameter: ProtocolParameter,
        new_value: i128,
        description: String,
    ) -> Result<u32, ContractError> {
        proposer.require_auth();
        let config: GovernanceConfig = env
            .storage()
            .persistent()
            .get(&GovernanceKey::ProtocolGovConfig)
            .ok_or(ContractError::Unauthorized)?;
        extend_key_ttl_to_full_window(&env, &GovernanceKey::ProtocolGovConfig);
        if Self::get_key_balance(env.clone(), config.key_id.clone(), proposer.clone())
            < config.minimum_balance
        {
            return Err(ContractError::InsufficientBalance);
        }
        if new_value < 0
            || (parameter == ProtocolParameter::KeyPrice && new_value == 0)
            || (parameter == ProtocolParameter::BondSlash && (new_value == 0 || new_value > 10_000))
            || description.is_empty()
            || description.len() > 1_024
        {
            return Err(ContractError::NotPositiveAmount);
        }
        let supply = read_registered_creator_profile(&env, &config.key_id)?.supply;
        if supply == 0 {
            return Err(ContractError::InsufficientSupply);
        }
        if parameter == ProtocolParameter::BondSlash
            && crate::performance_bonds::get_status(&env, &config.key_id)
                .map(|status| status.releasable <= 0)
                .unwrap_or(true)
        {
            return Err(ContractError::BondExecutionFailed);
        }
        let holders = Self::get_holder_registry(env.clone(), config.key_id.clone());
        if holders.len() > MAX_GOVERNANCE_HOLDERS {
            return Err(ContractError::SnapshotHolderLimitExceeded);
        }
        let id: u32 = env
            .storage()
            .persistent()
            .get(&GovernanceKey::ProtocolGovNextId)
            .unwrap_or(0);
        let next_id = id.checked_add(1).ok_or(ContractError::Overflow)?;
        let voting_ends_at = env
            .ledger()
            .timestamp()
            .checked_add(VOTING_SECONDS)
            .ok_or(ContractError::Overflow)?;
        let executable_at = voting_ends_at
            .checked_add(EXECUTION_DELAY_SECONDS)
            .ok_or(ContractError::Overflow)?;
        for holder in holders.iter() {
            let balance = Self::get_key_balance(env.clone(), config.key_id.clone(), holder.clone());
            let key = GovernanceKey::ProtocolGovWeight(id, holder);
            env.storage().persistent().set(&key, &balance);
            extend_key_ttl_to_full_window(&env, &key);
        }
        let proposal = ProtocolProposal {
            id,
            proposer: proposer.clone(),
            key_id: config.key_id,
            parameter,
            new_value,
            description,
            snapshot_supply: supply,
            voting_ends_at,
            executable_at,
            votes_for: 0,
            votes_against: 0,
            executed: false,
        };
        let key = GovernanceKey::ProtocolGovProposal(id);
        env.storage().persistent().set(&key, &proposal);
        extend_key_ttl_to_full_window(&env, &key);
        env.storage()
            .persistent()
            .set(&GovernanceKey::ProtocolGovNextId, &next_id);
        extend_key_ttl_to_full_window(&env, &GovernanceKey::ProtocolGovNextId);
        env.events()
            .publish((symbol_short!("gov_new"), proposer), proposal);
        Ok(id)
    }

    pub fn cast_proposal_vote(
        env: Env,
        voter: Address,
        proposal_id: u32,
        support: bool,
    ) -> Result<(), ContractError> {
        voter.require_auth();
        let mut proposal = read_proposal(&env, proposal_id)?;
        if env.ledger().timestamp() >= proposal.voting_ends_at || proposal.executed {
            return Err(ContractError::DeadlinePassed);
        }
        let voted = GovernanceKey::ProtocolGovVoted(proposal_id, voter.clone());
        if env.storage().persistent().has(&voted) {
            return Err(ContractError::AlreadyApproved);
        }
        let weight_key = GovernanceKey::ProtocolGovWeight(proposal_id, voter.clone());
        let weight: u32 = env.storage().persistent().get(&weight_key).unwrap_or(0);
        if weight == 0 {
            return Err(ContractError::InsufficientBalance);
        }
        extend_key_ttl_to_full_window(&env, &weight_key);
        if support {
            proposal.votes_for = proposal
                .votes_for
                .checked_add(u64::from(weight))
                .ok_or(ContractError::Overflow)?;
        } else {
            proposal.votes_against = proposal
                .votes_against
                .checked_add(u64::from(weight))
                .ok_or(ContractError::Overflow)?;
        }
        env.storage().persistent().set(&voted, &true);
        extend_key_ttl_to_full_window(&env, &voted);
        env.storage()
            .persistent()
            .set(&GovernanceKey::ProtocolGovProposal(proposal_id), &proposal);
        env.events().publish(
            (symbol_short!("gov_vote"), proposal_id, voter),
            (support, weight),
        );
        Ok(())
    }

    /// Permissionless execution, after 72h voting and a further 24h delay.
    /// At least 10% of snapshot supply must vote, with a strict yes majority.
    pub fn execute_proposal(env: Env, proposal_id: u32) -> Result<(), ContractError> {
        let mut proposal = read_proposal(&env, proposal_id)?;
        if proposal.executed {
            return Err(ContractError::ActionNotPending);
        }
        if env.ledger().timestamp() < proposal.executable_at {
            return Err(ContractError::TimelockNotElapsed);
        }
        let turnout = proposal
            .votes_for
            .checked_add(proposal.votes_against)
            .ok_or(ContractError::Overflow)?;
        if turnout * 10 < u64::from(proposal.snapshot_supply)
            || proposal.votes_for <= proposal.votes_against
        {
            return Err(ContractError::Unauthorized);
        }
        if proposal.parameter == ProtocolParameter::BondSlash {
            crate::performance_bonds::slash_after_governance(
                &env,
                &proposal.key_id,
                proposal.new_value as u32,
                &proposal.description,
            )
            .map_err(|_| ContractError::BondExecutionFailed)?;
        } else {
            let parameter_key = match proposal.parameter {
                ProtocolParameter::KeyPrice => constants::storage::KEY_PRICE,
                ProtocolParameter::CurveSlope => constants::storage::CURVE_SLOPE,
                ProtocolParameter::BondSlash => return Err(ContractError::BondExecutionFailed),
            };
            env.storage()
                .persistent()
                .set(&parameter_key, &proposal.new_value);
            extend_key_ttl_to_full_window(&env, &parameter_key);
        }
        proposal.executed = true;
        env.storage()
            .persistent()
            .set(&GovernanceKey::ProtocolGovProposal(proposal_id), &proposal);
        env.events()
            .publish((symbol_short!("gov_exec"), proposal_id), proposal);
        Ok(())
    }

    pub fn get_protocol_proposal(
        env: Env,
        proposal_id: u32,
    ) -> Result<ProtocolProposal, ContractError> {
        read_proposal(&env, proposal_id)
    }
}
