//! Holder governance with a fixed electorate, snapshot weights and delayed execution.
//!
//! Issue #1253 adds a rage-quit window between a proposal passing and its execution:
//! holders who voted against or abstained (did not vote for) may burn their keys and
//! receive a pro-rata share of the creator's fee reserve before the proposal takes
//! effect. Execution is blocked until the window closes.
use crate::{
    assert_is_admin, balance_checkpoints, constants, extend_key_ttl_to_full_window,
    read_creator_fee_recipient_balance, read_registered_creator_profile, write_creator_supply,
    ContractError, CreatorKeysContract, CreatorKeysContractArgs, CreatorKeysContractClient,
};
use common::guard::ReentrancyGuard;
use soroban_sdk::{contractimpl, contracttype, symbol_short, Address, Env, String};

pub const VOTING_SECONDS: u64 = 72 * 60 * 60;
pub const EXECUTION_DELAY_SECONDS: u64 = 24 * 60 * 60;
pub const MAX_GOVERNANCE_HOLDERS: u32 = 1_000;

/// Maximum rage-quit window in ledgers (≈7 days at ~5 s/ledger on Stellar).
pub const MAX_RAGEQUIT_WINDOW_LEDGERS: u32 = 120_960;
/// Default rage-quit window ledgers (≈24 h).
pub const DEFAULT_RAGEQUIT_WINDOW_LEDGERS: u32 = 17_280;
/// Default cap: at most 20% of snapshot supply may rage-quit per proposal.
pub const DEFAULT_MAX_RAGEQUIT_BPS: u32 = 2_000;
/// Ceiling on the per-proposal exit cap.
pub const MAX_RAGEQUIT_BPS_CEILING: u32 = 5_000;

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
    /// Ledger sequence number at which the rage-quit window opens (= voting end ledger).
    /// Zero means rage-quit is not configured for this proposal.
    pub ragequit_window_start_ledger: u32,
    /// Ledger sequence number at which the rage-quit window closes. Execution is
    /// blocked before this ledger.
    pub ragequit_window_end_ledger: u32,
    /// Running total of keys burned through rage-quit for this proposal.
    pub ragequit_exits: u32,
    /// Maximum keys that may rage-quit (max_ragequit_bps * snapshot_supply / 10_000).
    pub ragequit_cap: u32,
}

/// Rage-quit settings stored per-proposal in persistent storage.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RageQuitEvent {
    pub proposal_id: u32,
    pub holder: Address,
    pub keys_burned: u32,
    pub payout: i128,
}

#[derive(Clone)]
#[contracttype]
pub enum GovernanceKey {
    ProtocolGovConfig,
    ProtocolGovNextId,
    ProtocolGovProposal(u32),
    ProtocolGovWeight(u32, Address),
    ProtocolGovVoted(u32, Address),
    /// (proposal_id, holder) -> vote direction: true = voted for, false = voted against.
    ProtocolGovVoteDir(u32, Address),
    /// Rage-quit window length in ledgers (global setting).
    RageQuitWindowLedgers,
    /// Maximum rage-quit cap in bps of snapshot supply (global setting).
    MaxRageQuitBps,
    /// (proposal_id, holder) -> true when the holder has rage-quit on this proposal.
    RageQuitExited(u32, Address),
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

    /// Admin sets the number of ledgers the rage-quit window stays open after
    /// voting closes. Must be in `1..=MAX_RAGEQUIT_WINDOW_LEDGERS`.
    pub fn set_ragequit_window(
        env: Env,
        admin: Address,
        window_ledgers: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        if window_ledgers == 0 || window_ledgers > MAX_RAGEQUIT_WINDOW_LEDGERS {
            return Err(ContractError::InvalidRageQuitWindow);
        }
        env.storage()
            .persistent()
            .set(&GovernanceKey::RageQuitWindowLedgers, &window_ledgers);
        extend_key_ttl_to_full_window(&env, &GovernanceKey::RageQuitWindowLedgers);
        Ok(())
    }

