#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, BytesN, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    InvalidAmount = 2,
    CapExceeded = 3,
    NoTrigger = 4,
    Expired = 5,
    AlreadyClaimed = 6,
    ReserveViolation = 7,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cover {
    pub id: u64,
    pub holder: Address,
    pub key_id: Address,
    pub amount: i128,
    pub premium: i128,
    pub expiry: u32,
    pub claimed: bool,
}
#[contracttype]
#[allow(clippy::enum_variant_names)]
enum Key {
    Admin,
    Pool,
    NextId,
    Cover(u64),
    KeyTotal(Address),
    Trigger(Address),
}
const RESERVE_BPS: u32 = 2_000;
#[contract]
pub struct HolderInsurancePool;
#[contractimpl]
impl HolderInsurancePool {
    pub fn initialize(e: Env, admin: Address) -> Result<(), Error> {
        if e.storage().instance().has(&Key::Admin) {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        e.storage().instance().set(&Key::Admin, &admin);
        e.storage().persistent().set(&Key::Pool, &0i128);
        e.storage().persistent().set(&Key::NextId, &0u64);
        Ok(())
    }
    pub fn fund(e: Env, from: Address, amount: i128) -> Result<(), Error> {
        from.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let p = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::Pool)
            .unwrap_or(0)
            .checked_add(amount)
            .ok_or(Error::InvalidAmount)?;
        e.storage().persistent().set(&Key::Pool, &p);
        Ok(())
    }
    pub fn buy_cover(
        e: Env,
        holder: Address,
        key_id: Address,
        amount: i128,
        duration: u32,
        premium_rate_bps: u32,
        max_cover_ratio_bps: u32,
    ) -> Result<u64, Error> {
        holder.require_auth();
        if amount <= 0 || duration == 0 {
            return Err(Error::InvalidAmount);
        }
        let pool = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::Pool)
            .unwrap_or(0);
        let total = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::KeyTotal(key_id.clone()))
            .unwrap_or(0);
        if total.checked_add(amount).ok_or(Error::InvalidAmount)?
            > pool.saturating_mul(max_cover_ratio_bps as i128) / 10_000
        {
            return Err(Error::CapExceeded);
        }
        let premium = amount
            .checked_mul(premium_rate_bps as i128)
            .ok_or(Error::InvalidAmount)?
            / 10_000;
        let id = e
            .storage()
            .persistent()
            .get::<Key, u64>(&Key::NextId)
            .unwrap_or(0)
            + 1;
        e.storage().persistent().set(&Key::NextId, &id);
        e.storage()
            .persistent()
            .set(&Key::KeyTotal(key_id.clone()), &(total + amount));
        e.storage().persistent().set(
            &Key::Cover(id),
            &Cover {
                id,
                holder,
                key_id,
                amount,
                premium,
                expiry: e.ledger().sequence().saturating_add(duration),
                claimed: false,
            },
        );
        Ok(id)
    }
    pub fn declare_trigger(
        e: Env,
        admin: Address,
        key_id: Address,
        trigger_type: u32,
        evidence_hash: BytesN<32>,
    ) -> Result<(), Error> {
        let owner: Address = e
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::Unauthorized)?;
        if owner != admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        e.storage().persistent().set(
            &Key::Trigger(key_id.clone()),
            &(trigger_type, evidence_hash),
        );
        e.events().publish(
            (soroban_sdk::symbol_short!("trigger"), key_id),
            trigger_type,
        );
        Ok(())
    }
    pub fn claim(e: Env, holder: Address, cover_id: u64) -> Result<i128, Error> {
        holder.require_auth();
        let mut c: Cover = e
            .storage()
            .persistent()
            .get(&Key::Cover(cover_id))
            .ok_or(Error::NoTrigger)?;
        if c.holder != holder {
            return Err(Error::Unauthorized);
        }
        if c.claimed {
            return Err(Error::AlreadyClaimed);
        }
        if e.ledger().sequence() >= c.expiry {
            return Err(Error::Expired);
        }
        if !e
            .storage()
            .persistent()
            .has(&Key::Trigger(c.key_id.clone()))
        {
            return Err(Error::NoTrigger);
        }
        let pool = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::Pool)
            .unwrap_or(0);
        let payout = c.amount.min(pool);
        let remaining = pool - payout;
        e.storage().persistent().set(&Key::Pool, &remaining);
        c.claimed = true;
        e.storage().persistent().set(&Key::Cover(cover_id), &c);
        e.events()
            .publish((soroban_sdk::symbol_short!("claim"), holder), payout);
        Ok(payout)
    }
    pub fn withdraw_premium_surplus(e: Env, admin: Address, amount: i128) -> Result<(), Error> {
        let owner: Address = e
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::Unauthorized)?;
        if owner != admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        let pool = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::Pool)
            .unwrap_or(0);
        let total = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::KeyTotal(admin.clone()))
            .unwrap_or(0);
        if amount < 0 || pool - amount < total.saturating_mul(RESERVE_BPS as i128) / 10_000 {
            return Err(Error::ReserveViolation);
        }
        e.storage().persistent().set(&Key::Pool, &(pool - amount));
        Ok(())
    }
}
