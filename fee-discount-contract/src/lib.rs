#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};

const WINDOW: u64 = 30 * 24 * 60 * 60;
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidTier = 3,
    Overflow = 4,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Discount {
    pub tier: u32,
    pub discount_bps: u32,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tier {
    pub threshold: i128,
    pub discount_bps: u32,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum Key {
    Admin,
    Tiers,
    Volume(Address),
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct Volume {
    amount: i128,
    started_at: u64,
    tier: u32,
}
#[contract]
pub struct FeeDiscountContract;
#[contractimpl]
impl FeeDiscountContract {
    pub fn initialize(e: Env, admin: Address) -> Result<(), Error> {
        if e.storage().instance().has(&Key::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        e.storage().instance().set(&Key::Admin, &admin);
        e.storage()
            .instance()
            .set(&Key::Tiers, &Vec::<Tier>::new(&e));
        Ok(())
    }
    pub fn set_tiers(e: Env, caller: Address, tiers: Vec<Tier>) -> Result<(), Error> {
        let admin: Address = e
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::Unauthorized)?;
        if caller != admin {
            return Err(Error::Unauthorized);
        }
        caller.require_auth();
        let mut last = 0i128;
        for t in tiers.iter() {
            if t.threshold <= last || t.discount_bps > 10_000 {
                return Err(Error::InvalidTier);
            }
            last = t.threshold;
        }
        e.storage().instance().set(&Key::Tiers, &tiers);
        Ok(())
    }
    pub fn record_volume(e: Env, wallet: Address, amount: i128) -> Result<Discount, Error> {
        wallet.require_auth();
        if amount < 0 {
            return Err(Error::InvalidTier);
        }
        let now = e.ledger().timestamp();
        let old = e
            .storage()
            .persistent()
            .get::<Key, Volume>(&Key::Volume(wallet.clone()));
        let mut v = old.unwrap_or(Volume {
            amount: 0,
            started_at: now,
            tier: 0,
        });
        if now.saturating_sub(v.started_at) >= WINDOW {
            v = Volume {
                amount: 0,
                started_at: now,
                tier: 0,
            };
        }
        v.amount = v.amount.checked_add(amount).ok_or(Error::Overflow)?;
        let tiers = e
            .storage()
            .instance()
            .get::<Key, Vec<Tier>>(&Key::Tiers)
            .unwrap_or_else(|| Vec::new(&e));
        let mut tier = 0u32;
        let mut bps = 0u32;
        for (i, t) in tiers.iter().enumerate() {
            if v.amount >= t.threshold {
                tier = (i + 1) as u32;
                bps = t.discount_bps;
            }
        }
        if tier != v.tier {
            e.events().publish(
                (soroban_sdk::symbol_short!("tier"), wallet.clone()),
                Discount {
                    tier,
                    discount_bps: bps,
                },
            );
            v.tier = tier;
        }
        e.storage().persistent().set(&Key::Volume(wallet), &v);
        Ok(Discount {
            tier,
            discount_bps: bps,
        })
    }
    pub fn get_discount(e: Env, wallet: Address) -> Discount {
        let v = e
            .storage()
            .persistent()
            .get::<Key, Volume>(&Key::Volume(wallet))
            .unwrap_or(Volume {
                amount: 0,
                started_at: e.ledger().timestamp(),
                tier: 0,
            });
        let tiers = e
            .storage()
            .instance()
            .get::<Key, Vec<Tier>>(&Key::Tiers)
            .unwrap_or_else(|| Vec::new(&e));
        let mut bps = 0;
        for (i, t) in tiers.iter().enumerate() {
            if v.amount >= t.threshold && (i + 1) as u32 == v.tier {
                bps = t.discount_bps;
            }
        }
        Discount {
            tier: v.tier,
            discount_bps: bps,
        }
    }
}
