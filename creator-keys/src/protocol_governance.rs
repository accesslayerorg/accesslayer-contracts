//! Holder governance with a fixed electorate, snapshot weights and delayed execution.
//! Also contains conviction voting for continuous proposal funding signals.
use crate::{
    assert_is_admin, balance_checkpoints, constants, extend_key_ttl_to_full_window,
    read_registered_creator_profile, ContractError, CreatorKeysContract, CreatorKeysContractArgs,
    CreatorKeysContractClient,
};
use common::guard::ReentrancyGuard;
use soroban_sdk::{contractimpl, contracttype, symbol_short, Address, Env, String};

pub const VOTING_SECONDS: u64 = 72 * 60 * 60;
pub const EXECUTION_DELAY_SECONDS: u64 = 24 * 60 * 60;
pub const MAX_GOVERNANCE_HOLDERS: u32 = 1_000;

/// Fixed-point scale (1e9) used for conviction math.
pub const CONVICTION_SCALE: i128 = 1_000_000_000;
/// Per-ledger-interval decay factor, scaled by CONVICTION_SCALE (~0.99).
pub const CONVICTION_DECAY: i128 = 990_000_000;
/// Base threshold for a request of zero treasury share, in conviction units.
pub const BASE_THRESHOLD: i128 = 1_000 * CONVICTION_SCALE;
/// Basis points of treasury at which threshold doubles relative to base.
pub const TREASURY_SHARE_SCALE_BPS: i128 = 10_000;

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

/// Conviction voting state for a funding proposal.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ConvictionProposal {
    pub id: u32,
    pub proposer: Address,
    pub requested_amount: i128,
    /// Treasury size used to scale the threshold, in the same units as requested_amount.
    pub treasury_at_creation: i128,
    /// Total stake currently held across all holders for this proposal.
    pub total_stake: i128,
    /// Accumulated conviction (scaled by CONVICTION_SCALE).
    pub conviction: i128,
    /// Ledger sequence at which `conviction` was last updated.
    pub last_ledger: u32,
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
    ConvNextId,
    ConvProposal(u32),
    /// Per-holder stake on a conviction proposal.
    ConvStake(u32, Address),
    /// Total stake a holder has committed across all conviction proposals.
    ConvHolderTotal(Address),
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

fn read_conviction(env: &Env, id: u32) -> Result<ConvictionProposal, ContractError> {
    let key = GovernanceKey::ConvProposal(id);
    let proposal = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContractError::ProposalNotFound)?;
    extend_key_ttl_to_full_window(env, &key);
    Ok(proposal)
}

/// Applies `elapsed` ledger intervals of exponential decay to `conviction`.
fn decay_conviction(conviction: i128, elapsed: u32) -> Result<i128, ContractError> {
    let mut value = conviction;
    for _ in 0..elapsed {
        value = value
            .checked_mul(CONVICTION_DECAY)
            .ok_or(ContractError::Overflow)?
            / CONVICTION_SCALE;
        if value == 0 {
            break;
        }
    }
    Ok(value)
}

/// Brings a proposal's conviction up to the current ledger, applying decay for
/// the elapsed intervals and then adding the current stake (one interval's worth).
fn advance_conviction(env: &Env, proposal: &mut ConvictionProposal) -> Result<(), ContractError> {
    let now = env.ledger().sequence();
    let elapsed = now.saturating_sub(proposal.last_ledger);
    if elapsed > 0 {
        let decayed = decay_conviction(proposal.conviction, elapsed)?;
        proposal.conviction = decayed
            .checked_add(
                proposal
                    .total_stake
                    .checked_mul(CONVICTION_SCALE)
                    .ok_or(ContractError::Overflow)?
                    .checked_mul(i128::from(elapsed))
                    .ok_or(ContractError::Overflow)?,
            )
            .ok_or(ContractError::Overflow)?;
        proposal.last_ledger = now;
    }
    Ok(())
}

