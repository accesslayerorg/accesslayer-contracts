//! Key liquidity depth contract for on-chain order book tracking.
//!
//! Records buy and sell order volumes at each price level for creator keys,
//! maintaining an on-chain order book summary used by TWAP oracles and analytics.
//!
//! Exposes:
//! - [`KeyLiquidityDepthContract::record_order`]: Called by swap and limit order contracts
//!   when an order is placed to add depth at a price level.
//! - [`KeyLiquidityDepthContract::cancel_order_depth`]: Removes cancelled or filled order volume
//!   from depth at a price level.
//! - [`KeyLiquidityDepthContract::get_depth`]: Returns top N bid (descending) and ask (ascending)
//!   price levels with cumulative volume.
//! - [`KeyLiquidityDepthContract::get_price_level_volume`]: Read-only view of volume at a specific level.

#![no_std]
#[cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

#[cfg(test)]
mod test;

/// Event name emitted on order depth update (placement or cancellation).
pub const DEPTH_UPDATED_EVENT_NAME: Symbol = symbol_short!("depth_upd");

/// Stable field order for `DepthUpdated` event payload for downstream indexers.
pub const DEPTH_UPDATED_DATA_FIELDS: [&str; 5] = ["key_id", "price", "amount", "side", "volume"];

/// Default persistent storage TTL thresholds (~30 days).
pub const DEPTH_TTL_THRESHOLD: u32 = 100_000;
pub const DEPTH_TTL_EXTEND_TO: u32 = 518_400;

/// Side of an order in the order book.
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[contracttype]
#[repr(u32)]
pub enum OrderSide {
    /// Buy order (Bid) - buyers bidding for keys.
    Buy = 0,
    /// Sell order (Ask) - sellers offering keys.
    Sell = 1,
}

pub type Side = OrderSide;

/// Errors returned by the key liquidity depth tracker.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LiquidityDepthError {
    /// Price must be strictly positive (> 0).
    InvalidPrice = 1,
    /// Amount must be strictly positive (> 0).
    InvalidAmount = 2,
    /// Price level was not found when attempting to cancel depth.
    PriceLevelNotFound = 3,
    /// Cancellation amount exceeds the available depth volume at the price level.
    InsufficientDepth = 4,
    /// Arithmetic overflow in volume calculation.
    ArithmeticOverflow = 5,
}

pub type DepthError = LiquidityDepthError;

/// Storage keys for persistent state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Active price levels list for a key and side: (Address, OrderSide) -> Vec<i128>
    PriceLevels(Address, OrderSide),
    /// Volume at a specific price level: (Address, OrderSide, i128) -> i128
    LevelVolume(Address, OrderSide, i128),
}

/// A single price level in the order book depth with cumulative volume.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepthLevel {
    /// Price per unit at this level.
    pub price: i128,
    /// Volume available at this specific price level.
    pub volume: i128,
    /// Cumulative volume up to and including this price level.
    pub cumulative_volume: i128,
}

/// Full liquidity depth summary containing top bid and ask levels.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidityDepth {
    /// Bid levels sorted in descending order (highest price first).
    pub bids: Vec<DepthLevel>,
    /// Ask levels sorted in ascending order (lowest price first).
    pub asks: Vec<DepthLevel>,
}

pub type OrderBookDepth = LiquidityDepth;

/// Event payload emitted when order book depth changes.
///
/// Event shape:
/// - topics: `(DEPTH_UPDATED_EVENT_NAME, key_id)`
/// - data: `DepthUpdated`
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepthUpdated {
    /// Creator key whose order depth changed.
    pub key_id: Address,
    /// Price level affected.
    pub price: i128,
    /// Order amount added or cancelled.
    pub amount: i128,
    /// Order side (Buy or Sell).
    pub side: OrderSide,
    /// Remaining total volume at this price level after the update.
    pub volume: i128,
}

pub type DepthUpdatedEvent = DepthUpdated;

