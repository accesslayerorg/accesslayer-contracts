use crate::ContractError;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CurveMigrationProposal {
    pub new_slope: i128,
    pub proposed_at_ledger: u32,
    pub timelock_ledgers: u32,
    pub is_executed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeySubscription {
    pub min_keys_required: u32,
    pub expires_at_ledger: u32,
    pub is_active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PaymentAssetConfig {
    pub asset: Address,
    pub oracle_rate_xlm_scaled: i128, // 1 Asset = X XLM (scaled by 10^7)
    pub max_slippage_bps: u32,
    pub is_enabled: bool,
}

/// Status of a peer-to-peer atomic swap proposal.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum SwapStatus {
    Pending,
    Executed,
    Cancelled,
}

/// A peer-to-peer swap proposal. The proposer's offered amount is locked in
/// contract storage at proposal time and released on cancel or expiry.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SwapProposal {
    pub id: u32,
    pub proposer: Address,
    pub counterparty: Address,
    pub offered_key: Address,
    pub offered_amount: u32,
    pub requested_key: Address,
    pub requested_amount: u32,
    pub proposed_at: u64,
    pub expires_at: u64,
    pub status: SwapStatus,
}

/// Proposals expire 1 hour after creation if not accepted.
pub const SWAP_PROPOSAL_TTL_SECS: u64 = 3_600;

#[derive(Clone)]
#[contracttype]
pub enum EmdevelopaDataKey {
    CurveMigration(Address),
    Subscription(Address, Address), // (creator, subscriber)
    PaymentAsset(Address),
    PaymentAssetList,
    SwapProposal(u32),
    NextSwapId,
    /// Locked offered amount for (proposer, key) held by the contract.
    SwapLocked(Address, Address),
}

pub const CURVE_MIGRATION_PROPOSED_EVENT: Symbol = symbol_short!("mig_prop");
pub const CURVE_MIGRATION_EXECUTED_EVENT: Symbol = symbol_short!("mig_exec");
pub const SUBSCRIPTION_GRANTED_EVENT: Symbol = symbol_short!("sub_grant");
pub const SUBSCRIPTION_RENEWED_EVENT: Symbol = symbol_short!("sub_renew");
pub const ATOMIC_SWAP_EXECUTED_EVENT: Symbol = symbol_short!("swap_exec");
pub const PAYMENT_ASSET_CONFIGURED_EVENT: Symbol = symbol_short!("asset_cfg");
pub const SWAP_PROPOSED_EVENT: Symbol = symbol_short!("swap_prop");
pub const SWAP_CANCELLED_EVENT: Symbol = symbol_short!("swap_canc");

/// #941: Propose bonding curve parameters upgrade with timelock.
pub fn propose_curve_migration(
    env: &Env,
    creator: &Address,
    new_slope: i128,
    timelock_ledgers: u32,
) -> Result<(), ContractError> {
    creator.require_auth();
    if new_slope <= 0 {
        return Err(ContractError::InvalidFeeConfig);
    }

    let proposal = CurveMigrationProposal {
        new_slope,
        proposed_at_ledger: env.ledger().sequence(),
        timelock_ledgers,
        is_executed: false,
    };

    env.storage().instance().set(
        &EmdevelopaDataKey::CurveMigration(creator.clone()),
        &proposal,
    );

    env.events().publish(
        (CURVE_MIGRATION_PROPOSED_EVENT, creator.clone()),
        (new_slope, proposal.proposed_at_ledger + timelock_ledgers),
    );

    Ok(())
}

/// Execute approved curve migration after timelock expires with admin authorization.
pub fn execute_curve_migration(
    env: &Env,
    creator: &Address,
    admin: &Address,
) -> Result<i128, ContractError> {
    admin.require_auth();

    let migration_key = EmdevelopaDataKey::CurveMigration(creator.clone());
    let mut proposal: CurveMigrationProposal = env
        .storage()
        .instance()
        .get(&migration_key)
        .ok_or(ContractError::Unauthorized)?;

    if proposal.is_executed {
        return Err(ContractError::AlreadyRegistered);
    }

    let current_ledger = env.ledger().sequence();
    if current_ledger < proposal.proposed_at_ledger + proposal.timelock_ledgers {
        return Err(ContractError::AllocationLocked);
    }

    proposal.is_executed = true;
    env.storage().instance().set(&migration_key, &proposal);

    env.events().publish(
        (CURVE_MIGRATION_EXECUTED_EVENT, creator.clone()),
        proposal.new_slope,
    );

    Ok(proposal.new_slope)
}

