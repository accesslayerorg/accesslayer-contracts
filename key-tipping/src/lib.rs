#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Env, String, Symbol, Vec,
};

const PROTOCOL_FEE_BPS: i128 = 200;
const BPS_DENOMINATOR: i128 = 10_000;
const MAX_PAGE_SIZE: u32 = 100;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TipError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    UnknownKey = 5,
    Overflow = 6,
    InvalidPage = 7,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipRecord {
    pub tipper: Address,
    pub creator: Address,
    /// Amount transferred to the creator after the protocol fee.
    pub amount: i128,
    pub protocol_fee: i128,
    pub message_hash: Option<String>,
    pub ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipSentEvent {
    pub creator: Address,
    pub amount: i128,
    pub protocol_fee: i128,
    pub message_hash: Option<String>,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Asset,
    Treasury,
    Creator(Address),
    Count(Address),
    Tip(Address, u32),
    Total(Address),
}

#[contract]
pub struct KeyTippingContract;

#[contractimpl]
impl KeyTippingContract {
    /// Configure the payment token and protocol treasury once.
    pub fn initialize(
        env: Env,
        admin: Address,
        asset: Address,
        treasury: Address,
    ) -> Result<(), TipError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(TipError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Asset, &asset);
        env.storage().instance().set(&DataKey::Treasury, &treasury);
        Ok(())
    }

    /// Register which creator receives tips for a key identifier.
    pub fn set_creator(env: Env, key_id: Address, creator: Address) -> Result<(), TipError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(TipError::NotInitialized)?;
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Creator(key_id), &creator);
        Ok(())
    }

    /// Transfer a tip to a key's creator and append an on-chain receipt.
    pub fn send_tip(
        env: Env,
        key_id: Address,
        tipper: Address,
        amount: i128,
        message_hash: Option<String>,
    ) -> Result<TipRecord, TipError> {
        tipper.require_auth();
        if amount <= 0 {
            return Err(TipError::InvalidAmount);
        }

        let creator: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Creator(key_id.clone()))
            .ok_or(TipError::UnknownKey)?;
        let asset: Address = env
            .storage()
            .instance()
            .get(&DataKey::Asset)
            .ok_or(TipError::NotInitialized)?;
        let treasury: Address = env
            .storage()
            .instance()
            .get(&DataKey::Treasury)
            .ok_or(TipError::NotInitialized)?;

        let product = amount
            .checked_mul(PROTOCOL_FEE_BPS)
            .ok_or(TipError::Overflow)?;
        let fee = product / BPS_DENOMINATOR;
        let creator_amount = amount.checked_sub(fee).ok_or(TipError::Overflow)?;

        let token = token::Client::new(&env, &asset);
        let contract = env.current_contract_address();
        token.transfer(&tipper, &contract, &amount);
        token.transfer(&contract, &creator, &creator_amount);
        if fee > 0 {
            token.transfer(&contract, &treasury, &fee);
        }

        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Count(key_id.clone()))
            .unwrap_or(0);
        let next = count.checked_add(1).ok_or(TipError::Overflow)?;
        let record = TipRecord {
            tipper: tipper.clone(),
            creator: creator.clone(),
            amount: creator_amount,
            protocol_fee: fee,
            message_hash: message_hash.clone(),
            ledger: env.ledger().sequence(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Tip(key_id.clone(), count), &record);
        env.storage()
            .persistent()
            .set(&DataKey::Count(key_id.clone()), &next);
        let total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Total(key_id.clone()))
            .unwrap_or(0);
        let new_total = total
            .checked_add(creator_amount)
            .ok_or(TipError::Overflow)?;
        env.storage()
            .persistent()
            .set(&DataKey::Total(key_id.clone()), &new_total);

        env.events().publish(
            (Symbol::new(&env, "TipSent"), key_id, tipper),
            TipSentEvent {
                creator,
                amount: creator_amount,
                protocol_fee: fee,
                message_hash,
            },
        );
        Ok(record)
    }

    /// Read newest-first tip history. `offset` is measured from the newest tip.
    pub fn get_tip_history(
        env: Env,
        key_id: Address,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<TipRecord>, TipError> {
        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(TipError::InvalidPage);
        }
        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Count(key_id.clone()))
            .unwrap_or(0);
        let mut out = Vec::new(&env);
        let mut index = count.saturating_sub(offset);
        let mut remaining = limit;
        while index > 0 && remaining > 0 {
            index -= 1;
            if let Some(record) = env
                .storage()
                .persistent()
                .get(&DataKey::Tip(key_id.clone(), index))
            {
                out.push_back(record);
                remaining -= 1;
            }
        }
        Ok(out)
    }

    /// Cumulative amount received by the creator, after protocol fees.
    pub fn get_total_tips(env: Env, key_id: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Total(key_id))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
