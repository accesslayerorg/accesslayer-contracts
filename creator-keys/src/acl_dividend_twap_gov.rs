use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

use crate::ContractError;

// ═══════════════════════════════════════════════════════════════════════════════
// 1. ACL WHITELIST CONTRACT FOR EXTERNAL INTEGRATION GATING (#972)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AclEntry {
    pub contract_address: Address,
    pub allowed_functions: Vec<Symbol>,
    pub is_active: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum AclDataKey {
    Admin,
    Acl(Address),
    AclAddressList,
}

pub const ACL_UPDATED_EVENT: Symbol = symbol_short!("acl_upd");

/// Add or update an external contract in the ACL whitelist with permitted function selectors.
pub fn add_to_acl(
    env: &Env,
    admin: &Address,
    contract_addr: &Address,
    allowed_functions: Vec<Symbol>,
) -> Result<(), ContractError> {
    admin.require_auth();

    let entry = AclEntry {
        contract_address: contract_addr.clone(),
        allowed_functions,
        is_active: true,
    };

    let key = AclDataKey::Acl(contract_addr.clone());
    let list_key = AclDataKey::AclAddressList;

    let mut address_list: Vec<Address> = env
        .storage()
        .instance()
        .get(&list_key)
        .unwrap_or(Vec::new(env));

    let mut exists = false;
    for addr in address_list.iter() {
        if addr == *contract_addr {
            exists = true;
            break;
        }
    }
    if !exists {
        address_list.push_back(contract_addr.clone());
        env.storage().instance().set(&list_key, &address_list);
    }

    env.storage().instance().set(&key, &entry);

    env.events().publish(
        (ACL_UPDATED_EVENT, contract_addr.clone()),
        (symbol_short!("add"), true),
    );

    Ok(())
}

/// Remove external contract from ACL whitelist, clearing all function permissions.
pub fn remove_from_acl(
    env: &Env,
    admin: &Address,
    contract_addr: &Address,
) -> Result<(), ContractError> {
    admin.require_auth();

    let key = AclDataKey::Acl(contract_addr.clone());
    let empty_functions = Vec::new(env);
    let entry = AclEntry {
        contract_address: contract_addr.clone(),
        allowed_functions: empty_functions,
        is_active: false,
    };

    env.storage().instance().set(&key, &entry);

    env.events().publish(
        (ACL_UPDATED_EVENT, contract_addr.clone()),
        (symbol_short!("remove"), false),
    );

    Ok(())
}

/// Check whether a contract address and function selector is permitted by the ACL.
/// Handles both exact selector matches and wildcard entries (`wildcard` or `all`).
pub fn is_permitted(env: &Env, contract_addr: &Address, function_selector: &Symbol) -> bool {
    let key = AclDataKey::Acl(contract_addr.clone());
    let entry_opt: Option<AclEntry> = env.storage().instance().get(&key);

    if let Some(entry) = entry_opt {
        if !entry.is_active {
            return false;
        }

        let wildcard_all = symbol_short!("all");
        let wildcard_star = symbol_short!("wildcard");

        for func in entry.allowed_functions.iter() {
            if func == *function_selector || func == wildcard_all || func == wildcard_star {
                return true;
            }
        }
    }

    false
}

/// Returns full whitelist with function sets.
pub fn get_acl(env: &Env) -> Vec<AclEntry> {
    let list_key = AclDataKey::AclAddressList;
    let address_list: Vec<Address> = env
        .storage()
        .instance()
        .get(&list_key)
        .unwrap_or(Vec::new(env));

    let mut result = Vec::new(env);
    for addr in address_list.iter() {
        let key = AclDataKey::Acl(addr.clone());
        if let Some(entry) = env.storage().instance().get::<_, AclEntry>(&key) {
            if entry.is_active {
                result.push_back(entry);
            }
        }
    }

    result
}

pub mod acl_contract {
    use super::*;

    #[contract]
    pub struct AclWhitelistContract;

