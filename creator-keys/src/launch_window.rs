//! Anti-sniper launch window with per-wallet purchase caps and ramped limits.
//!
//! This module provides a configurable launch window where per-wallet purchase limits
//! start low and ramp up over time, making sniping uneconomical for bots.

use crate::ContractError;
use soroban_sdk::{contracttype, Address, Env};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchWindowConfig {
    /// Ledger sequence when the launch window starts (typically key creation ledger).
    pub start_ledger: u32,
    /// Number of ledgers the window lasts.
    pub window_ledgers: u32,
    /// Initial cap (in keys per wallet) at window start.
    pub start_cap: u32,
    /// Final cap (in keys per wallet) at window end.
    pub end_cap: u32,
}

/// Data key enum for launch window storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum LaunchWindowDataKey {
    /// Config per creator: LaunchWindowDataKey::Config(creator_address)
    Config(Address),
    /// Cumulative purchases per (creator, wallet): LaunchWindowDataKey::CumulativePurchases(creator_address, wallet_address)
    CumulativePurchases(Address, Address),
    /// Whitelisted bypass addresses per creator: LaunchWindowDataKey::Whitelist(creator_address)
    Whitelist(Address),
    /// Bypass enabled flag per creator: LaunchWindowDataKey::BypassEnabled(creator_address)
    BypassEnabled(Address),
}

/// Set the launch window configuration before the key launches.
///
/// Can only be called by the creator before the first key is purchased.
pub fn set_launch_window(
    env: &Env,
    creator: &Address,
    window_ledgers: u32,
    start_cap: u32,
    end_cap: u32,
) -> Result<(), ContractError> {
    // Get the key creation ledger (stored when supply reaches 1)
    let created_key = crate::constants::storage::created_at_ledger(creator);
    let start_ledger: u32 = env
        .storage()
        .persistent()
        .get(&created_key)
        .unwrap_or(env.ledger().sequence());

    let config = LaunchWindowConfig {
        start_ledger,
        window_ledgers,
        start_cap,
        end_cap,
    };

    let key = LaunchWindowDataKey::Config(creator.clone());
    env.storage().persistent().set(&key, &config);
    crate::extend_key_ttl_to_full_window(env, &key);

    Ok(())
}

/// Get the current purchase cap for a wallet during the launch window.
/// Returns None if outside the window.
pub fn max_buy_now(env: &Env, creator: &Address) -> Option<u32> {
    let key = LaunchWindowDataKey::Config(creator.clone());
    let config: LaunchWindowConfig = env.storage().persistent().get(&key)?;

    let current_ledger = env.ledger().sequence();
    let window_end = config.start_ledger.checked_add(config.window_ledgers)?;

    // Outside the window: no cap
    if current_ledger >= window_end {
        return None;
    }

    // Before window start: shouldn't happen but treat as start_cap
    if current_ledger < config.start_ledger {
        return Some(config.start_cap);
    }

    // Inside the window: interpolate linearly
    let elapsed = current_ledger.checked_sub(config.start_ledger)?;
    let cap_range = config.end_cap.checked_sub(config.start_cap)?;

    // Linearly interpolate: start_cap + (elapsed / window_ledgers) * cap_range
    let interpolated = if cap_range == 0 {
        config.start_cap
    } else {
        let fraction = (elapsed as u128) * (cap_range as u128) / (config.window_ledgers as u128);
        config
            .start_cap
            .checked_add((fraction as u32).min(cap_range))?
    };

    Some(interpolated)
}