/// Returns standard event topics tuple for `DepthUpdated`.
pub fn depth_updated_topics(key_id: &Address) -> (Symbol, Address) {
    (DEPTH_UPDATED_EVENT_NAME, key_id.clone())
}

fn extend_ttl(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, DEPTH_TTL_THRESHOLD, DEPTH_TTL_EXTEND_TO);
}

fn insert_sorted_price(prices: &mut Vec<i128>, price: i128, side: OrderSide) {
    let mut insert_pos = None;
    for i in 0..prices.len() {
        let existing = prices.get(i).unwrap();
        match side {
            OrderSide::Buy => {
                // Descending: highest bid price first
                if price > existing {
                    insert_pos = Some(i);
                    break;
                }
            }
            OrderSide::Sell => {
                // Ascending: lowest ask price first
                if price < existing {
                    insert_pos = Some(i);
                    break;
                }
            }
        }
    }

    if let Some(pos) = insert_pos {
        prices.insert(pos, price);
    } else {
        prices.push_back(price);
    }
}

#[contract]
pub struct KeyLiquidityDepthContract;

pub type LiquidityDepthContract = KeyLiquidityDepthContract;

#[contractimpl]
impl KeyLiquidityDepthContract {
    /// Records an order volume at the specified price level for a creator key.
    ///
    /// Called by swap and limit order contracts on order placement.
    /// Updates depth atomically and emits a [`DepthUpdated`] event.
    pub fn record_order(
        env: Env,
        key_id: Address,
        price: i128,
        amount: i128,
        side: OrderSide,
    ) -> Result<(), LiquidityDepthError> {
        if price <= 0 {
            return Err(LiquidityDepthError::InvalidPrice);
        }
        if amount <= 0 {
            return Err(LiquidityDepthError::InvalidAmount);
        }

        let vol_key = DataKey::LevelVolume(key_id.clone(), side, price);
        let current_vol = env
            .storage()
            .persistent()
            .get::<DataKey, i128>(&vol_key)
            .unwrap_or(0);

        let new_vol = current_vol
            .checked_add(amount)
            .ok_or(LiquidityDepthError::ArithmeticOverflow)?;

        if current_vol == 0 {
            let levels_key = DataKey::PriceLevels(key_id.clone(), side);
            let mut prices = env
                .storage()
                .persistent()
                .get::<DataKey, Vec<i128>>(&levels_key)
                .unwrap_or_else(|| Vec::new(&env));

            insert_sorted_price(&mut prices, price, side);

            env.storage().persistent().set(&levels_key, &prices);
            extend_ttl(&env, &levels_key);
        }

        env.storage().persistent().set(&vol_key, &new_vol);
        extend_ttl(&env, &vol_key);

        env.events().publish(
            depth_updated_topics(&key_id),
            DepthUpdated {
                key_id,
                price,
                amount,
                side,
                volume: new_vol,
            },
        );

        Ok(())
    }