    #[contractimpl]
    impl AclWhitelistContract {
        pub fn init(env: Env, admin: Address) {
            admin.require_auth();
            env.storage().instance().set(&AclDataKey::Admin, &admin);
        }

        pub fn get_admin(env: Env) -> Option<Address> {
            env.storage().instance().get(&AclDataKey::Admin)
        }

        pub fn add_to_acl(
            env: Env,
            caller: Address,
            contract_address: Address,
            allowed_functions: Vec<Symbol>,
        ) -> Result<(), ContractError> {
            let admin: Address = env
                .storage()
                .instance()
                .get(&AclDataKey::Admin)
                .unwrap_or(caller.clone());
            if caller != admin {
                return Err(ContractError::Unauthorized);
            }
            add_to_acl(&env, &admin, &contract_address, allowed_functions)
        }

        pub fn remove_from_acl(
            env: Env,
            caller: Address,
            contract_address: Address,
        ) -> Result<(), ContractError> {
            let admin: Address = env
                .storage()
                .instance()
                .get(&AclDataKey::Admin)
                .unwrap_or(caller.clone());
            if caller != admin {
                return Err(ContractError::Unauthorized);
            }
            remove_from_acl(&env, &admin, &contract_address)
        }

        pub fn is_permitted(
            env: Env,
            contract_address: Address,
            function_selector: Symbol,
        ) -> bool {
            is_permitted(&env, &contract_address, &function_selector)
        }

        pub fn get_acl(env: Env) -> Vec<AclEntry> {
            get_acl(&env)
        }
    }
}
pub use acl_contract::{AclWhitelistContract, AclWhitelistContractClient};

// ═══════════════════════════════════════════════════════════════════════════════
// 2. DIVIDEND POOL CONTRACT WITH PRO-RATA DISTRIBUTION (#971)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendSnapshot {
    pub epoch: u32,
    pub total_amount: i128,
    pub total_supply_snapshot: i128,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct HolderSnapshotRecord {
    pub holder: Address,
    pub balance: i128,
}

#[derive(Clone)]
#[contracttype]
pub enum DividendDataKey {
    Admin,
    CurrentEpoch(Address),                 // key_id -> epoch
    EpochDistribution(Address, u32),       // (key_id, epoch) -> DividendSnapshot
    HolderSnapshot(Address, u32, Address), // (key_id, epoch, holder) -> balance
    EpochClaimed(Address, u32, Address),   // (key_id, epoch, holder) -> bool
    DistributionHistory(Address),          // key_id -> Vec<DividendSnapshot>
}

pub const DIVIDEND_DEPOSITED_EVENT: Symbol = symbol_short!("div_dep");
pub const DIVIDEND_CLAIMED_EVENT: Symbol = symbol_short!("div_clm");

/// Snapshot holder balances at distribution time for a key.
pub fn snapshot_holdings(
    env: &Env,
    caller: &Address,
    key_id: &Address,
    total_supply: i128,
    holder_records: Vec<HolderSnapshotRecord>,
) -> Result<u32, ContractError> {
    caller.require_auth();
    if total_supply <= 0 {
        return Err(ContractError::InsufficientSupply);
    }

    let epoch_key = DividendDataKey::CurrentEpoch(key_id.clone());
    let current_epoch: u32 = env.storage().instance().get(&epoch_key).unwrap_or(0);
    let next_epoch = current_epoch + 1;

    for record in holder_records.iter() {
        let holder_key =
            DividendDataKey::HolderSnapshot(key_id.clone(), next_epoch, record.holder.clone());
        env.storage().instance().set(&holder_key, &record.balance);
    }

    let snapshot = DividendSnapshot {
        epoch: next_epoch,
        total_amount: 0,
        total_supply_snapshot: total_supply,
        timestamp: env.ledger().timestamp(),
    };

    env.storage().instance().set(
        &DividendDataKey::EpochDistribution(key_id.clone(), next_epoch),
        &snapshot,
    );
    env.storage().instance().set(&epoch_key, &next_epoch);

    Ok(next_epoch)
}

