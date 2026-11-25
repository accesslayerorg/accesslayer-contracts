use crate::{
    credit_creator_fee_balance, credit_treasury_balance, read_registered_creator_profile,
    ContractError,
};
use soroban_sdk::{contracttype, Address, Env, String, Symbol, Vec};

pub const TIP_SENT_EVENT_NAME: &str = "TipSent";
const PROTOCOL_FEE_BPS: i128 = 200;
const BPS: i128 = 10_000;

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

    let total_key = TipDataKey::Total(key_id.clone());
    let total: i128 = env.storage().persistent().get(&total_key).unwrap_or(0);
    let new_total = total.checked_add(amount).ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&total_key, &new_total);

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
    let history: Vec<TipRecord> = env
        .storage()
        .persistent()
        .get(&TipDataKey::History(key_id.clone()))
        .unwrap_or(Vec::new(env));
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
    env.storage()
        .persistent()
        .get(&TipDataKey::Total(key_id.clone()))
        .unwrap_or(0)
}