    /// Admin sets the maximum fraction of snapshot supply that may rage-quit per
    /// proposal, expressed in basis points. Must be in `1..=MAX_RAGEQUIT_BPS_CEILING`.
    pub fn set_max_ragequit_bps(
        env: Env,
        admin: Address,
        max_bps: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        if max_bps == 0 || max_bps > MAX_RAGEQUIT_BPS_CEILING {
            return Err(ContractError::NotPositiveAmount);
        }
        env.storage()
            .persistent()
            .set(&GovernanceKey::MaxRageQuitBps, &max_bps);
        extend_key_ttl_to_full_window(&env, &GovernanceKey::MaxRageQuitBps);
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

        // Compute rage-quit window in ledgers using the stored setting or default.
        let rq_window_ledgers: u32 = env
            .storage()
            .persistent()
            .get(&GovernanceKey::RageQuitWindowLedgers)
            .unwrap_or(DEFAULT_RAGEQUIT_WINDOW_LEDGERS);
        let max_rq_bps: u32 = env
            .storage()
            .persistent()
            .get(&GovernanceKey::MaxRageQuitBps)
            .unwrap_or(DEFAULT_MAX_RAGEQUIT_BPS);

        // Rage-quit window opens when voting ends (measured in ledger sequence).
        // We store it as a ledger offset: at proposal creation we do not know the
        // exact future ledger, so we store the *ledger count* and use the current
        // ledger + an approximate conversion. For simplicity, the window start and
        // end are stored as ledger sequence numbers captured at creation time.
        // Approximate ledgers for VOTING_SECONDS at ~5 s/ledger
        let voting_ledgers = (VOTING_SECONDS / 5) as u32;
        let rq_start = snapshot_ledger
            .checked_add(voting_ledgers)
            .ok_or(ContractError::Overflow)?;
        let rq_end = rq_start
            .checked_add(rq_window_ledgers)
            .ok_or(ContractError::Overflow)?;

        let ragequit_cap = (u64::from(supply) * u64::from(max_rq_bps) / 10_000) as u32;

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
            ragequit_window_start_ledger: rq_start,
            ragequit_window_end_ledger: rq_end,
            ragequit_exits: 0,
            ragequit_cap,
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
        // Store the vote direction so rage-quit eligibility can check it.
        let dir_key = GovernanceKey::ProtocolGovVoteDir(proposal_id, voter.clone());
        env.storage().persistent().set(&dir_key, &support);
        extend_key_ttl_to_full_window(&env, &dir_key);
        env.storage()
            .persistent()
            .set(&GovernanceKey::ProtocolGovProposal(proposal_id), &proposal);
        env.events().publish(
            (symbol_short!("gov_vote"), proposal_id, voter),
            (support, weight),
        );
        Ok(())
    }

    /// Burn `amount` keys and receive a pro-rata share of the creator's fee reserve.
    ///
    /// Eligibility:
    /// - The holder must NOT have voted in favour of the proposal.
    /// - The holder must have held keys at the snapshot (`ProtocolGovWeight > 0`).
    /// - The rage-quit window must be open (after voting ends, before `ragequit_window_end_ledger`).
    /// - The proposal must not have been executed yet.
    /// - The cumulative exits for this proposal must not exceed the cap.
    /// - The holder must not have already rage-quit on this proposal.
    pub fn ragequit(
        env: Env,
        holder: Address,
        proposal_id: u32,
        amount: u32,
    ) -> Result<i128, ContractError> {
        holder.require_auth();
        let _guard = ReentrancyGuard::enter(&env);

        if amount == 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let mut proposal = read_proposal(&env, proposal_id)?;

        if proposal.executed {
            return Err(ContractError::ActionNotPending);
        }

        // Check the rage-quit window is open.
        let now = env.ledger().sequence();
        if now < proposal.ragequit_window_start_ledger || now >= proposal.ragequit_window_end_ledger
        {
            return Err(ContractError::RageQuitWindowClosed);
        }

        // Check the holder did not vote for the proposal (they voted against or abstained).
        // ProtocolGovVoteDir stores true=voted-for, false=voted-against.
        // If the key is absent the holder abstained (did not vote) — eligible.
        let dir_key = GovernanceKey::ProtocolGovVoteDir(proposal_id, holder.clone());
        let voted_support: bool = env.storage().persistent().get(&dir_key).unwrap_or(false);
        if voted_support {
            return Err(ContractError::RageQuitVotedFor);
        }

        // Check the holder had a snapshot weight (held keys at proposal time).
        let weight_key = GovernanceKey::ProtocolGovWeight(proposal_id, holder.clone());
        let snapshot_weight: u32 = env.storage().persistent().get(&weight_key).unwrap_or(0);
        if snapshot_weight == 0 {
            return Err(ContractError::RageQuitNotEligible);
        }

        // Check the holder hasn't already rage-quit on this proposal.
        let exited_key = GovernanceKey::RageQuitExited(proposal_id, holder.clone());
        if env.storage().persistent().has(&exited_key) {
            return Err(ContractError::AlreadyApproved);
        }

        // Check cap.
        let new_exits = proposal
            .ragequit_exits
            .checked_add(amount)
            .ok_or(ContractError::Overflow)?;
        if new_exits > proposal.ragequit_cap {
            return Err(ContractError::RageQuitCapReached);
        }

        // Check the holder has enough liquid balance.
        let balance_key = constants::storage::holder_balance_key(&proposal.key_id, &holder);
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if current_balance < amount {
            return Err(ContractError::InsufficientBalance);
        }

        // Compute pro-rata payout: payout = amount * reserve / snapshot_supply.
        // We use snapshot_supply (not live supply) so late exits don't front-run early ones.
        let reserve = read_creator_fee_recipient_balance(&env, &proposal.key_id);
        let payout = if proposal.snapshot_supply > 0 && reserve > 0 {
            (reserve
                .checked_mul(i128::from(amount))
                .ok_or(ContractError::Overflow)?)
                / i128::from(proposal.snapshot_supply)
        } else {
            0i128
        };

        // --- State mutations ---

        // 1. Deduct holder balance.
        let new_balance = current_balance
            .checked_sub(amount)
            .ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&balance_key, &new_balance);
        extend_key_ttl_to_full_window(&env, &balance_key);

