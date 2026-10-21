#![no_std]

use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contracterror, contractimpl, contracttype, Address, Env, Symbol, Val, Vec,
};

/// Maximum signer count keeps vote counting and configuration updates bounded.
pub const MAX_SIGNERS: u32 = 20;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionKind {
    Freeze,
    Unfreeze,
    AclChange,
    CurveMigration,
}

/// A call proposal targets a contract method which must authorize this multisig
/// contract as its privileged actor. Signer updates use the same approval flow.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractCallAction {
    pub kind: ActionKind,
    pub target: Address,
    pub function: Symbol,
    pub args: Vec<Val>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateSignersAction {
    pub signers: Vec<Address>,
    pub threshold: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalAction {
    ContractCall(ContractCallAction),
    UpdateSigners(UpdateSignersAction),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u32,
    pub proposer: Address,
    pub action: ProposalAction,
    /// Approval membership and threshold are snapshotted at proposal time.
    pub signers: Vec<Address>,
    pub threshold: u32,
    pub approvals: u32,
    pub rejected: bool,
    pub executed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum DataKey {
    Signers,
    Threshold,
    NextProposalId,
    Proposal(u32),
    Approval(u32, Address),
    Rejection(u32, Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MultisigError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidSignerSet = 4,
    ProposalNotFound = 5,
    AlreadyVoted = 6,
    ProposalRejected = 7,
    ThresholdNotReached = 8,
    AlreadyExecuted = 9,
    InvalidAction = 10,
}

#[contract]
pub struct MultisigAdminContract;

fn read_signers(env: &Env) -> Result<Vec<Address>, MultisigError> {
    env.storage()
        .instance()
        .get(&DataKey::Signers)
        .ok_or(MultisigError::NotInitialized)
}

fn read_threshold(env: &Env) -> Result<u32, MultisigError> {
    env.storage()
        .instance()
        .get(&DataKey::Threshold)
        .ok_or(MultisigError::NotInitialized)
}

fn is_signer(signers: &Vec<Address>, candidate: &Address) -> bool {
    signers.iter().any(|signer| signer == *candidate)
}

fn assert_signer(signers: &Vec<Address>, candidate: &Address) -> Result<(), MultisigError> {
    if is_signer(signers, candidate) {
        Ok(())
    } else {
        Err(MultisigError::Unauthorized)
    }
}

fn validate_signers(signers: &Vec<Address>, threshold: u32) -> Result<(), MultisigError> {
    if signers.is_empty()
        || signers.len() > MAX_SIGNERS
        || threshold == 0
        || threshold > signers.len()
    {
        return Err(MultisigError::InvalidSignerSet);
    }
    for (index, signer) in signers.iter().enumerate() {
        for other in signers.iter().skip(index + 1) {
            if signer == other {
                return Err(MultisigError::InvalidSignerSet);
            }
        }
    }
    Ok(())
}

fn load_proposal(env: &Env, proposal_id: u32) -> Result<Proposal, MultisigError> {
    env.storage()
        .persistent()
        .get(&DataKey::Proposal(proposal_id))
        .ok_or(MultisigError::ProposalNotFound)
}

fn save_proposal(env: &Env, proposal: &Proposal) {
    env.storage()
        .persistent()
        .set(&DataKey::Proposal(proposal.id), proposal);
}

fn emit_action_event(env: &Env, name: Symbol, proposal_id: u32, actor: &Address) {
    env.events().publish((name, proposal_id, actor.clone()), ());
}

#[contractimpl]
impl MultisigAdminContract {
    /// One-time initialization of the signer set and approval threshold.
    pub fn initialize(
        env: Env,
        initializer: Address,
        signers: Vec<Address>,
        threshold: u32,
    ) -> Result<(), MultisigError> {
        if env.storage().instance().has(&DataKey::Signers) {
            return Err(MultisigError::AlreadyInitialized);
        }
        initializer.require_auth();
        validate_signers(&signers, threshold)?;
        assert_signer(&signers, &initializer)?;
        env.storage().instance().set(&DataKey::Signers, &signers);
        env.storage()
            .instance()
            .set(&DataKey::Threshold, &threshold);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &0u32);
        env.events().publish(
            (Symbol::new(&env, "initialized"), initializer),
            (signers, threshold),
        );
        Ok(())
    }

    /// Any configured signer may propose a privileged call or a signer-set update.
    pub fn propose_action(
        env: Env,
        proposer: Address,
        action: ProposalAction,
    ) -> Result<u32, MultisigError> {
        proposer.require_auth();
        let signers = read_signers(&env)?;
        assert_signer(&signers, &proposer)?;
        if let ProposalAction::UpdateSigners(update) = &action {
            validate_signers(&update.signers, update.threshold)?;
        }
        let threshold = read_threshold(&env)?;
        let proposal_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextProposalId)
            .unwrap_or(0);
        let next_id = proposal_id
            .checked_add(1)
            .ok_or(MultisigError::InvalidAction)?;
        let proposal = Proposal {
            id: proposal_id,
            proposer: proposer.clone(),
            action,
            signers,
            threshold,
            approvals: 0,
            rejected: false,
            executed: false,
        };
        save_proposal(&env, &proposal);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &next_id);
        emit_action_event(&env, Symbol::new(&env, "proposed"), proposal_id, &proposer);
        Ok(proposal_id)
    }

    /// Records one approval per authorized signer.
    pub fn sign_proposal(env: Env, signer: Address, proposal_id: u32) -> Result<(), MultisigError> {
        signer.require_auth();
        let mut proposal = load_proposal(&env, proposal_id)?;
        assert_signer(&proposal.signers, &signer)?;
        if proposal.rejected {
            return Err(MultisigError::ProposalRejected);
        }
        if proposal.executed {
            return Err(MultisigError::AlreadyExecuted);
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::Rejection(proposal_id, signer.clone()))
        {
            return Err(MultisigError::AlreadyVoted);
        }
        let vote_key = DataKey::Approval(proposal_id, signer.clone());
        if env.storage().persistent().has(&vote_key) {
            return Err(MultisigError::AlreadyVoted);
        }
        env.storage().persistent().set(&vote_key, &true);
        proposal.approvals = proposal
            .approvals
            .checked_add(1)
            .ok_or(MultisigError::InvalidAction)?;
        save_proposal(&env, &proposal);
        emit_action_event(&env, Symbol::new(&env, "signed"), proposal_id, &signer);
        Ok(())
    }

    /// A single authorized rejection is a permanent veto for the proposal.
    pub fn reject_proposal(
        env: Env,
        signer: Address,
        proposal_id: u32,
    ) -> Result<(), MultisigError> {
        signer.require_auth();
        let mut proposal = load_proposal(&env, proposal_id)?;
        assert_signer(&proposal.signers, &signer)?;
        if proposal.executed {
            return Err(MultisigError::AlreadyExecuted);
        }
        if proposal.rejected {
            return Err(MultisigError::ProposalRejected);
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::Approval(proposal_id, signer.clone()))
        {
            return Err(MultisigError::AlreadyVoted);
        }
        let vote_key = DataKey::Rejection(proposal_id, signer.clone());
        if env.storage().persistent().has(&vote_key) {
            return Err(MultisigError::AlreadyVoted);
        }
        env.storage().persistent().set(&vote_key, &true);
        proposal.rejected = true;
        save_proposal(&env, &proposal);
        emit_action_event(&env, Symbol::new(&env, "rejected"), proposal_id, &signer);
        Ok(())
    }

    /// Executes an approved proposal. Call proposals invoke the target as this
    /// contract, allowing targets to require authorization from the multisig.
    /// Execution is permissionless; `executor` is authenticated for auditability.
    pub fn execute_proposal(
        env: Env,
        executor: Address,
        proposal_id: u32,
    ) -> Result<(), MultisigError> {
        executor.require_auth();
        let mut proposal = load_proposal(&env, proposal_id)?;
        if proposal.rejected {
            return Err(MultisigError::ProposalRejected);
        }
        if proposal.executed {
            return Err(MultisigError::AlreadyExecuted);
        }
        if proposal.approvals < proposal.threshold {
            return Err(MultisigError::ThresholdNotReached);
        }

        match proposal.action.clone() {
            ProposalAction::ContractCall(call) => {
                let context = ContractContext {
                    contract: call.target.clone(),
                    fn_name: call.function.clone(),
                    args: call.args.clone(),
                };
                let invocation = InvokerContractAuthEntry::Contract(SubContractInvocation {
                    context,
                    sub_invocations: Vec::new(&env),
                });
                env.authorize_as_current_contract(Vec::from_array(&env, [invocation]));
                let _: Val = env.invoke_contract(&call.target, &call.function, call.args);
            }
            ProposalAction::UpdateSigners(update) => {
                // The proposal's approval threshold and electorate are snapshots
                // from creation, so the current signer set changes only after
                // its then-current threshold has approved this update.
                validate_signers(&update.signers, update.threshold)?;
                env.storage()
                    .instance()
                    .set(&DataKey::Signers, &update.signers);
                env.storage()
                    .instance()
                    .set(&DataKey::Threshold, &update.threshold);
            }
        }

        proposal.executed = true;
        save_proposal(&env, &proposal);
        emit_action_event(&env, Symbol::new(&env, "executed"), proposal_id, &executor);
        Ok(())
    }

    pub fn get_proposal(env: Env, proposal_id: u32) -> Result<Proposal, MultisigError> {
        load_proposal(&env, proposal_id)
    }

    pub fn get_signers(env: Env) -> Result<Vec<Address>, MultisigError> {
        read_signers(&env)
    }

    pub fn get_threshold(env: Env) -> Result<u32, MultisigError> {
        read_threshold(&env)
    }
}
