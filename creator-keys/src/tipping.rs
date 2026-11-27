use crate::{
    credit_creator_fee_balance, credit_treasury_balance, read_registered_creator_profile,
    ContractError,
};
use soroban_sdk::{contracttype, Address, Env, String, Symbol, Vec};

pub const TIP_SENT_EVENT_NAME: &str = "TipSent";
const PROTOCOL_FEE_BPS: i128 = 200;
const BPS: i128 = 10_000;

/// TTL policy constants for tipping persistent entries, centralized per entry type.
pub mod ttl {
    /// Ledger count below which a persistent entry is extended on access/write.
    pub const TIP_HISTORY_THRESHOLD: u32 = 17_280;
    /// Ledger count an extended tip history entry is bumped to.
    pub const TIP_HISTORY_EXTEND_TO: u32 = 518_400;
    /// Ledger count below which a tip total entry is extended on access/write.
    pub const TIP_TOTAL_THRESHOLD: u32 = 17_280;
    /// Ledger count an extended tip total entry is bumped to.
    pub const TIP_TOTAL_EXTEND_TO: u32 = 518_400;
    /// Maximum number of keys accepted in a single `bump` call.
    pub const MAX_BUMP_KEYS: u32 = 32;
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TipRecord {
    pub tipper: Address,
    pub amount: i128,
    pub message_hash: Option<String>,
}

#[derive(Clone)]
#[contracttype]
enum TipDataKey {
    History(Address),
    Total(Address),
}

fn extend_history(env: &Env, key: &TipDataKey) {
    env.storage().persistent().extend_ttl(
        key,
        ttl::TIP_HISTORY_THRESHOLD,
        ttl::TIP_HISTORY_EXTEND_TO,
    );
}

fn extend_total(env: &Env, key: &TipDataKey) {
    env.storage().persistent().extend_ttl(
        key,
        ttl::TIP_TOTAL_THRESHOLD,
        ttl::TIP_TOTAL_EXTEND_TO,
    );
}

pub fn send_tip(
    env: &Env,
    key_id: &Address,
    tipper: &Address,
    amount: i128,
    message_hash: Option<String>,
) -> Result<TipRecord, ContractError> {
    tipper.require_auth();
    read_registered_creator_profile(env, key_id)?;
    if amount <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    let fee = amount
        .checked_mul(PROTOCOL_FEE_BPS)
        .ok_or(ContractError::Overflow)?
        / BPS;
    let creator_amount = amount.checked_sub(fee).ok_or(ContractError::Overflow)?;
    credit_treasury_balance(env, fee)?;
    credit_creator_fee_balance(env, key_id, creator_amount)?;

    let record = TipRecord {
        tipper: tipper.clone(),
        amount,
        message_hash,
    };
    let history_key = TipDataKey::History(key_id.clone());
    let mut history: Vec<TipRecord> = env
        .storage()
        .persistent()
        .get(&history_key)
        .unwrap_or(Vec::new(env));
    history.push_back(record.clone());
    env.storage().persistent().set(&history_key, &history);
    extend_history(env, &history_key);

    let total_key = TipDataKey::Total(key_id.clone());
    let total: i128 = env.storage().persistent().get(&total_key).unwrap_or(0);
    let new_total = total.checked_add(amount).ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&total_key, &new_total);
    extend_total(env, &total_key);

    env.events().publish(
        (
            Symbol::new(env, TIP_SENT_EVENT_NAME),
            key_id.clone(),
            record.tipper.clone(),
        ),
        record.clone(),
    );
    Ok(record)
}

pub fn get_tip_history(env: &Env, key_id: &Address, offset: u32, limit: u32) -> Vec<TipRecord> {
    let history_key = TipDataKey::History(key_id.clone());
    let history: Vec<TipRecord> = match env.storage().persistent().get(&history_key) {
        Some(h) => {
            extend_history(env, &history_key);
            h
        }
        None => Vec::new(env),
    };
    let mut result = Vec::new(env);
    if limit == 0 {
        return result;
    }
    let end = offset.saturating_add(limit).min(history.len());
    let mut index = offset;
    while index < end {
        if let Some(record) = history.get(index) {
            result.push_back(record);
        }
        index += 1;
    }
    result
}

pub fn get_total_tips(env: &Env, key_id: &Address) -> i128 {
    let total_key = TipDataKey::Total(key_id.clone());
    match env.storage().persistent().get(&total_key) {
        Some(t) => {
            extend_total(env, &total_key);
            t
        }
        None => 0,
    }
}

/// Extends the TTL of the tip history and total entries for `key_id`.
/// Callable by anyone. Only keys belonging to a registered creator are
/// extended, and at most `ttl::MAX_BUMP_KEYS` keys are processed per call.
/// Returns the number of entries extended.
pub fn bump(env: &Env, key_id: &Address, keys: Vec<Symbol>) -> Result<u32, ContractError> {
    if keys.len() > ttl::MAX_BUMP_KEYS {
        return Err(ContractError::Overflow);
    }
    read_registered_creator_profile(env, key_id)?;
    let history_sym = Symbol::new(env, "history");
    let total_sym = Symbol::new(env, "total");
    let mut extended = 0u32;
    for sym in keys.iter() {
        if sym == history_sym {
            let k = TipDataKey::History(key_id.clone());
            if env.storage().persistent().has(&k) {
                extend_history(env, &k);
                extended += 1;
            }
        } else if sym == total_sym {
            let k = TipDataKey::Total(key_id.clone());
            if env.storage().persistent().has(&k) {
                extend_total(env, &k);
                extended += 1;
            }
        }
    }
    Ok(extended)
}

/// Monitoring view: reports which tip entries for `key_id` have a remaining
/// TTL at or below `threshold` ledgers. Returns (history_nearing, total_nearing).
pub fn entries_nearing_expiry(env: &Env, key_id: &Address, threshold: u32) -> Vec<Symbol> {
    let mut out = Vec::new(env);
    let history_key = TipDataKey::History(key_id.clone());
    if env.storage().persistent().has(&history_key)
        && env.storage().persistent().get_ttl(&history_key) <= threshold
    {
        out.push_back(Symbol::new(env, "history"));
    }
    let total_key = TipDataKey::Total(key_id.clone());
    if env.storage().persistent().has(&total_key)
        && env.storage().persistent().get_ttl(&total_key) <= threshold
    {
        out.push_back(Symbol::new(env, "total"));
    }
    out
}
