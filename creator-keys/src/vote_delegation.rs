//! Creator key vote delegation for proxy voting rights (Issue #1116).
//!
//! Enables creator key holders to delegate their governance voting weight to another
//! wallet without transferring ownership of the underlying keys.

use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::{
    constants,
    events::{publish_delegation_revoked, publish_voting_delegated},
    extend_key_ttl_to_full_window, read_registered_creator_profile, ContractError,
};

/// Storage keys for creator key vote delegation (Issue #1116).
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum VoteDelegationKey {
    /// (key_id, holder) -> Address (the delegate wallet)
    Delegate(Address, Address),
    /// (key_id, delegate) -> Vec<Address> (list of delegator wallets)
    Delegators(Address, Address),
}

/// Resolves `(key_id, holder)` regardless of whether caller passed `(holder, key_id)` or `(key_id, holder)`.
pub fn resolve_key_and_wallet(env: &Env, arg1: &Address, arg2: &Address) -> (Address, Address) {
    if read_registered_creator_profile(env, arg1).is_ok()
        && read_registered_creator_profile(env, arg2).is_err()
    {
        (arg1.clone(), arg2.clone())
    } else {
        (arg2.clone(), arg1.clone())
    }
}

/// Returns the liquid key balance of `wallet` for `key_id`.
pub fn read_holder_balance(env: &Env, key_id: &Address, wallet: &Address) -> u32 {
    let key = constants::storage::holder_balance_key(key_id, wallet);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Checks whether `wallet` currently has an active vote delegation for `key_id`.
pub fn has_active_delegation(env: &Env, key_id: &Address, wallet: &Address) -> bool {
    let key = VoteDelegationKey::Delegate(key_id.clone(), wallet.clone());
    env.storage().persistent().has(&key)
}

/// Returns the delegate for `holder` on `key_id`, if one exists.
pub fn get_vote_delegate(env: &Env, holder: &Address, key_id: &Address) -> Option<Address> {
    let (key_id, holder) = resolve_key_and_wallet(env, holder, key_id);
    let key = VoteDelegationKey::Delegate(key_id, holder);
    env.storage().persistent().get(&key)
}

/// Returns all delegators who have delegated their voting weight to `delegate` on `key_id`.
pub fn get_vote_delegators(env: &Env, delegate: &Address, key_id: &Address) -> Vec<Address> {
    let (key_id, delegate) = resolve_key_and_wallet(env, delegate, key_id);
    let key = VoteDelegationKey::Delegators(key_id, delegate);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env))
}

/// Assigns the voting weight of `key_id` held by `holder` to `delegate_wallet`.
pub fn delegate_votes(
    env: &Env,
    arg1: &Address,
    arg2: &Address,
    delegate_wallet: &Address,
) -> Result<(), ContractError> {
    let (key_id, holder) = resolve_key_and_wallet(env, arg1, arg2);

    holder.require_auth();

    // Key must be a registered creator
    read_registered_creator_profile(env, &key_id)?;

    // Reject self-delegation
    if holder == *delegate_wallet {
        return Err(ContractError::SelfDelegation);
    }

    // Holder must currently hold keys for this key_id
    let balance = read_holder_balance(env, &key_id, &holder);
    if balance == 0 {
        return Err(ContractError::NotAHolder);
    }

    // Reject delegating an already-delegated key (must revoke first)
    let delegate_key = VoteDelegationKey::Delegate(key_id.clone(), holder.clone());
    if env.storage().persistent().has(&delegate_key) {
        return Err(ContractError::AlreadyDelegated);
    }

    // Delegation chain limit (1 level):
    // 1. A wallet that has received delegated weight cannot delegate
    let holder_delegators_key = VoteDelegationKey::Delegators(key_id.clone(), holder.clone());
    if let Some(delegators) = env
        .storage()
        .persistent()
        .get::<_, Vec<Address>>(&holder_delegators_key)
    {
        if !delegators.is_empty() {
            return Err(ContractError::CannotReDelegate);
        }
    }

    // 2. A wallet that has delegated cannot receive delegated weight
    let target_delegate_key = VoteDelegationKey::Delegate(key_id.clone(), delegate_wallet.clone());
    if env.storage().persistent().has(&target_delegate_key) {
        return Err(ContractError::CannotReDelegate);
    }

    // Persist delegation
    env.storage()
        .persistent()
        .set(&delegate_key, delegate_wallet);
    extend_key_ttl_to_full_window(env, &delegate_key);

    // Update delegate's delegators list
    let target_delegators_key =
        VoteDelegationKey::Delegators(key_id.clone(), delegate_wallet.clone());
    let mut delegators = env
        .storage()
        .persistent()
        .get::<_, Vec<Address>>(&target_delegators_key)
        .unwrap_or_else(|| Vec::new(env));
    delegators.push_back(holder.clone());
    env.storage()
        .persistent()
        .set(&target_delegators_key, &delegators);
    extend_key_ttl_to_full_window(env, &target_delegators_key);

    // Emit VotingDelegated event
    publish_voting_delegated(env, &holder, delegate_wallet, &key_id, balance);

    Ok(())
}

