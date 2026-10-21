use crate::{
    events, extend_key_ttl_to_full_window, read_registered_creator_profile, ContractError,
    CreatorKeysContract, CreatorKeysContractArgs, CreatorKeysContractClient, DataKey,
};
use soroban_sdk::{contractimpl, contracttype, Address, Env};

pub const CURVE_UPDATE_DELAY_SECONDS: u64 = 48 * 60 * 60;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CurveUpdateProposal {
    pub id: u32,
    pub creator: Address,
    pub new_slope: i128,
    pub new_intercept: i128,
    pub eta: u64,
    pub pending: bool,
}

#[contractimpl]
impl CreatorKeysContract {
    pub fn propose_curve_update(
        env: Env,
        key_id: Address,
        new_slope: i128,
        new_intercept: i128,
    ) -> Result<u32, ContractError> {
        key_id.require_auth();
        read_registered_creator_profile(&env, &key_id)?;
        if new_slope < 0 || new_intercept <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let id: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::CurveUpdateNextId)
            .unwrap_or(0);
        let next_id = id.checked_add(1).ok_or(ContractError::Overflow)?;
        let eta = env
            .ledger()
            .timestamp()
            .checked_add(CURVE_UPDATE_DELAY_SECONDS)
            .ok_or(ContractError::Overflow)?;
        let proposal = CurveUpdateProposal {
            id,
            creator: key_id.clone(),
            new_slope,
            new_intercept,
            eta,
            pending: true,
        };
        let proposal_key = DataKey::CurveUpdateProposal(id);
        env.storage().persistent().set(&proposal_key, &proposal);
        extend_key_ttl_to_full_window(&env, &proposal_key);
        env.storage()
            .persistent()
            .set(&DataKey::CurveUpdateNextId, &next_id);
        extend_key_ttl_to_full_window(&env, &DataKey::CurveUpdateNextId);
        env.events().publish(
            events::curve_update_proposed_topics(&key_id, id),
            events::CurveUpdateProposedEvent {
                proposal_id: id,
                creator: key_id,
                new_slope,
                new_intercept,
                eta,
            },
        );
        Ok(id)
    }

    pub fn execute_curve_update(env: Env, proposal_id: u32) -> Result<(), ContractError> {
        let proposal_key = DataKey::CurveUpdateProposal(proposal_id);
        let mut proposal: CurveUpdateProposal = env
            .storage()
            .persistent()
            .get(&proposal_key)
            .ok_or(ContractError::ProposalNotFound)?;
        extend_key_ttl_to_full_window(&env, &proposal_key);
        if !proposal.pending {
            return Err(ContractError::ActionNotPending);
        }
        if env.ledger().timestamp() < proposal.eta {
            return Err(ContractError::TimelockNotElapsed);
        }

        let old_slope: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorCurveSlope(proposal.creator.clone()))
            .unwrap_or_else(|| {
                env.storage()
                    .persistent()
                    .get(&crate::constants::storage::CURVE_SLOPE)
                    .unwrap_or(0)
            });
        let old_intercept: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorCurveIntercept(proposal.creator.clone()))
            .unwrap_or_else(|| {
                env.storage()
                    .persistent()
                    .get(&crate::constants::storage::KEY_PRICE)
                    .unwrap_or(0)
            });

        let slope_key = DataKey::CreatorCurveSlope(proposal.creator.clone());
        let intercept_key = DataKey::CreatorCurveIntercept(proposal.creator.clone());
        env.storage()
            .persistent()
            .set(&slope_key, &proposal.new_slope);
        env.storage()
            .persistent()
            .set(&intercept_key, &proposal.new_intercept);
        extend_key_ttl_to_full_window(&env, &slope_key);
        extend_key_ttl_to_full_window(&env, &intercept_key);

        proposal.pending = false;
        env.storage().persistent().set(&proposal_key, &proposal);
        env.events().publish(
            events::curve_update_executed_topics(&proposal.creator, proposal_id),
            events::CurveUpdateExecutedEvent {
                proposal_id,
                creator: proposal.creator,
                old_slope,
                old_intercept,
                new_slope: proposal.new_slope,
                new_intercept: proposal.new_intercept,
            },
        );
        Ok(())
    }

    pub fn cancel_curve_update(env: Env, proposal_id: u32) -> Result<(), ContractError> {
        let proposal_key = DataKey::CurveUpdateProposal(proposal_id);
        let mut proposal: CurveUpdateProposal = env
            .storage()
            .persistent()
            .get(&proposal_key)
            .ok_or(ContractError::ProposalNotFound)?;
        extend_key_ttl_to_full_window(&env, &proposal_key);
        if !proposal.pending {
            return Err(ContractError::ActionNotPending);
        }
        proposal.creator.require_auth();
        proposal.pending = false;
        env.storage().persistent().set(&proposal_key, &proposal);
        Ok(())
    }
}