/// Creator deposits distribution amount into dividend pool for current epoch.
pub fn deposit_dividends(
    env: &Env,
    creator: &Address,
    key_id: &Address,
    amount: i128,
) -> Result<u32, ContractError> {
    creator.require_auth();
    if amount <= 0 {
        return Err(ContractError::ZeroDistributionAmount);
    }

    let epoch_key = DividendDataKey::CurrentEpoch(key_id.clone());
    let mut epoch: u32 = env.storage().instance().get(&epoch_key).unwrap_or(0);

    if epoch == 0 {
        epoch = 1;
        env.storage().instance().set(&epoch_key, &epoch);
    }

    let dist_key = DividendDataKey::EpochDistribution(key_id.clone(), epoch);
    let mut snapshot: DividendSnapshot =
        env.storage()
            .instance()
            .get(&dist_key)
            .unwrap_or(DividendSnapshot {
                epoch,
                total_amount: 0,
                total_supply_snapshot: 0,
                timestamp: env.ledger().timestamp(),
            });

    snapshot.total_amount += amount;
    snapshot.timestamp = env.ledger().timestamp();
    env.storage().instance().set(&dist_key, &snapshot);

    // Update distribution history
    let hist_key = DividendDataKey::DistributionHistory(key_id.clone());
    let mut history: Vec<DividendSnapshot> = env
        .storage()
        .instance()
        .get(&hist_key)
        .unwrap_or(Vec::new(env));
    history.push_back(snapshot.clone());
    env.storage().instance().set(&hist_key, &history);

    env.events()
        .publish((DIVIDEND_DEPOSITED_EVENT, key_id.clone()), (epoch, amount));

    Ok(epoch)
}

/// View pending unclaimed dividend amount for a holder across all epochs.
pub fn get_pending_dividends(env: &Env, key_id: &Address, holder: &Address) -> i128 {
    let epoch_key = DividendDataKey::CurrentEpoch(key_id.clone());
    let current_epoch: u32 = env.storage().instance().get(&epoch_key).unwrap_or(0);

    if current_epoch == 0 {
        return 0;
    }

    let mut pending_total: i128 = 0;

    for epoch in 1..=current_epoch {
        let claimed_key = DividendDataKey::EpochClaimed(key_id.clone(), epoch, holder.clone());
        let is_claimed: bool = env.storage().instance().get(&claimed_key).unwrap_or(false);
        if is_claimed {
            continue;
        }

        let dist_key = DividendDataKey::EpochDistribution(key_id.clone(), epoch);
        if let Some(snapshot) = env
            .storage()
            .instance()
            .get::<_, DividendSnapshot>(&dist_key)
        {
            if snapshot.total_amount <= 0 || snapshot.total_supply_snapshot <= 0 {
                continue;
            }

            let holder_key = DividendDataKey::HolderSnapshot(key_id.clone(), epoch, holder.clone());
            let holder_balance: i128 = env.storage().instance().get(&holder_key).unwrap_or(0);

            if holder_balance > 0 {
                let share =
                    (snapshot.total_amount * holder_balance) / snapshot.total_supply_snapshot;
                pending_total += share;
            }
        }
    }

    pending_total
}