/// Revokes a vote delegation and returns voting weight to `holder`.
pub fn revoke_delegation(env: &Env, arg1: &Address, arg2: &Address) -> Result<(), ContractError> {
    let (key_id, holder) = resolve_key_and_wallet(env, arg1, arg2);

    holder.require_auth();

    read_registered_creator_profile(env, &key_id)?;

    let delegate_key = VoteDelegationKey::Delegate(key_id.clone(), holder.clone());
    let previous_delegate: Address = match env.storage().persistent().get(&delegate_key) {
        Some(d) => d,
        None => return Err(ContractError::NoDelegation),
    };

    let balance = read_holder_balance(env, &key_id, &holder);

    // Remove delegation
    env.storage().persistent().remove(&delegate_key);

    // Update previous delegate's delegators list
    let delegators_key = VoteDelegationKey::Delegators(key_id.clone(), previous_delegate.clone());
    if let Some(delegators) = env
        .storage()
        .persistent()
        .get::<_, Vec<Address>>(&delegators_key)
    {
        let mut new_delegators = Vec::new(env);
        for d in delegators.iter() {
            if d != holder {
                new_delegators.push_back(d);
            }
        }
        if new_delegators.is_empty() {
            env.storage().persistent().remove(&delegators_key);
        } else {
            env.storage()
                .persistent()
                .set(&delegators_key, &new_delegators);
            extend_key_ttl_to_full_window(env, &delegators_key);
        }
    }

    // Emit DelegationRevoked event
    publish_delegation_revoked(env, &holder, &previous_delegate, &key_id, balance);

    Ok(())
}

/// Returns the wallet's own weight plus any weight delegated to it for `key_id`.
pub fn get_voting_weight(env: &Env, arg1: &Address, arg2: &Address) -> u32 {
    let (key_id, wallet) = resolve_key_and_wallet(env, arg1, arg2);

    // A delegating holder must not also count their own weight (returns 0)
    if has_active_delegation(env, &key_id, &wallet) {
        return 0;
    }

    // Check old delegate storage as well for compatibility
    let old_delegate_key = constants::storage::delegate(&key_id, &wallet);
    if env.storage().persistent().has(&old_delegate_key) {
        return 0;
    }

    let own_balance = read_holder_balance(env, &key_id, &wallet);
    let mut total_weight = own_balance;

    let delegators_key = VoteDelegationKey::Delegators(key_id.clone(), wallet.clone());
    if let Some(delegators) = env
        .storage()
        .persistent()
        .get::<_, Vec<Address>>(&delegators_key)
    {
        for d in delegators.iter() {
            let d_bal = read_holder_balance(env, &key_id, &d);
            total_weight = total_weight
                .checked_add(d_bal)
                .expect("voting weight overflow");
        }
    }

    total_weight
}

/// Automatically prunes a delegation if `holder`'s key balance has dropped to 0.
pub fn prune_if_zero_balance(env: &Env, key_id: &Address, holder: &Address) {
    let balance = read_holder_balance(env, key_id, holder);
    if balance == 0 {
        let delegate_key = VoteDelegationKey::Delegate(key_id.clone(), holder.clone());
        if let Some(delegate_wallet) = env.storage().persistent().get::<_, Address>(&delegate_key) {
            env.storage().persistent().remove(&delegate_key);
            let delegators_key =
                VoteDelegationKey::Delegators(key_id.clone(), delegate_wallet.clone());
            if let Some(delegators) = env
                .storage()
                .persistent()
                .get::<_, Vec<Address>>(&delegators_key)
            {
                let mut new_delegators = Vec::new(env);
                for d in delegators.iter() {
                    if d != *holder {
                        new_delegators.push_back(d);
                    }
                }
                if new_delegators.is_empty() {
                    env.storage().persistent().remove(&delegators_key);
                } else {
                    env.storage()
                        .persistent()
                        .set(&delegators_key, &new_delegators);
                    extend_key_ttl_to_full_window(env, &delegators_key);
                }
            }
        }
    }
}