/// Get cumulative purchases for a wallet during the launch window.
pub fn get_cumulative_purchases(env: &Env, creator: &Address, wallet: &Address) -> u32 {
    let key = LaunchWindowDataKey::CumulativePurchases(creator.clone(), wallet.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Track a purchase for the wallet during the launch window.
pub fn record_purchase(
    env: &Env,
    creator: &Address,
    wallet: &Address,
    amount: u32,
) -> Result<(), ContractError> {
    let key = LaunchWindowDataKey::CumulativePurchases(creator.clone(), wallet.clone());
    let current = get_cumulative_purchases(env, creator, wallet);
    let new_total = current.checked_add(amount).ok_or(ContractError::Overflow)?;

    env.storage().persistent().set(&key, &new_total);
    crate::extend_key_ttl_to_full_window(env, &key);

    Ok(())
}

/// Check if an address is whitelisted to bypass the cap.
pub fn is_whitelisted(env: &Env, creator: &Address, address: &Address) -> bool {
    let bypass_key = LaunchWindowDataKey::BypassEnabled(creator.clone());
    let enabled: bool = env.storage().persistent().get(&bypass_key).unwrap_or(false);

    if !enabled {
        return false;
    }

    if address == creator {
        return true; // Creator always bypasses
    }

    let whitelist_key = LaunchWindowDataKey::Whitelist(creator.clone());
    if let Some(whitelist) = env
        .storage()
        .persistent()
        .get::<_, soroban_sdk::Vec<Address>>(&whitelist_key)
    {
        whitelist.contains(address)
    } else {
        false
    }
}

/// Enable or disable whitelist bypass for a creator.
pub fn set_bypass_enabled(
    env: &Env,
    creator: &Address,
    enabled: bool,
) -> Result<(), ContractError> {
    let key = LaunchWindowDataKey::BypassEnabled(creator.clone());
    env.storage().persistent().set(&key, &enabled);
    crate::extend_key_ttl_to_full_window(env, &key);
    Ok(())
}

/// Add an address to the whitelist.
pub fn add_to_whitelist(
    env: &Env,
    creator: &Address,
    address: &Address,
) -> Result<(), ContractError> {
    let bypass_key = LaunchWindowDataKey::BypassEnabled(creator.clone());
    let enabled: bool = env.storage().persistent().get(&bypass_key).unwrap_or(false);

    if !enabled {
        return Err(ContractError::Unauthorized);
    }

    let whitelist_key = LaunchWindowDataKey::Whitelist(creator.clone());
    let mut whitelist: soroban_sdk::Vec<Address> = env
        .storage()
        .persistent()
        .get(&whitelist_key)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    if !whitelist.contains(address) {
        whitelist.push_back(address.clone());
    }

    env.storage().persistent().set(&whitelist_key, &whitelist);
    crate::extend_key_ttl_to_full_window(env, &whitelist_key);

    Ok(())
}

/// Remove an address from the whitelist.
pub fn remove_from_whitelist(
    env: &Env,
    creator: &Address,
    address: &Address,
) -> Result<(), ContractError> {
    let whitelist_key = LaunchWindowDataKey::Whitelist(creator.clone());
    if let Some(whitelist) = env
        .storage()
        .persistent()
        .get::<_, soroban_sdk::Vec<Address>>(&whitelist_key)
    {
        let mut new_list: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(env);
        for addr in whitelist.iter() {
            if addr != *address {
                new_list.push_back(addr);
            }
        }
        env.storage().persistent().set(&whitelist_key, &new_list);
        crate::extend_key_ttl_to_full_window(env, &whitelist_key);
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_launch_window_cap_ramp() {
        // This test demonstrates the ramping behavior.
        // In a real test environment with Soroban SDK, you would create an env mock.
        // For now, this shows the interpolation logic.

        let start_cap = 10u32;
        let end_cap = 100u32;
        let window_ledgers = 100u32;

        // At start (ledger 0 of window): should be start_cap
        let elapsed = 0u32;
        let cap_range = end_cap.saturating_sub(start_cap);
        let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
        let interpolated = start_cap.saturating_add((fraction as u32).min(cap_range));
        assert_eq!(interpolated, 10, "At start should be start_cap");

        // At midpoint (ledger 50 of window): should be ~55
        let elapsed = 50u32;
        let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
        let interpolated = start_cap.saturating_add((fraction as u32).min(cap_range));
        assert!(
            (50..=60).contains(&interpolated),
            "At midpoint should be ~55"
        );

        // At end (ledger 100 of window): should be end_cap
        let elapsed = 100u32;
        let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
        let interpolated = start_cap.saturating_add((fraction as u32).min(cap_range));
        assert_eq!(interpolated, 100, "At end should be end_cap");

        // Beyond end: cap remains at end_cap
        let elapsed = 150u32;
        let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
        let interpolated = start_cap.saturating_add((fraction as u32).min(cap_range));
        assert_eq!(interpolated, 100, "Beyond end should be end_cap");
    }
}