/// Threshold required to pass a request, scaling linearly with its share of the treasury.
pub fn threshold_for_amount(requested_amount: i128, treasury: i128) -> Result<i128, ContractError> {
    if requested_amount <= 0 || treasury <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    let share_bps = requested_amount
        .checked_mul(TREASURY_SHARE_SCALE_BPS)
        .ok_or(ContractError::Overflow)?
        / treasury;
    BASE_THRESHOLD
        .checked_add(
            BASE_THRESHOLD
                .checked_mul(share_bps)
                .ok_or(ContractError::Overflow)?
                / TREASURY_SHARE_SCALE_BPS,
        )
        .ok_or(ContractError::Overflow)
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
        // Snapshot weights from the historical checkpoint at this ledger so a
        // later trade cannot change the electorate after the proposal opens.
        let snapshot_ledger = env.ledger().sequence();
        for holder in holders.iter() {
            let balance = balance_checkpoints::balance_at_or_live(
                &env,
                &config.key_id,
                &holder,
                snapshot_ledger,
            );
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
        let _guard = ReentrancyGuard::enter(&env);
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

    /// Creates a conviction-voting funding proposal. `treasury` is the treasury
    /// balance used to scale the passing threshold for `requested_amount`.
    pub fn create_conviction_proposal(
        env: Env,
        proposer: Address,
        requested_amount: i128,
        treasury: i128,
    ) -> Result<u32, ContractError> {
        proposer.require_auth();
        if requested_amount <= 0 || treasury <= 0 || requested_amount > treasury {
            return Err(ContractError::NotPositiveAmount);
        }
        // Validate threshold computation up front.
        threshold_for_amount(requested_amount, treasury)?;
        let id: u32 = env
            .storage()
            .persistent()
            .get(&GovernanceKey::ConvNextId)
            .unwrap_or(0);
        let next_id = id.checked_add(1).ok_or(ContractError::Overflow)?;
        let proposal = ConvictionProposal {
            id,
            proposer,
            requested_amount,
            treasury_at_creation: treasury,
            total_stake: 0,
            conviction: 0,
            last_ledger: env.ledger().sequence(),
            executed: false,
        };
        let key = GovernanceKey::ConvProposal(id);
        env.storage().persistent().set(&key, &proposal);
        extend_key_ttl_to_full_window(&env, &key);
        env.storage()
            .persistent()
            .set(&GovernanceKey::ConvNextId, &next_id);
        extend_key_ttl_to_full_window(&env, &GovernanceKey::ConvNextId);
        Ok(id)
    }

    /// Stakes `amount` of the holder's key balance behind a conviction proposal.
    /// Total stake across all proposals is capped at the holder's key balance.
    pub fn stake_on(
        env: Env,
        holder: Address,
        proposal_id: u32,
        amount: i128,
    ) -> Result<(), ContractError> {
        holder.require_auth();
        if amount <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        let mut proposal = read_conviction(&env, proposal_id)?;
        if proposal.executed {
            return Err(ContractError::ActionNotPending);
        }
        let config: GovernanceConfig = env
            .storage()
            .persistent()
            .get(&GovernanceKey::ProtocolGovConfig)
            .ok_or(ContractError::Unauthorized)?;
        let balance = i128::from(Self::get_key_balance(
            env.clone(),
            config.key_id,
            holder.clone(),
        ));
        let holder_total_key = GovernanceKey::ConvHolderTotal(holder.clone());
        let holder_total: i128 = env
            .storage()
            .persistent()
            .get(&holder_total_key)
            .unwrap_or(0);
        let new_holder_total = holder_total
            .checked_add(amount)
            .ok_or(ContractError::Overflow)?;
        if new_holder_total > balance {
            return Err(ContractError::InsufficientBalance);
        }

        advance_conviction(&env, &mut proposal)?;

        let stake_key = GovernanceKey::ConvStake(proposal_id, holder.clone());
        let current: i128 = env.storage().persistent().get(&stake_key).unwrap_or(0);
        let new_stake = current.checked_add(amount).ok_or(ContractError::Overflow)?;
        proposal.total_stake = proposal
            .total_stake
            .checked_add(amount)
            .ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&stake_key, &new_stake);
        extend_key_ttl_to_full_window(&env, &stake_key);
        env.storage()
            .persistent()
            .set(&holder_total_key, &new_holder_total);
        extend_key_ttl_to_full_window(&env, &holder_total_key);
        let key = GovernanceKey::ConvProposal(proposal_id);
        env.storage().persistent().set(&key, &proposal);
        extend_key_ttl_to_full_window(&env, &key);

        env.events()
            .publish((symbol_short!("staked"), proposal_id, holder), amount);
        Ok(())
    }

    /// Withdraws `amount` of stake from a conviction proposal. Conviction decays
    /// from this point on because total stake drops.
    pub fn withdraw_stake(
        env: Env,
        holder: Address,
        proposal_id: u32,
        amount: i128,
    ) -> Result<(), ContractError> {
        holder.require_auth();
        if amount <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        let mut proposal = read_conviction(&env, proposal_id)?;
        if proposal.executed {
            return Err(ContractError::ActionNotPending);
        }
        let stake_key = GovernanceKey::ConvStake(proposal_id, holder.clone());
        let current: i128 = env.storage().persistent().get(&stake_key).unwrap_or(0);
        if amount > current {
            return Err(ContractError::InsufficientBalance);
        }

        advance_conviction(&env, &mut proposal)?;

        let new_stake = current - amount;
        proposal.total_stake -= amount;
        if new_stake == 0 {
            env.storage().persistent().remove(&stake_key);
        } else {
            env.storage().persistent().set(&stake_key, &new_stake);
            extend_key_ttl_to_full_window(&env, &stake_key);
        }
        let holder_total_key = GovernanceKey::ConvHolderTotal(holder.clone());
        let holder_total: i128 = env
            .storage()
            .persistent()
            .get(&holder_total_key)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&holder_total_key, &(holder_total - amount));
        extend_key_ttl_to_full_window(&env, &holder_total_key);
        let key = GovernanceKey::ConvProposal(proposal_id);
        env.storage().persistent().set(&key, &proposal);
        extend_key_ttl_to_full_window(&env, &key);

        env.events()
            .publish((symbol_short!("unstaked"), proposal_id, holder), amount);
        Ok(())
    }

    /// Current conviction of a proposal, decayed to the present ledger.
    pub fn conviction_of(env: Env, proposal_id: u32) -> Result<i128, ContractError> {
        let mut proposal = read_conviction(&env, proposal_id)?;
        advance_conviction(&env, &mut proposal)?;
        Ok(proposal.conviction)
    }

    /// Threshold a request must reach, scaled by its share of the treasury
    /// recorded at proposal creation.
    pub fn threshold_for(env: Env, proposal_id: u32) -> Result<i128, ContractError> {
        let proposal = read_conviction(&env, proposal_id)?;
        threshold_for_amount(proposal.requested_amount, proposal.treasury_at_creation)
    }

    /// Executes a conviction proposal once its conviction meets its threshold.
    pub fn execute(env: Env, proposal_id: u32) -> Result<(), ContractError> {
        let _guard = ReentrancyGuard::enter(&env);
        let mut proposal = read_conviction(&env, proposal_id)?;
        if proposal.executed {
            return Err(ContractError::ActionNotPending);
        }
        advance_conviction(&env, &mut proposal)?;
        let threshold =
            threshold_for_amount(proposal.requested_amount, proposal.treasury_at_creation)?;
        if proposal.conviction < threshold {
            return Err(ContractError::Unauthorized);
        }
        proposal.executed = true;
        let key = GovernanceKey::ConvProposal(proposal_id);
        env.storage().persistent().set(&key, &proposal);
        extend_key_ttl_to_full_window(&env, &key);
        env.events().publish(
            (symbol_short!("executed"), proposal_id),
            (proposal.proposer.clone(), proposal.requested_amount),
        );
        Ok(())
    }

    pub fn get_conviction_proposal(
        env: Env,
        proposal_id: u32,
    ) -> Result<ConvictionProposal, ContractError> {
        read_conviction(&env, proposal_id)
    }
}
