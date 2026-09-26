#![no_std]
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
    pub conversion_ratio_bps: u32, // 10000 = 1:1
    pub is_approved_by_admin: bool,
    pub is_executed: bool,
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
    MergeProposal(Address), // source_key
    SunsetStatus(Address),
}

pub const ACL_UPDATED_EVENT: Symbol = symbol_short!("acl_upd");
pub const BUY_LIMIT_CONFIGURED_EVENT: Symbol = symbol_short!("buy_lim");
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
pub fn propose_key_merge(
    env: &Env,
    creator: &Address,
    source_key: &Address,
    target_key: &Address,
    conversion_ratio_bps: u32,
) -> Result<(), ContractError> {
    creator.require_auth();
    if source_key == target_key || conversion_ratio_bps == 0 {
        return Err(ContractError::InvalidFeeConfig);
    }

    let proposal = KeyMergeProposal {
        source_key: source_key.clone(),
        target_key: target_key.clone(),
        conversion_ratio_bps,
        is_approved_by_admin: false,
        is_executed: false,
    };

    env.storage().instance().set(
        &EmwulrdDataKey::MergeProposal(source_key.clone()),
        &proposal,
    );

    Ok(())
}

/// Execute approved key merge and migrate holder balance ratio.
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
        .ok_or(ContractError::Unauthorized)?;

    if proposal.is_executed || proposal.target_key != *target_key {
        return Err(ContractError::AlreadyRegistered);
    }

    proposal.is_approved_by_admin = true;
    proposal.is_executed = true;

    env.storage().instance().set(&proposal_key, &proposal);

    env.events().publish(
        (MERGE_EXECUTED_EVENT, source_key.clone(), target_key.clone()),
        proposal.conversion_ratio_bps,
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