        // 2. Reduce supply.
        let mut profile = read_registered_creator_profile(&env, &proposal.key_id)?;
        if new_balance == 0 && current_balance > 0 {
            profile.holder_count = profile.holder_count.saturating_sub(1);
        }
        profile.supply = profile
            .supply
            .checked_sub(amount)
            .ok_or(ContractError::Overflow)?;
        let profile_key = constants::storage::creator(&proposal.key_id);
        env.storage().persistent().set(&profile_key, &profile);
        write_creator_supply(&env, &proposal.key_id, profile.supply);

        // 3. Deduct from reserve (fees are skipped — full pro-rata payout).
        if payout > 0 {
            let new_reserve = reserve.checked_sub(payout).ok_or(ContractError::Overflow)?;
            let reserve_key = constants::storage::creator_fee_balance(&proposal.key_id);
            env.storage().persistent().set(&reserve_key, &new_reserve);
            extend_key_ttl_to_full_window(&env, &reserve_key);
        }

        // 4. Mark holder as exited.
        env.storage().persistent().set(&exited_key, &true);
        extend_key_ttl_to_full_window(&env, &exited_key);

        // 5. Update proposal exit counter.
        proposal.ragequit_exits = new_exits;
        let proposal_key = GovernanceKey::ProtocolGovProposal(proposal_id);
        env.storage().persistent().set(&proposal_key, &proposal);
        extend_key_ttl_to_full_window(&env, &proposal_key);

        // 6. Emit event.
        env.events().publish(
            (symbol_short!("rq_exit"), proposal_id, holder.clone()),
            RageQuitEvent {
                proposal_id,
                holder,
                keys_burned: amount,
                payout,
            },
        );

        Ok(payout)
    }

    /// Returns `true` when the rage-quit window for `proposal_id` is currently open.
    pub fn ragequit_open(env: Env, proposal_id: u32) -> Result<bool, ContractError> {
        let proposal = read_proposal(&env, proposal_id)?;
        let now = env.ledger().sequence();
        Ok(!proposal.executed
            && now >= proposal.ragequit_window_start_ledger
            && now < proposal.ragequit_window_end_ledger)
    }

    /// Permissionless execution, after 72h voting and a further 24h delay.
    /// At least 10% of snapshot supply must vote, with a strict yes majority.
    /// Blocked until the rage-quit window has closed.
    pub fn execute_proposal(env: Env, proposal_id: u32) -> Result<(), ContractError> {
        let _guard = ReentrancyGuard::enter(&env);
        let mut proposal = read_proposal(&env, proposal_id)?;
        if proposal.executed {
            return Err(ContractError::ActionNotPending);
        }
        if env.ledger().timestamp() < proposal.executable_at {
            return Err(ContractError::TimelockNotElapsed);
        }
        // Block execution until the rage-quit window has closed.
        let now = env.ledger().sequence();
        if now < proposal.ragequit_window_end_ledger {
            return Err(ContractError::RageQuitWindowClosed);
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