/// Holder claims pro-rata dividend share across eligible epochs.
pub fn claim_dividends(
    env: &Env,
    key_id: &Address,
    holder: &Address,
) -> Result<i128, ContractError> {
    holder.require_auth();

    let epoch_key = DividendDataKey::CurrentEpoch(key_id.clone());
    let current_epoch: u32 = env.storage().instance().get(&epoch_key).unwrap_or(0);

    if current_epoch == 0 {
        return Err(ContractError::NoDividendClaimable);
    }

    let mut total_claimed: i128 = 0;

    for epoch in 1..=current_epoch {
        let claimed_key = DividendDataKey::EpochClaimed(key_id.clone(), epoch, holder.clone());
        let is_claimed: bool = env.storage().instance().get(&claimed_key).unwrap_or(false);
        if is_claimed {
            continue;
        }

        let dist_key = DividendDataKey::EpochDistribution(key_id.clone(), epoch);
        if let Some(snapshot) = env
            .storage()
            .instance()
            .get::<_, DividendSnapshot>(&dist_key)
        {
            if snapshot.total_amount <= 0 || snapshot.total_supply_snapshot <= 0 {
                continue;
            }

            let holder_key = DividendDataKey::HolderSnapshot(key_id.clone(), epoch, holder.clone());
            let holder_balance: i128 = env.storage().instance().get(&holder_key).unwrap_or(0);

            if holder_balance > 0 {
                let share =
                    (snapshot.total_amount * holder_balance) / snapshot.total_supply_snapshot;
                if share > 0 {
                    total_claimed += share;
                    env.storage().instance().set(&claimed_key, &true);

                    env.events().publish(
                        (DIVIDEND_CLAIMED_EVENT, key_id.clone(), holder.clone()),
                        (epoch, share),
                    );
                }
            }
        }
    }

    if total_claimed == 0 {
        return Err(ContractError::NoDividendClaimable);
    }

    Ok(total_claimed)
}

/// Retrieve distribution history for a key.
pub fn get_distribution_history(env: &Env, key_id: &Address) -> Vec<DividendSnapshot> {
    let hist_key = DividendDataKey::DistributionHistory(key_id.clone());
    env.storage()
        .instance()
        .get(&hist_key)
        .unwrap_or(Vec::new(env))
}

pub mod dividend_contract {
    use super::*;

    #[contract]
    pub struct DividendPoolContract;

    #[contractimpl]
    impl DividendPoolContract {
        pub fn init(env: Env, admin: Address) {
            admin.require_auth();
            env.storage()
                .instance()
                .set(&DividendDataKey::Admin, &admin);
        }

        pub fn snapshot_holdings(
            env: Env,
            caller: Address,
            key_id: Address,
            total_supply: i128,
            holder_records: Vec<HolderSnapshotRecord>,
        ) -> Result<u32, ContractError> {
            snapshot_holdings(&env, &caller, &key_id, total_supply, holder_records)
        }

        pub fn deposit_dividends(
            env: Env,
            creator: Address,
            key_id: Address,
            amount: i128,
        ) -> Result<u32, ContractError> {
            deposit_dividends(&env, &creator, &key_id, amount)
        }

        pub fn claim_dividends(
            env: Env,
            key_id: Address,
            holder: Address,
        ) -> Result<i128, ContractError> {
            claim_dividends(&env, &key_id, &holder)
        }

        pub fn get_pending_dividends(env: Env, key_id: Address, holder: Address) -> i128 {
            get_pending_dividends(&env, &key_id, &holder)
        }

        pub fn get_distribution_history(env: Env, key_id: Address) -> Vec<DividendSnapshot> {
            get_distribution_history(&env, &key_id)
        }
    }
}
pub use dividend_contract::{DividendPoolContract, DividendPoolContractClient};