    /// Removes cancelled or filled order volume from depth at a price level.
    ///
    /// Called by swap and limit order contracts on order cancellation or settlement.
    /// Updates depth atomically and emits a [`DepthUpdated`] event.
    pub fn cancel_order_depth(
        env: Env,
        key_id: Address,
        price: i128,
        amount: i128,
        side: OrderSide,
    ) -> Result<(), LiquidityDepthError> {
        if price <= 0 {
            return Err(LiquidityDepthError::InvalidPrice);
        }
        if amount <= 0 {
            return Err(LiquidityDepthError::InvalidAmount);
        }

        let vol_key = DataKey::LevelVolume(key_id.clone(), side, price);
        let current_vol = env
            .storage()
            .persistent()
            .get::<DataKey, i128>(&vol_key)
            .unwrap_or(0);

        if current_vol == 0 {
            return Err(LiquidityDepthError::PriceLevelNotFound);
        }

        if amount > current_vol {
            return Err(LiquidityDepthError::InsufficientDepth);
        }

        let new_vol = current_vol - amount;

        if new_vol == 0 {
            env.storage().persistent().remove(&vol_key);

            let levels_key = DataKey::PriceLevels(key_id.clone(), side);
            if let Some(mut prices) = env
                .storage()
                .persistent()
                .get::<DataKey, Vec<i128>>(&levels_key)
            {
                let mut remove_idx = None;
                for i in 0..prices.len() {
                    if prices.get(i).unwrap() == price {
                        remove_idx = Some(i);
                        break;
                    }
                }
                if let Some(idx) = remove_idx {
                    prices.remove(idx);
                    if prices.is_empty() {
                        env.storage().persistent().remove(&levels_key);
                    } else {
                        env.storage().persistent().set(&levels_key, &prices);
                        extend_ttl(&env, &levels_key);
                    }
                }
            }
        } else {
            env.storage().persistent().set(&vol_key, &new_vol);
            extend_ttl(&env, &vol_key);
        }

        env.events().publish(
            depth_updated_topics(&key_id),
            DepthUpdated {
                key_id,
                price,
                amount,
                side,
                volume: new_vol,
            },
        );

        Ok(())
    }

    /// Returns top `levels` bid and ask price levels with cumulative volume.
    ///
    /// - Bids are sorted in descending order of price (highest bid first).
    /// - Asks are sorted in ascending order of price (lowest ask first).
    /// - Cumulative volume accumulates from top of book downward.
    pub fn get_depth(env: Env, key_id: Address, levels: u32) -> LiquidityDepth {
        let mut bids = Vec::new(&env);
        let mut asks = Vec::new(&env);

        if levels == 0 {
            return LiquidityDepth { bids, asks };
        }

        // Bids (Buy side)
        let bid_levels_key = DataKey::PriceLevels(key_id.clone(), OrderSide::Buy);
        if let Some(bid_prices) = env
            .storage()
            .persistent()
            .get::<DataKey, Vec<i128>>(&bid_levels_key)
        {
            let mut cum_vol: i128 = 0;
            let count = levels.min(bid_prices.len());
            for i in 0..count {
                let p = bid_prices.get(i).unwrap();
                let vol_key = DataKey::LevelVolume(key_id.clone(), OrderSide::Buy, p);
                let vol = env
                    .storage()
                    .persistent()
                    .get::<DataKey, i128>(&vol_key)
                    .unwrap_or(0);
                if vol > 0 {
                    cum_vol = cum_vol.saturating_add(vol);
                    bids.push_back(DepthLevel {
                        price: p,
                        volume: vol,
                        cumulative_volume: cum_vol,
                    });
                }
            }
        }

        // Asks (Sell side)
        let ask_levels_key = DataKey::PriceLevels(key_id.clone(), OrderSide::Sell);
        if let Some(ask_prices) = env
            .storage()
            .persistent()
            .get::<DataKey, Vec<i128>>(&ask_levels_key)
        {
            let mut cum_vol: i128 = 0;
            let count = levels.min(ask_prices.len());
            for i in 0..count {
                let p = ask_prices.get(i).unwrap();
                let vol_key = DataKey::LevelVolume(key_id.clone(), OrderSide::Sell, p);
                let vol = env
                    .storage()
                    .persistent()
                    .get::<DataKey, i128>(&vol_key)
                    .unwrap_or(0);
                if vol > 0 {
                    cum_vol = cum_vol.saturating_add(vol);
                    asks.push_back(DepthLevel {
                        price: p,
                        volume: vol,
                        cumulative_volume: cum_vol,
                    });
                }
            }
        }

        LiquidityDepth { bids, asks }
    }

    /// Read-only view of total volume recorded at a specific price level.
    pub fn get_price_level_volume(env: Env, key_id: Address, price: i128, side: OrderSide) -> i128 {
        let vol_key = DataKey::LevelVolume(key_id, side, price);
        env.storage()
            .persistent()
            .get::<DataKey, i128>(&vol_key)
            .unwrap_or(0)
    }
}
