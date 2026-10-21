use crate::ContractError;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AclEntry {
    pub contract_address: Address,
    pub permissions: u32,
    pub is_active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyMergeProposal {
    pub source_key: Address,
    pub target_key: Address,
    pub conversion_ratio_bps: u32,   // 10000 = 1:1
    pub approval_threshold_bps: u32, // share of cast weight that must approve
    pub votes_for: u32,
    pub votes_against: u32,
    pub total_voted_weight: u32,
    pub is_approved_by_admin: bool,
    pub is_executed: bool,
}

/// A single holder's recorded vote on a merge proposal.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MergeVoteRecord {
    pub approve: bool,
    pub weight: u32,
}

/// One migrated source-key position, kept as the on-chain migration log.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MergeMigrationEntry {
    pub holder: Address,
    pub liquid_migrated: u32,
    pub staked_migrated: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeySunsetStatus {
    pub last_trade_ledger: u32,
    pub is_sunset_pending: bool,
    pub is_deprecated: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum EmwulrdDataKey {
    Acl(Address),
    MaxBuyPerTx(Address),
    MergeProposal(Address), // source_key -> KeyMergeProposal
    /// source_key -> Vec<Address>, the holders whose positions are migrated.
    MergeHolders(Address),
    /// (source_key, voter) -> MergeVoteRecord
    MergeVote(Address, Address),
    /// source_key -> Vec<MergeMigrationEntry>
    MergeMigrationLog(Address),
    SunsetStatus(Address),
}

pub const ACL_UPDATED_EVENT: Symbol = symbol_short!("acl_upd");
pub const BUY_LIMIT_CONFIGURED_EVENT: Symbol = symbol_short!("buy_lim");
pub const MERGE_VOTE_EVENT: Symbol = symbol_short!("merge_vt");
pub const MERGE_EXECUTED_EVENT: Symbol = symbol_short!("merge_ex");
pub const KEY_SUNSET_FLAGGED_EVENT: Symbol = symbol_short!("sunset");
pub const KEY_DEPRECATED_EVENT: Symbol = symbol_short!("deprec");

/// #948: Add external contract to cross-contract Access Control List (ACL).
pub fn add_to_acl(
    env: &Env,
    admin: &Address,
    contract_addr: &Address,
    permissions: u32,
) -> Result<(), ContractError> {
    admin.require_auth();

    let entry = AclEntry {
        contract_address: contract_addr.clone(),
        permissions,
        is_active: true,
    };

    env.storage()
        .instance()
        .set(&EmwulrdDataKey::Acl(contract_addr.clone()), &entry);

    env.events().publish(
        (ACL_UPDATED_EVENT, contract_addr.clone()),
        (permissions, true),
    );

    Ok(())
}

/// Remove contract from ACL.
pub fn remove_from_acl(
    env: &Env,
    admin: &Address,
    contract_addr: &Address,
) -> Result<(), ContractError> {
    admin.require_auth();

    let entry = AclEntry {
        contract_address: contract_addr.clone(),
        permissions: 0,
        is_active: false,
    };

    env.storage()
        .instance()
        .set(&EmwulrdDataKey::Acl(contract_addr.clone()), &entry);

    env.events()
        .publish((ACL_UPDATED_EVENT, contract_addr.clone()), (0u32, false));

    Ok(())
}

/// Verify caller has required ACL permission bitmask.
pub fn assert_acl_permission(
    env: &Env,
    caller: &Address,
    required_perm: u32,
) -> Result<(), ContractError> {
    let entry: Option<AclEntry> = env
        .storage()
        .instance()
        .get(&EmwulrdDataKey::Acl(caller.clone()));
    match entry {
        Some(e) if e.is_active && (e.permissions & required_perm) == required_perm => Ok(()),
        _ => Err(ContractError::Unauthorized),
    }
}

/// #947: Configure maximum buy quantity per transaction for a creator key.
pub fn set_max_buy_limit(
    env: &Env,
    creator: &Address,
    max_buy_qty: u32,
) -> Result<(), ContractError> {
    creator.require_auth();
    if max_buy_qty == 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    env.storage()
        .instance()
        .set(&EmwulrdDataKey::MaxBuyPerTx(creator.clone()), &max_buy_qty);

    env.events()
        .publish((BUY_LIMIT_CONFIGURED_EVENT, creator.clone()), max_buy_qty);

    Ok(())
}

/// Read max buy per tx limit.
pub fn get_max_buy_limit(env: &Env, creator: &Address) -> u32 {
    env.storage()
        .instance()
        .get(&EmwulrdDataKey::MaxBuyPerTx(creator.clone()))
        .unwrap_or(u32::MAX)
}

/// Verify quantity does not exceed transaction buy limit.
pub fn check_tx_buy_limit(
    env: &Env,
    creator: &Address,
    requested_qty: u32,
) -> Result<(), ContractError> {
    let limit = get_max_buy_limit(env, creator);
    if requested_qty > limit {
        return Err(ContractError::WalletCapExceeded);
    }
    Ok(())
}

/// #949: Propose key consolidation/merge into a target key.
///
/// The creator supplies the complete set of `source_holders` that currently
/// hold a position on `source_key`; this list is the enumeration the async
/// execution path migrates from. Duplicate addresses are rejected so the
/// migration can never credit the same position twice.
pub fn propose_key_merge(
    env: &Env,
    creator: &Address,
    source_key: &Address,
    target_key: &Address,
    conversion_ratio_bps: u32,
    approval_threshold_bps: u32,
    source_holders: Vec<Address>,
) -> Result<(), ContractError> {
    creator.require_auth();
    if source_key == target_key
        || conversion_ratio_bps == 0
        || conversion_ratio_bps > 10_000
        || approval_threshold_bps == 0
        || approval_threshold_bps > 10_000
    {
        return Err(ContractError::InvalidFeeConfig);
    }
    if source_holders.is_empty() {
        return Err(ContractError::NoKeyHolders);
    }

    // Reject duplicate holders: a repeated address would be migrated twice.
    let len = source_holders.len();
    let mut i = 0;
    while i < len {
        let mut j = i + 1;
        while j < len {
            if source_holders.get(i) == source_holders.get(j) {
                return Err(ContractError::AlreadyRegistered);
            }
            j += 1;
        }
        i += 1;
    }

    let proposal_key = EmwulrdDataKey::MergeProposal(source_key.clone());
    if let Some(existing) = env
        .storage()
        .instance()
        .get::<_, KeyMergeProposal>(&proposal_key)
    {
        if existing.is_executed {
            return Err(ContractError::AlreadyRegistered);
        }
    }

    let proposal = KeyMergeProposal {
        source_key: source_key.clone(),
        target_key: target_key.clone(),
        conversion_ratio_bps,
        approval_threshold_bps,
        votes_for: 0,
        votes_against: 0,
        total_voted_weight: 0,
        is_approved_by_admin: false,
        is_executed: false,
    };

    env.storage().instance().set(&proposal_key, &proposal);
    env.storage().instance().set(
        &EmwulrdDataKey::MergeHolders(source_key.clone()),
        &source_holders,
    );

    Ok(())
}

/// Read the merge proposal for a source key, if one exists.
pub fn get_key_merge_proposal(env: &Env, source_key: &Address) -> Option<KeyMergeProposal> {
    env.storage()
        .instance()
        .get(&EmwulrdDataKey::MergeProposal(source_key.clone()))
}

/// Read the migration log captured when a merge executed.
pub fn get_merge_migration_log(env: &Env, source_key: &Address) -> Vec<MergeMigrationEntry> {
    env.storage()
        .instance()
        .get(&EmwulrdDataKey::MergeMigrationLog(source_key.clone()))
        .unwrap_or(Vec::new(env))
}

/// #962: Cast a merge vote as a holder of the source key.
///
/// Vote weight is the voter's liquid (non-staked) source-key balance, mirroring
/// the token-weighted poll and governance implementations. Casting a new vote
/// replaces the previous one, removing the old weight before adding the new so
/// a holder can never be counted twice.
pub fn vote_on_merge(
    env: &Env,
    source_key_holder: &Address,
    source_key: &Address,
    approve: bool,
) -> Result<(), ContractError> {
    source_key_holder.require_auth();

    let proposal_key = EmwulrdDataKey::MergeProposal(source_key.clone());
    let mut proposal: KeyMergeProposal = env
        .storage()
        .instance()
        .get(&proposal_key)
        .ok_or(ContractError::ProposalNotFound)?;

    if proposal.is_executed {
        return Err(ContractError::ActionNotPending);
    }

    let balance_key = crate::constants::storage::holder_balance_key(source_key, source_key_holder);
    let weight: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
    if weight == 0 {
        return Err(ContractError::InsufficientBalance);
    }

    let vote_key = EmwulrdDataKey::MergeVote(source_key.clone(), source_key_holder.clone());
    let previous: Option<MergeVoteRecord> = env.storage().instance().get(&vote_key);
    if let Some(prev) = previous {
        if prev.approve {
            proposal.votes_for = proposal
                .votes_for
                .checked_sub(prev.weight)
                .ok_or(ContractError::Overflow)?;
        } else {
            proposal.votes_against = proposal
                .votes_against
                .checked_sub(prev.weight)
                .ok_or(ContractError::Overflow)?;
        }
        proposal.total_voted_weight = proposal
            .total_voted_weight
            .checked_sub(prev.weight)
            .ok_or(ContractError::Overflow)?;
    }

    if approve {
        proposal.votes_for = proposal
            .votes_for
            .checked_add(weight)
            .ok_or(ContractError::Overflow)?;
    } else {
        proposal.votes_against = proposal
            .votes_against
            .checked_add(weight)
            .ok_or(ContractError::Overflow)?;
    }
    proposal.total_voted_weight = proposal
        .total_voted_weight
        .checked_add(weight)
        .ok_or(ContractError::Overflow)?;

    env.storage().instance().set(&proposal_key, &proposal);
    env.storage()
        .instance()
        .set(&vote_key, &MergeVoteRecord { approve, weight });

    env.events().publish(
        (
            MERGE_VOTE_EVENT,
            source_key.clone(),
            source_key_holder.clone(),
        ),
        (approve, weight),
    );

    Ok(())
}

/// Convert a source balance at the proposal ratio, rounding down.
fn convert_at_ratio(amount: u32, ratio_bps: u32) -> Result<u32, ContractError> {
    let scaled = (amount as u64)
        .checked_mul(ratio_bps as u64)
        .ok_or(ContractError::Overflow)?;
    u32::try_from(scaled / 10_000).map_err(|_| ContractError::Overflow)
}

/// #962: Execute an approved key merge and migrate holder balance ratio.
///
/// The merge only runs once the weighted approval share reaches the proposal's
/// `approval_threshold_bps`. Every registered source holder's liquid and staked
/// positions are then moved to the target key at `conversion_ratio_bps`, the
/// source aggregate stake total is transferred, and the source key is
/// deprecated in the same call so no window exists where it is neither live
/// nor deprecated.
pub fn execute_key_merge(
    env: &Env,
    creator: &Address,
    admin: &Address,
    source_key: &Address,
    target_key: &Address,
) -> Result<u32, ContractError> {
    creator.require_auth();
    admin.require_auth();

    let proposal_key = EmwulrdDataKey::MergeProposal(source_key.clone());
    let mut proposal: KeyMergeProposal = env
        .storage()
        .instance()
        .get(&proposal_key)
        .ok_or(ContractError::ProposalNotFound)?;

    if proposal.is_executed || proposal.target_key != *target_key {
        return Err(ContractError::AlreadyRegistered);
    }

    // Validate the vote threshold before touching any holder position. A merge
    // with no cast weight can never satisfy a positive threshold.
    let approval_share_bps = if proposal.total_voted_weight == 0 {
        0u64
    } else {
        (proposal.votes_for as u64) * 10_000 / (proposal.total_voted_weight as u64)
    };
    if approval_share_bps < proposal.approval_threshold_bps as u64 {
        return Err(ContractError::Unauthorized);
    }

    let holders: Vec<Address> = env
        .storage()
        .instance()
        .get(&EmwulrdDataKey::MergeHolders(source_key.clone()))
        .ok_or(ContractError::NoKeyHolders)?;
    if holders.is_empty() {
        return Err(ContractError::NoKeyHolders);
    }

    let ratio = proposal.conversion_ratio_bps;
    let mut total_liquid_migrated: u32 = 0;
    let mut total_staked_migrated: u32 = 0;
    let mut log: Vec<MergeMigrationEntry> = Vec::new(env);

    for holder in holders.iter() {
        let source_liquid_key = crate::constants::storage::holder_balance_key(source_key, &holder);
        let source_staked_key = crate::constants::storage::staked_balance(source_key, &holder);

        let liquid: u32 = env
            .storage()
            .persistent()
            .get(&source_liquid_key)
            .unwrap_or(0);
        let staked: u32 = env
            .storage()
            .persistent()
            .get(&source_staked_key)
            .unwrap_or(0);

        if liquid == 0 && staked == 0 {
            continue;
        }

        let liquid_migrated = convert_at_ratio(liquid, ratio)?;
        let staked_migrated = convert_at_ratio(staked, ratio)?;

        // Drain the source position even when rounding leaves zero: the merge is
        // a move, and dust must not keep the deprecated key alive.
        if liquid > 0 {
            if liquid_migrated > 0 {
                let target_balance_key =
                    crate::constants::storage::holder_balance_key(target_key, &holder);
                let existing: u32 = env
                    .storage()
                    .persistent()
                    .get(&target_balance_key)
                    .unwrap_or(0);
                let updated = existing
                    .checked_add(liquid_migrated)
                    .ok_or(ContractError::Overflow)?;
                env.storage()
                    .persistent()
                    .set(&target_balance_key, &updated);
            }
            env.storage().persistent().remove(&source_liquid_key);
        }

        if staked > 0 {
            if staked_migrated > 0 {
                let target_staked_key =
                    crate::constants::storage::staked_balance(target_key, &holder);
                let existing: u32 = env
                    .storage()
                    .persistent()
                    .get(&target_staked_key)
                    .unwrap_or(0);
                let updated = existing
                    .checked_add(staked_migrated)
                    .ok_or(ContractError::Overflow)?;
                env.storage().persistent().set(&target_staked_key, &updated);
            }
            env.storage().persistent().remove(&source_staked_key);
            total_staked_migrated = total_staked_migrated
                .checked_add(staked_migrated)
                .ok_or(ContractError::Overflow)?;
        }

        total_liquid_migrated = total_liquid_migrated
            .checked_add(liquid_migrated)
            .ok_or(ContractError::Overflow)?;

        log.push_back(MergeMigrationEntry {
            holder,
            liquid_migrated,
            staked_migrated,
        });
    }

    // Move the aggregate staked total so the source key reports no stake after
    // deprecation and the target key's total reflects the migrated positions.
    if total_staked_migrated > 0 {
        let source_total_key = crate::constants::storage::total_staked(source_key);
        let target_total_key = crate::constants::storage::total_staked(target_key);
        let source_total = crate::read_total_staked(env, source_key);
        let target_total = crate::read_total_staked(env, target_key);

        let remaining = source_total.saturating_sub(total_staked_migrated);
        if remaining == 0 {
            env.storage().persistent().remove(&source_total_key);
        } else {
            env.storage()
                .persistent()
                .set(&source_total_key, &remaining);
        }

        let new_target_total = target_total
            .checked_add(total_staked_migrated)
            .ok_or(ContractError::Overflow)?;
        env.storage()
            .persistent()
            .set(&target_total_key, &new_target_total);
    }

    // Atomically deprecate the source key: the protocol-wide marker rejects new
    // buys, and the module's sunset status records the deprecation.
    env.storage().persistent().set(
        &crate::constants::storage::deprecated_key(source_key),
        &0i128,
    );
    env.storage().instance().set(
        &EmwulrdDataKey::SunsetStatus(source_key.clone()),
        &KeySunsetStatus {
            last_trade_ledger: env.ledger().sequence(),
            is_sunset_pending: false,
            is_deprecated: true,
        },
    );

    env.storage()
        .instance()
        .set(&EmwulrdDataKey::MergeMigrationLog(source_key.clone()), &log);

    proposal.is_approved_by_admin = true;
    proposal.is_executed = true;
    env.storage().instance().set(&proposal_key, &proposal);

    env.events().publish(
        (MERGE_EXECUTED_EVENT, source_key.clone(), target_key.clone()),
        (
            proposal.conversion_ratio_bps,
            log.len(),
            total_liquid_migrated,
            total_staked_migrated,
        ),
    );
    env.events().publish(
        (KEY_DEPRECATED_EVENT, source_key.clone()),
        env.ledger().sequence(),
    );

    Ok(proposal.conversion_ratio_bps)
}

/// #931: Update last trade ledger activity.
pub fn record_trade_ledger_activity(env: &Env, creator: &Address) {
    let key = EmwulrdDataKey::SunsetStatus(creator.clone());
    let status = KeySunsetStatus {
        last_trade_ledger: env.ledger().sequence(),
        is_sunset_pending: false,
        is_deprecated: false,
    };
    env.storage().instance().set(&key, &status);
}

/// Flag key for sunset if inactivity ledger threshold exceeded.
pub fn flag_inactive_key_sunset(
    env: &Env,
    creator: &Address,
    inactivity_threshold_ledgers: u32,
) -> Result<bool, ContractError> {
    let key = EmwulrdDataKey::SunsetStatus(creator.clone());
    let mut status: KeySunsetStatus =
        env.storage()
            .instance()
            .get(&key)
            .unwrap_or(KeySunsetStatus {
                last_trade_ledger: env.ledger().sequence(),
                is_sunset_pending: false,
                is_deprecated: false,
            });

    let current = env.ledger().sequence();
    if current.saturating_sub(status.last_trade_ledger) >= inactivity_threshold_ledgers {
        status.is_sunset_pending = true;
        env.storage().instance().set(&key, &status);

        env.events().publish(
            (KEY_SUNSET_FLAGGED_EVENT, creator.clone()),
            status.last_trade_ledger,
        );

        return Ok(true);
    }

    Ok(false)
}

/// Finalize sunset deprecation with admin authorization.
pub fn finalize_key_deprecation(
    env: &Env,
    admin: &Address,
    creator: &Address,
) -> Result<(), ContractError> {
    admin.require_auth();

    let key = EmwulrdDataKey::SunsetStatus(creator.clone());
    let mut status: KeySunsetStatus = env
        .storage()
        .instance()
        .get(&key)
        .ok_or(ContractError::Unauthorized)?;

    if !status.is_sunset_pending {
        return Err(ContractError::Unauthorized);
    }

    status.is_deprecated = true;
    status.is_sunset_pending = false;

    env.storage().instance().set(&key, &status);

    env.events().publish(
        (KEY_DEPRECATED_EVENT, creator.clone()),
        env.ledger().sequence(),
    );

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup_contract(env: &Env) -> Address {
        env.register(crate::CreatorKeysContract, ())
    }

    fn set_position(
        env: &Env,
        contract_id: &Address,
        creator: &Address,
        holder: &Address,
        liquid: u32,
        staked: u32,
    ) {
        env.as_contract(contract_id, || {
            if liquid > 0 {
                env.storage().persistent().set(
                    &crate::constants::storage::holder_balance_key(creator, holder),
                    &liquid,
                );
            }
            if staked > 0 {
                env.storage().persistent().set(
                    &crate::constants::storage::staked_balance(creator, holder),
                    &staked,
                );
                // Mirror the staking entrypoint: keep the aggregate in step.
                let total_key = crate::constants::storage::total_staked(creator);
                let current: u32 = env.storage().persistent().get(&total_key).unwrap_or(0);
                env.storage()
                    .persistent()
                    .set(&total_key, &(current + staked));
            }
        });
    }

    fn read_liquid(env: &Env, contract_id: &Address, creator: &Address, holder: &Address) -> u32 {
        env.as_contract(contract_id, || {
            env.storage()
                .persistent()
                .get(&crate::constants::storage::holder_balance_key(
                    creator, holder,
                ))
                .unwrap_or(0)
        })
    }

    fn read_staked(env: &Env, contract_id: &Address, creator: &Address, holder: &Address) -> u32 {
        env.as_contract(contract_id, || {
            env.storage()
                .persistent()
                .get(&crate::constants::storage::staked_balance(creator, holder))
                .unwrap_or(0)
        })
    }

    /// The proposal under test: which keys are being merged, the quorum the
    /// contract will enforce, and the holders whose votes the test casts.
    ///
    /// Grouping these keeps `propose` within clippy's argument budget while
    /// still mirroring the entrypoint's own seven parameters.
    struct MergeProposalSpec<'a> {
        source_key: &'a Address,
        target_key: &'a Address,
        ratio: u32,
        threshold: u32,
        holders: Vec<Address>,
    }

    // Each entrypoint is invoked in its own contract frame: in production the
    // proposal, the votes and the execution are separate transactions, and the
    // mock auth recorder rejects a repeated `require_auth` inside one frame.
    fn propose(
        env: &Env,
        contract_id: &Address,
        creator: &Address,
        spec: MergeProposalSpec<'_>,
    ) -> Result<(), ContractError> {
        env.as_contract(contract_id, || {
            propose_key_merge(
                env,
                creator,
                spec.source_key,
                spec.target_key,
                spec.ratio,
                spec.threshold,
                spec.holders,
            )
        })
    }

    fn vote(
        env: &Env,
        contract_id: &Address,
        holder: &Address,
        source_key: &Address,
        approve: bool,
    ) -> Result<(), ContractError> {
        env.as_contract(contract_id, || {
            vote_on_merge(env, holder, source_key, approve)
        })
    }

    fn execute(
        env: &Env,
        contract_id: &Address,
        creator: &Address,
        admin: &Address,
        source_key: &Address,
        target_key: &Address,
    ) -> Result<u32, ContractError> {
        env.as_contract(contract_id, || {
            execute_key_merge(env, creator, admin, source_key, target_key)
        })
    }

    fn read_proposal(env: &Env, contract_id: &Address, source_key: &Address) -> KeyMergeProposal {
        env.as_contract(contract_id, || {
            get_key_merge_proposal(env, source_key).unwrap()
        })
    }

    #[test]
    fn merge_votes_use_liquid_source_balance_as_weight() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_contract(&env);

        let creator = Address::generate(&env);
        let source_key = Address::generate(&env);
        let target_key = Address::generate(&env);
        let holder_a = Address::generate(&env);
        let holder_b = Address::generate(&env);

        set_position(&env, &contract_id, &source_key, &holder_a, 600, 0);
        set_position(&env, &contract_id, &source_key, &holder_b, 400, 0);

        let mut holders = Vec::new(&env);
        holders.push_back(holder_a.clone());
        holders.push_back(holder_b.clone());

        assert!(propose(
            &env,
            &contract_id,
            &creator,
            MergeProposalSpec {
                source_key: &source_key,
                target_key: &target_key,
                ratio: 10_000,
                threshold: 5_000,
                holders,
            },
        )
        .is_ok());

        assert!(vote(&env, &contract_id, &holder_a, &source_key, true).is_ok());
        assert!(vote(&env, &contract_id, &holder_b, &source_key, false).is_ok());

        let proposal = read_proposal(&env, &contract_id, &source_key);
        assert_eq!(proposal.votes_for, 600);
        assert_eq!(proposal.votes_against, 400);
        assert_eq!(proposal.total_voted_weight, 1_000);

        // A wallet with no liquid source-key balance cannot vote.
        let non_holder = Address::generate(&env);
        let rejected = vote(&env, &contract_id, &non_holder, &source_key, true);
        assert_eq!(rejected, Err(ContractError::InsufficientBalance));
    }

    #[test]
    fn changing_vote_replaces_weight_without_double_counting() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_contract(&env);

        let creator = Address::generate(&env);
        let source_key = Address::generate(&env);
        let target_key = Address::generate(&env);
        let holder_a = Address::generate(&env);
        let holder_b = Address::generate(&env);

        set_position(&env, &contract_id, &source_key, &holder_a, 600, 0);
        set_position(&env, &contract_id, &source_key, &holder_b, 400, 0);

        let mut holders = Vec::new(&env);
        holders.push_back(holder_a.clone());
        holders.push_back(holder_b.clone());
        assert!(propose(
            &env,
            &contract_id,
            &creator,
            MergeProposalSpec {
                source_key: &source_key,
                target_key: &target_key,
                ratio: 10_000,
                threshold: 5_000,
                holders,
            },
        )
        .is_ok());

        assert!(vote(&env, &contract_id, &holder_a, &source_key, true).is_ok());
        assert!(vote(&env, &contract_id, &holder_b, &source_key, false).is_ok());
        // holder_b switches to approve: its 400 moves from against to for.
        assert!(vote(&env, &contract_id, &holder_b, &source_key, true).is_ok());

        let proposal = read_proposal(&env, &contract_id, &source_key);
        assert_eq!(proposal.votes_for, 1_000);
        assert_eq!(proposal.votes_against, 0);
        assert_eq!(proposal.total_voted_weight, 1_000);
    }

    #[test]
    fn execute_rejects_below_approval_threshold() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_contract(&env);

        let creator = Address::generate(&env);
        let admin = Address::generate(&env);
        let source_key = Address::generate(&env);
        let target_key = Address::generate(&env);
        let holder_a = Address::generate(&env);
        let holder_b = Address::generate(&env);

        set_position(&env, &contract_id, &source_key, &holder_a, 900, 0);
        set_position(&env, &contract_id, &source_key, &holder_b, 600, 0);

        let mut holders = Vec::new(&env);
        holders.push_back(holder_a.clone());
        holders.push_back(holder_b.clone());
        // 70% approval required; only 900 of 1500 cast weight approves.
        assert!(propose(
            &env,
            &contract_id,
            &creator,
            MergeProposalSpec {
                source_key: &source_key,
                target_key: &target_key,
                ratio: 10_000,
                threshold: 7_000,
                holders,
            },
        )
        .is_ok());

        assert!(vote(&env, &contract_id, &holder_a, &source_key, true).is_ok());
        assert!(vote(&env, &contract_id, &holder_b, &source_key, false).is_ok());

        let result = execute(
            &env,
            &contract_id,
            &creator,
            &admin,
            &source_key,
            &target_key,
        );
        assert_eq!(result, Err(ContractError::Unauthorized));

        // No position moved and the source key is not deprecated.
        assert_eq!(read_liquid(&env, &contract_id, &source_key, &holder_a), 900);
        assert_eq!(read_liquid(&env, &contract_id, &source_key, &holder_b), 600);
        let deprecated = env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .has(&crate::constants::storage::deprecated_key(&source_key))
        });
        assert!(!deprecated);

        // A proposal with zero cast weight is also rejected.
        let no_votes = execute(
            &env,
            &contract_id,
            &creator,
            &admin,
            &source_key,
            &target_key,
        );
        assert_eq!(no_votes, Err(ContractError::Unauthorized));
    }

    #[test]
    fn execute_migrates_all_positions_and_deprecates_source() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_contract(&env);

        let creator = Address::generate(&env);
        let admin = Address::generate(&env);
        let source_key = Address::generate(&env);
        let target_key = Address::generate(&env);
        let holder_a = Address::generate(&env);
        let holder_b = Address::generate(&env);

        // holder_a: 600 liquid + 200 staked on the source key.
        // holder_b: 400 liquid on the source key.
        set_position(&env, &contract_id, &source_key, &holder_a, 600, 200);
        set_position(&env, &contract_id, &source_key, &holder_b, 400, 0);
        // holder_a already has a target position that must receive the migrated keys.
        set_position(&env, &contract_id, &target_key, &holder_a, 50, 10);

        let ratio = {
            let mut holders = Vec::new(&env);
            holders.push_back(holder_a.clone());
            holders.push_back(holder_b.clone());
            assert!(propose(
                &env,
                &contract_id,
                &creator,
                MergeProposalSpec {
                    source_key: &source_key,
                    target_key: &target_key,
                    ratio: 5_000,
                    threshold: 5_000,
                    holders,
                },
            )
            .is_ok());

            assert!(vote(&env, &contract_id, &holder_a, &source_key, true).is_ok());
            assert!(vote(&env, &contract_id, &holder_b, &source_key, true).is_ok());

            execute(
                &env,
                &contract_id,
                &creator,
                &admin,
                &source_key,
                &target_key,
            )
            .unwrap()
        };
        assert_eq!(ratio, 5_000);

        // 600 * 0.5 = 300 liquid and 200 * 0.5 = 100 staked added to holder_a.
        assert_eq!(read_liquid(&env, &contract_id, &target_key, &holder_a), 350);
        assert_eq!(read_staked(&env, &contract_id, &target_key, &holder_a), 110);
        // 400 * 0.5 = 200 liquid for holder_b.
        assert_eq!(read_liquid(&env, &contract_id, &target_key, &holder_b), 200);

        // Source positions are fully drained.
        assert_eq!(read_liquid(&env, &contract_id, &source_key, &holder_a), 0);
        assert_eq!(read_staked(&env, &contract_id, &source_key, &holder_a), 0);
        assert_eq!(read_liquid(&env, &contract_id, &source_key, &holder_b), 0);

        let (log_len, staked_total, is_deprecated, is_executed) =
            env.as_contract(&contract_id, || {
                let log = get_merge_migration_log(&env, &source_key);
                let staked_total = crate::read_total_staked(&env, &target_key);
                let status: KeySunsetStatus = env
                    .storage()
                    .instance()
                    .get(&EmwulrdDataKey::SunsetStatus(source_key.clone()))
                    .unwrap();
                let proposal = get_key_merge_proposal(&env, &source_key).unwrap();
                (
                    log.len(),
                    staked_total,
                    status.is_deprecated,
                    proposal.is_executed,
                )
            });
        assert_eq!(log_len, 2);
        assert_eq!(staked_total, 110);
        assert!(is_deprecated);
        assert!(is_executed);

        // Protocol-wide deprecation marker is set with a zero buyback price.
        let marker = env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .get::<_, i128>(&crate::constants::storage::deprecated_key(&source_key))
        });
        assert_eq!(marker, Some(0));

        // The merge cannot be executed twice.
        let second = execute(
            &env,
            &contract_id,
            &creator,
            &admin,
            &source_key,
            &target_key,
        );
        assert_eq!(second, Err(ContractError::AlreadyRegistered));
    }

    #[test]
    fn propose_rejects_duplicate_holders_and_bad_parameters() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_contract(&env);

        let creator = Address::generate(&env);
        let source_key = Address::generate(&env);
        let target_key = Address::generate(&env);
        let holder = Address::generate(&env);

        let duplicate = env.as_contract(&contract_id, || {
            let mut holders = Vec::new(&env);
            holders.push_back(holder.clone());
            holders.push_back(holder.clone());
            propose_key_merge(
                &env,
                &creator,
                &source_key,
                &target_key,
                10_000,
                5_000,
                holders,
            )
        });
        assert_eq!(duplicate, Err(ContractError::AlreadyRegistered));

        let empty = env.as_contract(&contract_id, || {
            propose_key_merge(
                &env,
                &creator,
                &source_key,
                &target_key,
                10_000,
                5_000,
                Vec::new(&env),
            )
        });
        assert_eq!(empty, Err(ContractError::NoKeyHolders));

        let bad_ratio = env.as_contract(&contract_id, || {
            let mut holders = Vec::new(&env);
            holders.push_back(holder.clone());
            propose_key_merge(&env, &creator, &source_key, &target_key, 0, 5_000, holders)
        });
        assert_eq!(bad_ratio, Err(ContractError::InvalidFeeConfig));
    }
}