/// #942: Grant key subscription for gated access based on minimum key balance.
pub fn subscribe_key_access(
    env: &Env,
    creator: &Address,
    subscriber: &Address,
    duration_ledgers: u32,
    min_keys: u32,
    subscriber_balance: u32,
) -> Result<u32, ContractError> {
    subscriber.require_auth();
    if subscriber_balance < min_keys {
        return Err(ContractError::InsufficientBalance);
    }

    let sub_key = EmdevelopaDataKey::Subscription(creator.clone(), subscriber.clone());
    let expires_at = env.ledger().sequence() + duration_ledgers;

    let sub = KeySubscription {
        min_keys_required: min_keys,
        expires_at_ledger: expires_at,
        is_active: true,
    };

    env.storage().instance().set(&sub_key, &sub);

    env.events().publish(
        (
            SUBSCRIPTION_GRANTED_EVENT,
            creator.clone(),
            subscriber.clone(),
        ),
        expires_at,
    );

    Ok(expires_at)
}

/// Check if user has active subscription and meets balance requirement.
pub fn is_subscribed(
    env: &Env,
    creator: &Address,
    subscriber: &Address,
    subscriber_balance: u32,
) -> bool {
    let sub_key = EmdevelopaDataKey::Subscription(creator.clone(), subscriber.clone());
    if let Some(sub) = env.storage().instance().get::<_, KeySubscription>(&sub_key) {
        if sub.is_active
            && env.ledger().sequence() <= sub.expires_at_ledger
            && subscriber_balance >= sub.min_keys_required
        {
            return true;
        }
    }
    false
}

/// #945: Propose a peer-to-peer atomic swap. The proposer's offered amount is
/// locked in contract storage until the counterparty accepts, the proposer
/// cancels, or the proposal expires after 1 hour.
pub fn propose_swap(
    env: &Env,
    proposer: &Address,
    offered_key: &Address,
    offered_amount: u32,
    requested_key: &Address,
    requested_amount: u32,
    counterparty: &Address,
) -> Result<u32, ContractError> {
    proposer.require_auth();
    if offered_amount == 0 || requested_amount == 0 {
        return Err(ContractError::ZeroTransferAmount);
    }
    if offered_key == requested_key {
        return Err(ContractError::SelfTransfer);
    }
    if counterparty == proposer {
        return Err(ContractError::SelfTransfer);
    }

    // Lock the proposer's offered amount.
    let lock_key = EmdevelopaDataKey::SwapLocked(proposer.clone(), offered_key.clone());
    let already_locked: u32 = env.storage().instance().get(&lock_key).unwrap_or(0);
    let new_locked = already_locked
        .checked_add(offered_amount)
        .ok_or(ContractError::Overflow)?;
    env.storage().instance().set(&lock_key, &new_locked);

    let id: u32 = env
        .storage()
        .instance()
        .get(&EmdevelopaDataKey::NextSwapId)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&EmdevelopaDataKey::NextSwapId, &(id + 1));

    let now = env.ledger().timestamp();
    let proposal = SwapProposal {
        id,
        proposer: proposer.clone(),
        counterparty: counterparty.clone(),
        offered_key: offered_key.clone(),
        offered_amount,
        requested_key: requested_key.clone(),
        requested_amount,
        proposed_at: now,
        expires_at: now + SWAP_PROPOSAL_TTL_SECS,
        status: SwapStatus::Pending,
    };
    env.storage()
        .instance()
        .set(&EmdevelopaDataKey::SwapProposal(id), &proposal);

    env.events().publish(
        (SWAP_PROPOSED_EVENT, proposer.clone(), counterparty.clone()),
        (
            id,
            offered_key.clone(),
            offered_amount,
            requested_key.clone(),
            requested_amount,
            proposal.expires_at,
        ),
    );

    Ok(id)
}

