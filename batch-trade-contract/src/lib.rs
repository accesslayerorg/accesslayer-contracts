#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};
const MAX_ITEMS: u32 = 15;
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    TooManyItems = 1,
    InvalidAmount = 2,
    SlippageExceeded = 3,
    Overflow = 4,
}
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Buy = 0,
    Sell = 1,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapItem {
    pub key_id: Address,
    pub amount: i128,
    pub direction: Direction,
    pub max_slippage_bps: u32,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapResult {
    pub key_id: Address,
    pub input: i128,
    pub output: i128,
    pub fee: i128,
    pub price_impact_bps: u32,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyMarket {
    pub buy_price: i128,
    pub sell_price: i128,
    pub liquidity: i128,
    pub fee_bps: u32,
}
#[contracttype]
enum Key {
    Market(Address),
}
#[contract]
pub struct BatchTradeContract;
fn quote(m: &KeyMarket, i: &SwapItem) -> Result<SwapResult, Error> {
    if i.amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    let p = match i.direction {
        Direction::Buy => m.buy_price,
        Direction::Sell => m.sell_price,
    };
    if p <= 0 {
        return Err(Error::InvalidAmount);
    }
    let fee = i
        .amount
        .checked_mul(m.fee_bps as i128)
        .ok_or(Error::Overflow)?
        / 10_000;
    let out = i.amount.checked_mul(p).ok_or(Error::Overflow)? / 1_000_000;
    let impact = if m.liquidity == 0 {
        10_000
    } else {
        ((i.amount * 10_000) / m.liquidity).min(10_000) as u32
    };
    Ok(SwapResult {
        key_id: i.key_id.clone(),
        input: i.amount,
        output: out.saturating_sub(fee),
        fee,
        price_impact_bps: impact,
    })
}
#[contractimpl]
impl BatchTradeContract {
    pub fn set_market(e: Env, key: Address, market: KeyMarket) {
        e.storage().persistent().set(&Key::Market(key), &market)
    }
    pub fn get_batch_quote(e: Env, items: Vec<SwapItem>) -> Result<Vec<SwapResult>, Error> {
        if items.len() > MAX_ITEMS {
            return Err(Error::TooManyItems);
        }
        let mut out = Vec::new(&e);
        for i in items.iter() {
            let m = e
                .storage()
                .persistent()
                .get(&Key::Market(i.key_id.clone()))
                .unwrap_or(KeyMarket {
                    buy_price: 1_000_000,
                    sell_price: 1_000_000,
                    liquidity: i128::MAX,
                    fee_bps: 0,
                });
            out.push_back(quote(&m, &i)?);
        }
        Ok(out)
    }
    pub fn batch_swap(
        e: Env,
        caller: Address,
        items: Vec<SwapItem>,
    ) -> Result<Vec<SwapResult>, Error> {
        caller.require_auth();
        let results = Self::get_batch_quote(e.clone(), items.clone())?;
        for (i, r) in items.iter().zip(results.iter()) {
            if r.price_impact_bps > i.max_slippage_bps {
                return Err(Error::SlippageExceeded);
            }
        }
        e.events().publish(
            (soroban_sdk::symbol_short!("batch"), caller),
            results.clone(),
        );
        Ok(results)
    }
}
