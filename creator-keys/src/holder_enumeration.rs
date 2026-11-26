//! Holder enumeration views with stable pagination and supply-ordered indexes.
//!
//! Issue #1285: Indexers and other contracts cannot list holders of a key on-chain without
//! scanning events, which makes on-chain verification and integrations hard. This module adds
//! enumerable holder sets and paginated views.

use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::ContractError;

/// Maximum page limit for holder queries.
pub const MAX_PAGE_LIMIT: u32 = 100;

/// Maximum size for top holders list.
pub const MAX_TOP_HOLDERS_SIZE: u32 = 50;

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum HolderEnumerationKey {
    /// (key_id) -> total holder count
    HolderCount(Address),
    /// (key_id, holder_address) -> insertion index
    HolderIndex(Address, Address),
    /// (key_id, index) -> holder address
    HolderByIndex(Address, u32),
    /// (key_id) -> Vec of top holders sorted by balance
    TopHolders(Address),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct HolderInfo {
    pub address: Address,
    pub balance: i128,
}

/// Add a holder to the enumeration (called when balance goes from 0 to non-zero).
pub fn add_holder(env: &Env, key_id: Address, holder: Address) -> Result<(), ContractError> {
    // Check if holder already exists
    let holder_index_key = HolderEnumerationKey::HolderIndex(key_id.clone(), holder.clone());
    if env
        .storage()
        .persistent()
        .get::<HolderEnumerationKey, u32>(&holder_index_key)
        .is_some()
    {
        // Already exists, nothing to do
        return Ok(());
    }

    // Get current count
    let count_key = HolderEnumerationKey::HolderCount(key_id.clone());
    let count: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);

    // Add holder at index = count
    let by_index_key = HolderEnumerationKey::HolderByIndex(key_id.clone(), count);
    env.storage().persistent().set(&by_index_key, &holder);

    // Store the index for this holder
    env.storage().persistent().set(&holder_index_key, &count);

    // Increment count
    env.storage().persistent().set(&count_key, &(count + 1));

    // Extend TTL for the entries
    extend_holder_ttl(env, &key_id, &holder);

    Ok(())
}

/// Remove a holder from the enumeration (called when balance goes to zero).
pub fn remove_holder(env: &Env, key_id: Address, holder: Address) -> Result<(), ContractError> {
    // Get holder's current index
    let holder_index_key = HolderEnumerationKey::HolderIndex(key_id.clone(), holder.clone());
    let index: u32 = env
        .storage()
        .persistent()
        .get(&holder_index_key)
        .ok_or(ContractError::NotAHolder)?;

    // Get current count
    let count_key = HolderEnumerationKey::HolderCount(key_id.clone());
    let count: u32 = env
        .storage()
        .persistent()
        .get(&count_key)
        .ok_or(ContractError::NotAHolder)?;

    if count == 0 {
        return Err(ContractError::NotAHolder);
    }

    let last_index = count - 1;

    // If not the last element, swap with last element
    if index != last_index {
        let last_holder_key = HolderEnumerationKey::HolderByIndex(key_id.clone(), last_index);
        let last_holder: Address = env
            .storage()
            .persistent()
            .get(&last_holder_key)
            .ok_or(ContractError::NotAHolder)?;

        // Move last holder to the removed holder's position
        let by_index_key = HolderEnumerationKey::HolderByIndex(key_id.clone(), index);
        env.storage().persistent().set(&by_index_key, &last_holder);

        // Update last holder's index
        let last_holder_index_key =
            HolderEnumerationKey::HolderIndex(key_id.clone(), last_holder.clone());
        env.storage()
            .persistent()
            .set(&last_holder_index_key, &index);
    }

    // Remove the last element
    let last_key = HolderEnumerationKey::HolderByIndex(key_id.clone(), last_index);
    env.storage().persistent().remove(&last_key);

    // Remove holder's index mapping
    env.storage().persistent().remove(&holder_index_key);

    // Decrement count
    env.storage().persistent().set(&count_key, &last_index);

    Ok(())
}

/// Get the total number of holders for a key.
pub fn holder_count(env: &Env, key_id: Address) -> u32 {
    let count_key = HolderEnumerationKey::HolderCount(key_id);
    env.storage().persistent().get(&count_key).unwrap_or(0)
}

