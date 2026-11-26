//! Badge-gated access control for buys, claims and creator actions.
//!
//! Issue #1286: Some actions should be limited to wallets holding a particular badge, such as
//! verified investors or event attendees, but access control only knows addresses and balances.
//! This module lets keys and features require soulbound badges.

use soroban_sdk::{contractclient, contracttype, Address, Env, Vec};

use crate::ContractError;

/// Actions that can be gated by badges.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[contracttype]
pub enum BadgeGatedAction {
    Buy = 1,
    Claim = 2,
    Vote = 3,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum BadgeAccessControlKey {
    /// (key_id, action) -> Vec<Address> of accepted badge contract addresses
    BadgeRequirement(Address, BadgeGatedAction),
    /// (key_id) -> badge contract address (if single badge system)
    KeyBadgeContract(Address),
}

/// Badge contract interface for checking badge ownership.
#[contractclient(name = "BadgeContractClient")]
pub trait BadgeContract {
    /// Check if a wallet has a specific badge.
    fn has_badge(env: Env, wallet: Address, badge_id: u64) -> bool;
}

/// Set a badge requirement for a specific action on a key.
pub fn set_badge_requirement(
    env: &Env,
    creator: Address,
    key_id: Address,
    action: BadgeGatedAction,
    badge_contract: Address,
) -> Result<(), ContractError> {
    creator.require_auth();

    // Note: In real implementation, would verify creator owns the key

    // Get existing requirements
    let req_key = BadgeAccessControlKey::BadgeRequirement(key_id.clone(), action);
    let mut badges: Vec<Address> = env
        .storage()
        .persistent()
        .get(&req_key)
        .unwrap_or(Vec::new(env));

    // Add badge contract if not already present
    let mut found = false;
    for i in 0..badges.len() {
        if let Some(addr) = badges.get(i) {
            if addr == badge_contract {
                found = true;
                break;
            }
        }
    }

    if !found {
        badges.push_back(badge_contract.clone());
    }

    // Store updated requirements
    env.storage().persistent().set(&req_key, &badges);

    // Emit event
    env.events().publish(
        (soroban_sdk::symbol_short!("badge_set"),),
        (key_id, action as u32, badge_contract),
    );

    Ok(())
}

/// Clear a badge requirement for a specific action.
pub fn clear_badge_requirement(
    env: &Env,
    creator: Address,
    key_id: Address,
    action: BadgeGatedAction,
) -> Result<(), ContractError> {
    creator.require_auth();

    // Note: In real implementation, would verify creator owns the key

    // Remove requirement
    let req_key = BadgeAccessControlKey::BadgeRequirement(key_id.clone(), action);
    env.storage().persistent().remove(&req_key);

    // Emit event
    env.events().publish(
        (soroban_sdk::symbol_short!("badge_clr"),),
        (key_id, action as u32),
    );

    Ok(())
}

/// Get badge requirements for a specific action.
pub fn badge_requirement_of(env: &Env, key_id: Address, action: BadgeGatedAction) -> Vec<Address> {
    let req_key = BadgeAccessControlKey::BadgeRequirement(key_id, action);
    env.storage()
        .persistent()
        .get(&req_key)
        .unwrap_or(Vec::new(env))
}

/// Check if a wallet has the required badge for an action.
/// This function implements "any-of" semantics - wallet needs at least one of the accepted badges.
pub fn require_badge(
    env: &Env,
    wallet: Address,
    key_id: Address,
    action: BadgeGatedAction,
) -> Result<(), ContractError> {
    // Get badge requirements
    let required_badges = badge_requirement_of(env, key_id, action);

    // If no requirements, allow access
    if required_badges.is_empty() {
        return Ok(());
    }

    // Check if wallet has any of the required badges
    // For simplicity, we'll use badge_id = 1 as a default
    // In a real implementation, the badge_id would be stored with the requirement
    let badge_id: u64 = 1;

    for i in 0..required_badges.len() {
        if let Some(badge_contract) = required_badges.get(i) {
            let client = BadgeContractClient::new(env, &badge_contract);
            let has_badge = client.has_badge(&wallet, &badge_id);

            if has_badge {
                // Wallet has at least one accepted badge
                return Ok(());
            }
        }
    }

    // Wallet doesn't have any accepted badge
    Err(ContractError::WhitelistOnly)
}

/// Check if a badge has been revoked for a wallet.
/// Note: This relies on the badge contract's has_badge returning false for revoked badges.
pub fn check_badge_revocation(
    env: &Env,
    wallet: Address,
    badge_contract: Address,
    badge_id: u64,
) -> bool {
    let client = BadgeContractClient::new(env, &badge_contract);
    !client.has_badge(&wallet, &badge_id)
}

/// Add multiple accepted badge contracts for an action (any-of semantics).
pub fn add_badge_contracts(
    env: &Env,
    creator: Address,
    key_id: Address,
    action: BadgeGatedAction,
    badge_contracts: Vec<Address>,
) -> Result<(), ContractError> {
    creator.require_auth();

    // Get existing requirements
    let req_key = BadgeAccessControlKey::BadgeRequirement(key_id.clone(), action);
    let mut badges: Vec<Address> = env
        .storage()
        .persistent()
        .get(&req_key)
        .unwrap_or(Vec::new(env));

    // Add each badge contract if not already present
    for i in 0..badge_contracts.len() {
        if let Some(badge_contract) = badge_contracts.get(i) {
            let mut found = false;
            for j in 0..badges.len() {
                if let Some(addr) = badges.get(j) {
                    if addr == badge_contract {
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                badges.push_back(badge_contract.clone());
            }
        }
    }

    // Store updated requirements
    env.storage().persistent().set(&req_key, &badges);

    // Emit event for each added badge
    for i in 0..badge_contracts.len() {
        if let Some(badge_contract) = badge_contracts.get(i) {
            env.events().publish(
                (soroban_sdk::symbol_short!("badge_add"),),
                (key_id.clone(), action as u32, badge_contract),
            );
        }
    }

    Ok(())
}

/// Remove a specific badge contract from requirements.
pub fn remove_badge_contract(
    env: &Env,
    creator: Address,
    key_id: Address,
    action: BadgeGatedAction,
    badge_contract: Address,
) -> Result<(), ContractError> {
    creator.require_auth();

    // Get existing requirements
    let req_key = BadgeAccessControlKey::BadgeRequirement(key_id.clone(), action);
    let badges: Vec<Address> = env
        .storage()
        .persistent()
        .get(&req_key)
        .unwrap_or(Vec::new(env));

    // Create new list without the specified badge contract
    let mut new_badges = Vec::new(env);
    for i in 0..badges.len() {
        if let Some(addr) = badges.get(i) {
            if addr != badge_contract {
                new_badges.push_back(addr);
            }
        }
    }

    // Store updated requirements
    if new_badges.is_empty() {
        env.storage().persistent().remove(&req_key);
    } else {
        env.storage().persistent().set(&req_key, &new_badges);
    }

    // Emit event
    env.events().publish(
        (soroban_sdk::symbol_short!("badge_rmv"),),
        (key_id, action as u32, badge_contract),
    );

    Ok(())
}

/// Check if a key has any badge requirements for a specific action.
pub fn has_badge_requirement(env: &Env, key_id: Address, action: BadgeGatedAction) -> bool {
    let required_badges = badge_requirement_of(env, key_id, action);
    !required_badges.is_empty()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_badge_gated_action_values() {
        assert_eq!(BadgeGatedAction::Buy as u32, 1);
        assert_eq!(BadgeGatedAction::Claim as u32, 2);
        assert_eq!(BadgeGatedAction::Vote as u32, 3);
    }
}