// ═══════════════════════════════════════════════════════════════════════════════
// 3. TWAP ORACLE STORAGE & UPDATE MECHANISM FOR BONDING CURVE (#968)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TwapObservation {
    pub timestamp: u64,
    pub price: i128,
    pub cumulative_price_time: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TwapConfig {
    pub min_observations: u32,
    pub max_window_seconds: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum TwapDataKey {
    TwapObservations(Address), // key_id -> Vec<TwapObservation>
    TwapConfig(Address),       // key_id -> TwapConfig
}

pub const TWAP_UPDATED_EVENT: Symbol = symbol_short!("twap_upd");

/// Record price observation on bonding curve trade and accumulate price * time product.
/// Unaffected by wash trades within the same block / timestamp.
pub fn record_twap_trade(
    env: &Env,
    key_id: &Address,
    current_price: i128,
) -> Result<i128, ContractError> {
    if current_price < 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    let key = TwapDataKey::TwapObservations(key_id.clone());
    let mut observations: Vec<TwapObservation> =
        env.storage().instance().get(&key).unwrap_or(Vec::new(env));

    let now = env.ledger().timestamp();
    let config = get_twap_config(env, key_id);

    let cumulative_price_time: i128;

    if observations.is_empty() {
        cumulative_price_time = 0;
        let obs = TwapObservation {
            timestamp: now,
            price: current_price,
            cumulative_price_time: 0,
        };
        observations.push_back(obs);
    } else {
        let last_idx = observations.len() - 1;
        let last_obs = observations.get(last_idx).unwrap();

        if now == last_obs.timestamp {
            // Same-block update (wash trade protection): update price without time accumulation
            cumulative_price_time = last_obs.cumulative_price_time;
            let updated_obs = TwapObservation {
                timestamp: now,
                price: current_price,
                cumulative_price_time,
            };
            observations.set(last_idx, updated_obs);
        } else {
            let time_delta = (now.saturating_sub(last_obs.timestamp)) as i128;
            let price_time = last_obs.price.saturating_mul(time_delta);
            cumulative_price_time = last_obs.cumulative_price_time.saturating_add(price_time);

            let new_obs = TwapObservation {
                timestamp: now,
                price: current_price,
                cumulative_price_time,
            };
            observations.push_back(new_obs);

            // Storage pruning: prune observations older than max_window_seconds
            let cutoff = now.saturating_sub(config.max_window_seconds);
            let mut pruned = Vec::new(env);
            let mut keep_from_idx = 0;

            for (idx, obs) in observations.iter().enumerate() {
                if obs.timestamp >= cutoff {
                    if idx > 0 && keep_from_idx == 0 {
                        // Keep one boundary observation before the cutoff for window calculations
                        keep_from_idx = idx.saturating_sub(1);
                    }
                    break;
                }
            }

            for (idx, obs) in observations.iter().enumerate() {
                if idx >= keep_from_idx {
                    pruned.push_back(obs);
                }
            }

            if !pruned.is_empty() {
                observations = pruned;
            }
        }
    }

    env.storage().instance().set(&key, &observations);

    env.events().publish(
        (TWAP_UPDATED_EVENT, key_id.clone()),
        (current_price, cumulative_price_time),
    );

    Ok(cumulative_price_time)
}

/// Read TWAP for the requested window in seconds (e.g. 1h = 3600, 4h = 14400, 24h = 86400).
pub fn get_twap(env: &Env, key_id: &Address, window_seconds: u64) -> Result<i128, ContractError> {
    if window_seconds == 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    let key = TwapDataKey::TwapObservations(key_id.clone());
    let observations: Vec<TwapObservation> = env
        .storage()
        .instance()
        .get(&key)
        .ok_or(ContractError::OraclePriceNotSet)?;

    let config = get_twap_config(env, key_id);
    if observations.len() < config.min_observations {
        return Err(ContractError::OraclePriceNotSet);
    }

    let now = env.ledger().timestamp();
    let target_time = now.saturating_sub(window_seconds);

    let latest_obs = observations.get(observations.len() - 1).unwrap();

    // Determine latest cumulative at `now`
    let latest_time_delta = (now.saturating_sub(latest_obs.timestamp)) as i128;
    let latest_cumulative = latest_obs
        .cumulative_price_time
        .saturating_add(latest_obs.price.saturating_mul(latest_time_delta));

    // Determine cumulative at `target_time`
    let mut target_cumulative = observations.get(0).unwrap().cumulative_price_time;
    let mut found = false;

    for i in (0..observations.len()).rev() {
        let obs = observations.get(i).unwrap();
        if obs.timestamp <= target_time {
            let delta = (target_time.saturating_sub(obs.timestamp)) as i128;
            target_cumulative = obs
                .cumulative_price_time
                .saturating_add(obs.price.saturating_mul(delta));
            found = true;
            break;
        }
    }

    if !found {
        let first = observations.get(0).unwrap();
        target_cumulative = first.cumulative_price_time;
    }

    let total_time = (now.saturating_sub(target_time)) as i128;
    if total_time <= 0 {
        return Ok(latest_obs.price);
    }

    let twap = latest_cumulative
        .saturating_sub(target_cumulative)
        .checked_div(total_time)
        .unwrap_or(latest_obs.price);

    Ok(twap)
}

/// Read or default TWAP configuration.
pub fn get_twap_config(env: &Env, key_id: &Address) -> TwapConfig {
    let key = TwapDataKey::TwapConfig(key_id.clone());
    env.storage().instance().get(&key).unwrap_or(TwapConfig {
        min_observations: 2,
        max_window_seconds: 86400, // 24 hours
    })
}

/// Set TWAP configuration for a key.
pub fn set_twap_config(
    env: &Env,
    admin: &Address,
    key_id: &Address,
    min_observations: u32,
    max_window_seconds: u64,
) -> Result<(), ContractError> {
    admin.require_auth();
    if min_observations == 0 || max_window_seconds == 0 {
        return Err(ContractError::InvalidFeeConfig);
    }

    let config = TwapConfig {
        min_observations,
        max_window_seconds,
    };

    env.storage()
        .instance()
        .set(&TwapDataKey::TwapConfig(key_id.clone()), &config);

    Ok(())
}

pub mod twap_contract {
    use super::*;

    #[contract]
    pub struct TwapOracleContract;

    #[contractimpl]
    impl TwapOracleContract {
        pub fn record_trade(env: Env, key_id: Address, price: i128) -> Result<i128, ContractError> {
            record_twap_trade(&env, &key_id, price)
        }

        pub fn get_twap(
            env: Env,
            key_id: Address,
            window_seconds: u64,
        ) -> Result<i128, ContractError> {
            get_twap(&env, &key_id, window_seconds)
        }

        pub fn set_config(
            env: Env,
            admin: Address,
            key_id: Address,
            min_observations: u32,
            max_window_seconds: u64,
        ) -> Result<(), ContractError> {
            set_twap_config(&env, &admin, &key_id, min_observations, max_window_seconds)
        }

        pub fn get_observations(env: Env, key_id: Address) -> Vec<TwapObservation> {
            let key = TwapDataKey::TwapObservations(key_id);
            env.storage().instance().get(&key).unwrap_or(Vec::new(&env))
        }
    }
}
pub use twap_contract::{TwapOracleContract, TwapOracleContractClient};

// ═══════════════════════════════════════════════════════════════════════════════
// 4. GOVERNANCE PROPOSAL CONTRACT WITH TOKEN-WEIGHTED VOTING (#969)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum ProposalStatus {
    Pending = 1,
    Active = 2,
    Passed = 3,
    Failed = 4,
    Executed = 5,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CreateProposalParams {
    pub key_id: Address,
    pub discussion_link: String,
    pub proposer_balance: i128,
    pub min_holding_threshold: i128,
    pub voting_duration_ledgers: u32,
    pub min_quorum_weight: i128,
    pub pass_threshold_bps: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Proposal {
    pub id: u32,
    pub proposer: Address,
    pub key_id: Address,
    pub discussion_link: String,
    pub start_ledger: u32,
    pub end_ledger: u32,
    pub min_quorum_weight: i128,
    pub pass_threshold_bps: u32, // e.g. 5000 = 50%
    pub votes_for: i128,
    pub votes_against: i128,
    pub total_voted_weight: i128,
    pub status: ProposalStatus,
}

#[derive(Clone)]
#[contracttype]
pub enum GovDataKey {
    Admin,
    MinHoldingThreshold,
    ProposalCount,
    Proposal(u32),
    VoterRecord(u32, Address), // (proposal_id, voter) -> bool
}

pub const PROPOSAL_CREATED_EVENT: Symbol = symbol_short!("prop_new");
pub const PROPOSAL_VOTED_EVENT: Symbol = symbol_short!("prop_vote");
pub const PROPOSAL_EXECUTED_EVENT: Symbol = symbol_short!("prop_exec");

/// Submit governance proposal if proposer meets holding eligibility threshold.
pub fn create_proposal(
    env: &Env,
    proposer: &Address,
    params: &CreateProposalParams,
) -> Result<u32, ContractError> {
    proposer.require_auth();

    if params.proposer_balance < params.min_holding_threshold || params.min_holding_threshold < 0 {
        return Err(ContractError::InsufficientBalance);
    }
    if params.voting_duration_ledgers == 0 || params.pass_threshold_bps > 10_000 {
        return Err(ContractError::InvalidFeeConfig);
    }

    let count_key = GovDataKey::ProposalCount;
    let count: u32 = env.storage().instance().get(&count_key).unwrap_or(0);
    let proposal_id = count + 1;

    let start_ledger = env.ledger().sequence();
    let end_ledger = start_ledger + params.voting_duration_ledgers;

    let proposal = Proposal {
        id: proposal_id,
        proposer: proposer.clone(),
        key_id: params.key_id.clone(),
        discussion_link: params.discussion_link.clone(),
        start_ledger,
        end_ledger,
        min_quorum_weight: params.min_quorum_weight,
        pass_threshold_bps: params.pass_threshold_bps,
        votes_for: 0,
        votes_against: 0,
        total_voted_weight: 0,
        status: ProposalStatus::Active,
    };

    env.storage()
        .instance()
        .set(&GovDataKey::Proposal(proposal_id), &proposal);
    env.storage().instance().set(&count_key, &proposal_id);

    env.events().publish(
        (
            PROPOSAL_CREATED_EVENT,
            params.key_id.clone(),
            proposer.clone(),
        ),
        (proposal_id, end_ledger),
    );

    Ok(proposal_id)
}

/// Cast token-weighted vote on a proposal. Prevents double voting.
pub fn vote(
    env: &Env,
    voter: &Address,
    proposal_id: u32,
    approve: bool,
    voter_weight: i128,
) -> Result<(), ContractError> {
    voter.require_auth();

    if voter_weight <= 0 {
        return Err(ContractError::InsufficientBalance);
    }

    let prop_key = GovDataKey::Proposal(proposal_id);
    let mut proposal: Proposal = env
        .storage()
        .instance()
        .get(&prop_key)
        .ok_or(ContractError::ProposalNotFound)?;

    let current_ledger = env.ledger().sequence();
    if current_ledger < proposal.start_ledger || current_ledger > proposal.end_ledger {
        return Err(ContractError::DeadlinePassed);
    }
    if proposal.status != ProposalStatus::Active {
        return Err(ContractError::ActionNotPending);
    }

    let voter_key = GovDataKey::VoterRecord(proposal_id, voter.clone());
    let has_voted: bool = env.storage().instance().get(&voter_key).unwrap_or(false);
    if has_voted {
        return Err(ContractError::AlreadyApproved);
    }

    if approve {
        proposal.votes_for += voter_weight;
    } else {
        proposal.votes_against += voter_weight;
    }
    proposal.total_voted_weight += voter_weight;

    env.storage().instance().set(&prop_key, &proposal);
    env.storage().instance().set(&voter_key, &true);

    env.events().publish(
        (PROPOSAL_VOTED_EVENT, proposal.key_id.clone(), voter.clone()),
        (proposal_id, approve, voter_weight),
    );

    Ok(())
}

/// Execute proposal after voting period concludes and approval threshold is satisfied.
pub fn execute_proposal(
    env: &Env,
    caller: &Address,
    proposal_id: u32,
) -> Result<ProposalStatus, ContractError> {
    caller.require_auth();

    let prop_key = GovDataKey::Proposal(proposal_id);
    let mut proposal: Proposal = env
        .storage()
        .instance()
        .get(&prop_key)
        .ok_or(ContractError::ProposalNotFound)?;

    let current_ledger = env.ledger().sequence();
    if current_ledger <= proposal.end_ledger {
        return Err(ContractError::TimelockNotElapsed);
    }

    if proposal.status == ProposalStatus::Executed {
        return Err(ContractError::AlreadyRegistered);
    }

    let is_quorum_met = proposal.total_voted_weight >= proposal.min_quorum_weight;
    let is_approval_met = if proposal.total_voted_weight > 0 {
        ((proposal.votes_for * 10_000) / proposal.total_voted_weight) as u32
            >= proposal.pass_threshold_bps
    } else {
        false
    };

    if is_quorum_met && is_approval_met {
        proposal.status = ProposalStatus::Executed;
        env.storage().instance().set(&prop_key, &proposal);

        env.events().publish(
            (PROPOSAL_EXECUTED_EVENT, proposal.key_id.clone()),
            (proposal_id, symbol_short!("executed")),
        );

        Ok(ProposalStatus::Executed)
    } else {
        proposal.status = ProposalStatus::Failed;
        env.storage().instance().set(&prop_key, &proposal);
        Err(ContractError::Unauthorized)
    }
}

/// Read proposal status and full vote tally.
pub fn get_proposal(env: &Env, proposal_id: u32) -> Result<Proposal, ContractError> {
    let prop_key = GovDataKey::Proposal(proposal_id);
    let mut proposal: Proposal = env
        .storage()
        .instance()
        .get(&prop_key)
        .ok_or(ContractError::ProposalNotFound)?;

    let current_ledger = env.ledger().sequence();
    if proposal.status == ProposalStatus::Active && current_ledger > proposal.end_ledger {
        let is_quorum_met = proposal.total_voted_weight >= proposal.min_quorum_weight;
        let is_approval_met = if proposal.total_voted_weight > 0 {
            ((proposal.votes_for * 10_000) / proposal.total_voted_weight) as u32
                >= proposal.pass_threshold_bps
        } else {
            false
        };

        if is_quorum_met && is_approval_met {
            proposal.status = ProposalStatus::Passed;
        } else {
            proposal.status = ProposalStatus::Failed;
        }
    }

    Ok(proposal)
}

pub mod gov_contract {
    use super::*;

    #[contract]
    pub struct GovernanceProposalContract;

    #[contractimpl]
    impl GovernanceProposalContract {
        pub fn init(env: Env, admin: Address, min_holding_threshold: i128) {
            admin.require_auth();
            env.storage().instance().set(&GovDataKey::Admin, &admin);
            env.storage()
                .instance()
                .set(&GovDataKey::MinHoldingThreshold, &min_holding_threshold);
        }

        pub fn create_proposal(
            env: Env,
            proposer: Address,
            params: CreateProposalParams,
        ) -> Result<u32, ContractError> {
            create_proposal(&env, &proposer, &params)
        }

        pub fn vote(
            env: Env,
            voter: Address,
            proposal_id: u32,
            approve: bool,
            voter_weight: i128,
        ) -> Result<(), ContractError> {
            vote(&env, &voter, proposal_id, approve, voter_weight)
        }

        pub fn execute_proposal(
            env: Env,
            caller: Address,
            proposal_id: u32,
        ) -> Result<ProposalStatus, ContractError> {
            execute_proposal(&env, &caller, proposal_id)
        }

        pub fn get_proposal(env: Env, proposal_id: u32) -> Result<Proposal, ContractError> {
            get_proposal(&env, proposal_id)
        }
    }
}
pub use gov_contract::{GovernanceProposalContract, GovernanceProposalContractClient};