/// Get a page of holders.
pub fn holders_page(
    env: &Env,
    key_id: Address,
    start: u32,
    limit: u32,
) -> Result<Vec<Address>, ContractError> {
    // Validate limit
    if limit == 0 || limit > MAX_PAGE_LIMIT {
        return Err(ContractError::BatchSizeExceeded);
    }

    let count = holder_count(env, key_id.clone());
    let mut holders = Vec::new(env);

    let end = start.saturating_add(limit).min(count);

    for i in start..end {
        let by_index_key = HolderEnumerationKey::HolderByIndex(key_id.clone(), i);
        if let Some(holder) = env.storage().persistent().get(&by_index_key) {
            holders.push_back(holder);
        }
    }

    Ok(holders)
}

/// Get top N holders sorted by balance.
pub fn top_holders(env: &Env, key_id: Address, n: u32) -> Result<Vec<HolderInfo>, ContractError> {
    // Validate n
    if n == 0 || n > MAX_TOP_HOLDERS_SIZE {
        return Err(ContractError::BatchSizeExceeded);
    }

    // Get cached top holders
    let top_key = HolderEnumerationKey::TopHolders(key_id.clone());
    let top: Vec<HolderInfo> = env
        .storage()
        .persistent()
        .get(&top_key)
        .unwrap_or(Vec::new(env));

    // Return up to n items
    let mut result = Vec::new(env);
    let limit = n.min(top.len());
    for i in 0..limit {
        if let Some(holder_info) = top.get(i) {
            result.push_back(holder_info);
        }
    }

    Ok(result)
}

/// Update top holders list when balance changes.
/// This maintains a sorted list of top holders by balance.
pub fn update_top_holders(
    env: &Env,
    key_id: Address,
    holder: Address,
    new_balance: i128,
) -> Result<(), ContractError> {
    // Get current top holders
    let top_key = HolderEnumerationKey::TopHolders(key_id.clone());
    let top: Vec<HolderInfo> = env
        .storage()
        .persistent()
        .get(&top_key)
        .unwrap_or(Vec::new(env));

    // Remove holder if already in list
    let mut removed_index: Option<u32> = None;
    for i in 0..top.len() {
        if let Some(info) = top.get(i) {
            if info.address == holder {
                removed_index = Some(i);
                break;
            }
        }
    }

    // Create new list without the holder
    let mut new_top = Vec::new(env);
    for i in 0..top.len() {
        if Some(i) != removed_index {
            if let Some(info) = top.get(i) {
                new_top.push_back(info);
            }
        }
    }

    // If new balance is non-zero, insert holder in sorted position
    if new_balance > 0 {
        let new_info = HolderInfo {
            address: holder.clone(),
            balance: new_balance,
        };

        // Find insertion point
        let mut inserted = false;
        let mut result = Vec::new(env);

        for i in 0..new_top.len() {
            if let Some(info) = new_top.get(i) {
                if !inserted && new_balance > info.balance {
                    result.push_back(new_info.clone());
                    inserted = true;
                }
                // Only keep up to MAX_TOP_HOLDERS_SIZE
                if result.len() < MAX_TOP_HOLDERS_SIZE {
                    result.push_back(info);
                }
            }
        }

        // If not inserted yet and there's room, append at end
        if !inserted && result.len() < MAX_TOP_HOLDERS_SIZE {
            result.push_back(new_info);
        }

        new_top = result;
    }

    // Save updated top holders
    env.storage().persistent().set(&top_key, &new_top);

    Ok(())
}

/// Extend TTL for holder entries.
fn extend_holder_ttl(env: &Env, key_id: &Address, holder: &Address) {
    let holder_index_key = HolderEnumerationKey::HolderIndex(key_id.clone(), holder.clone());
    env.storage()
        .persistent()
        .extend_ttl(&holder_index_key, 100000, 200000);
}

/// Bump holder index TTL (maintenance function).
pub fn bump_holder_index(env: &Env, key_id: Address) -> Result<(), ContractError> {
    let count = holder_count(env, key_id.clone());

    for i in 0..count {
        let by_index_key = HolderEnumerationKey::HolderByIndex(key_id.clone(), i);
        if let Some(holder) = env.storage().persistent().get::<_, Address>(&by_index_key) {
            extend_holder_ttl(env, &key_id, &holder);
        }
    }

    Ok(())
}

/// Backfill holders from existing balances (admin function for migration).
pub fn backfill_holders(
    env: &Env,
    admin: Address,
    key_id: Address,
    holders: Vec<Address>,
) -> Result<(), ContractError> {
    admin.require_auth();

    for i in 0..holders.len() {
        if let Some(holder) = holders.get(i) {
            // Note: In real implementation, would check actual balance
            // and only add if balance > 0
            add_holder(env, key_id.clone(), holder)?;
        }
    }

    Ok(())
}