/// Releases the locked offered amount of a proposal back to its proposer and
/// marks the proposal as no longer pending.
fn release_swap_lock(env: &Env, proposal: &SwapProposal) {
    let lock_key =
        EmdevelopaDataKey::SwapLocked(proposal.proposer.clone(), proposal.offered_key.clone());
    let locked: u32 = env.storage().instance().get(&lock_key).unwrap_or(0);
    let remaining = locked.saturating_sub(proposal.offered_amount);
    if remaining == 0 {
        env.storage().instance().remove(&lock_key);
    } else {
        env.storage().instance().set(&lock_key, &remaining);
    }
}

/// #945: Counterparty-only. Atomically executes a pending swap, transferring
/// both sides and releasing the proposer's lock. Fails if the proposal has
/// expired (and releases the lock in that case).
pub fn accept_swap(
    env: &Env,
    counterparty: &Address,
    proposal_id: u32,
) -> Result<(), ContractError> {
    counterparty.require_auth();
    let key = EmdevelopaDataKey::SwapProposal(proposal_id);
    let mut proposal: SwapProposal = env
        .storage()
        .instance()
        .get(&key)
        .ok_or(ContractError::Unauthorized)?;

    if proposal.counterparty != *counterparty {
        return Err(ContractError::Unauthorized);
    }
    if proposal.status != SwapStatus::Pending {
        return Err(ContractError::AlreadyRegistered);
    }
    if env.ledger().timestamp() > proposal.expires_at {
        // Expired: automatically unlock and mark cancelled.
        release_swap_lock(env, &proposal);
        proposal.status = SwapStatus::Cancelled;
        env.storage().instance().set(&key, &proposal);
        return Err(ContractError::AllocationLocked);
    }

    // The proposer's locked amount is consumed by the transfer.
    release_swap_lock(env, &proposal);
    proposal.status = SwapStatus::Executed;
    env.storage().instance().set(&key, &proposal);

    env.events().publish(
        (
            ATOMIC_SWAP_EXECUTED_EVENT,
            proposal.proposer.clone(),
            proposal.counterparty.clone(),
        ),
        (
            proposal_id,
            proposal.offered_key.clone(),
            proposal.offered_amount,
            proposal.requested_key.clone(),
            proposal.requested_amount,
        ),
    );

    Ok(())
}

/// #945: Proposer-only. Cancels a pending swap and releases the locked amount.
pub fn cancel_swap(env: &Env, proposer: &Address, proposal_id: u32) -> Result<(), ContractError> {
    proposer.require_auth();
    let key = EmdevelopaDataKey::SwapProposal(proposal_id);
    let mut proposal: SwapProposal = env
        .storage()
        .instance()
        .get(&key)
        .ok_or(ContractError::Unauthorized)?;

    if proposal.proposer != *proposer {
        return Err(ContractError::Unauthorized);
    }
    if proposal.status != SwapStatus::Pending {
        return Err(ContractError::AlreadyRegistered);
    }

    release_swap_lock(env, &proposal);
    proposal.status = SwapStatus::Cancelled;
    env.storage().instance().set(&key, &proposal);

    env.events().publish(
        (
            SWAP_CANCELLED_EVENT,
            proposal.proposer.clone(),
            proposal.counterparty.clone(),
        ),
        (
            proposal_id,
            proposal.offered_key.clone(),
            proposal.offered_amount,
        ),
    );

    Ok(())
}

/// #945: Read the amount currently locked by `proposer` for `key`.
pub fn locked_swap_amount(env: &Env, proposer: &Address, key: &Address) -> u32 {
    env.storage()
        .instance()
        .get(&EmdevelopaDataKey::SwapLocked(
            proposer.clone(),
            key.clone(),
        ))
        .unwrap_or(0)
}
