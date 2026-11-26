//! Multi-wallet key management through role-scoped co-creators (Issue #1132).
//!
//! The primary creator grants per-wallet roles. `Admin` may perform every
//! creator action, `Content` may only publish announcements, and `Revenue` may
//! only claim royalties / distribute revenue. Sensitive operations can
//! additionally require M-of-N approvals from the configured co-creators.

use crate::{
    events, extend_key_ttl_to_full_window, read_registered_creator_profile, Announcement,
    AnnouncementCategory, AnnouncementKey, ContractError, CreatorKeysContract,
    CreatorKeysContractArgs, CreatorKeysContractClient, FeatureError, FeatureKey,
};
use soroban_sdk::{contractimpl, contracttype, Address, Env, String, Vec};

/// Permission tiers assigned to a co-creator wallet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum CoCreatorRole {
    Admin = 0,
    Content = 1,
    Revenue = 2,
}

impl CoCreatorRole {
    /// Issue spelling aliases for the role variants.
    #[allow(non_upper_case_globals)]
    pub const ADMIN: Self = Self::Admin;
    #[allow(non_upper_case_globals)]
    pub const CONTENT: Self = Self::Content;
    #[allow(non_upper_case_globals)]
    pub const REVENUE: Self = Self::Revenue;
}

/// Creator operations subject to role-based authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum CreatorAction {
    Announcement = 0,
    Revenue = 1,
    All = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
enum CoCreatorKey {
    Role(Address, Address),
    Members(Address),
    Threshold(Address),
    Approvals(Address, u64, Address),
    NextAction(Address),
}

fn role_key(key_id: &Address, wallet: &Address) -> CoCreatorKey {
    CoCreatorKey::Role(key_id.clone(), wallet.clone())
}

fn members_key(key_id: &Address) -> CoCreatorKey {
    CoCreatorKey::Members(key_id.clone())
}

/// Removes a role grant and its membership entry, if present.
pub(crate) fn remove_role_entry(
    env: &Env,
    key_id: &Address,
    wallet: &Address,
) -> Option<CoCreatorRole> {
    let key = role_key(key_id, wallet);
    let role = env.storage().persistent().get::<_, CoCreatorRole>(&key)?;
    env.storage().persistent().remove(&key);
    let members_key = members_key(key_id);
    let mut members: Vec<Address> = env
        .storage()
        .persistent()
        .get(&members_key)
        .unwrap_or_else(|| Vec::new(env));
    if let Some(index) = members.first_index_of(wallet) {
        members.remove(index);
        save(env, members_key, members);
    }
    Some(role)
}

fn threshold_key(key_id: &Address) -> CoCreatorKey {
    CoCreatorKey::Threshold(key_id.clone())
}

fn save(env: &Env, key: CoCreatorKey, value: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.storage().persistent().set(&key, &value);
    extend_key_ttl_to_full_window(env, &key);
}

/// Returns the role granted to `wallet` for `key_id`, if any.
pub fn read_co_creator_role(
    env: &Env,
    key_id: &Address,
    wallet: &Address,
) -> Option<CoCreatorRole> {
    let key = role_key(key_id, wallet);
    env.storage().persistent().get::<_, CoCreatorRole>(&key)
}

/// Verifies that `caller` is the primary creator of `key_id`.
pub fn assert_primary_creator(
    env: &Env,
    key_id: &Address,
    caller: &Address,
) -> Result<(), FeatureError> {
    let profile =
        read_registered_creator_profile(env, key_id).map_err(|_| FeatureError::NotRegistered)?;
    if profile.creator != *caller {
        return Err(FeatureError::Unauthorized);
    }
    Ok(())
}

/// Verifies that `caller` holds a role that permits `action`.
///
/// The primary creator always passes. An `Admin` co-creator passes every
/// action; `Content` only announcements; `Revenue` only revenue operations.
pub fn assert_co_creator_can(
    env: &Env,
    key_id: &Address,
    caller: &Address,
    action: CreatorAction,
) -> Result<(), FeatureError> {
    if read_registered_creator_profile(env, key_id)
        .map(|p| p.creator == *caller)
        .unwrap_or(false)
    {
        return Ok(());
    }
    let role = read_co_creator_role(env, key_id, caller).ok_or(FeatureError::Unauthorized)?;
    let allowed = matches!(
        (role, action),
        (CoCreatorRole::Admin, _)
            | (CoCreatorRole::Content, CreatorAction::Announcement)
            | (CoCreatorRole::Revenue, CreatorAction::Revenue)
    );
    if !allowed {
        return Err(FeatureError::Unauthorized);
    }
    Ok(())
}

#[contractimpl]
impl CreatorKeysContract {
    /// Grants `role` to `wallet` for `key_id`. Only the primary creator may call.
    pub fn add_co_creator(
        env: Env,
        key_id: Address,
        caller: Address,
        wallet: Address,
        role: CoCreatorRole,
    ) -> Result<(), FeatureError> {
        caller.require_auth();
        assert_primary_creator(&env, &key_id, &caller)?;
        if wallet == key_id {
            return Err(FeatureError::Unauthorized);
        }
        save(&env, role_key(&key_id, &wallet), role);
        let members_key = members_key(&key_id);
        let mut members: Vec<Address> = env
            .storage()
            .persistent()
            .get(&members_key)
            .unwrap_or_else(|| Vec::new(&env));
        if !members.contains(&wallet) {
            members.push_back(wallet.clone());
            save(&env, members_key, members);
        }
        env.events().publish(
            events::co_creator_added_topics(&key_id, &wallet),
            events::CoCreatorAddedEvent {
                key_id: key_id.clone(),
                wallet: wallet.clone(),
                role,
                ledger: env.ledger().sequence(),
            },
        );
        Ok(())
    }

    /// Revokes all roles held by `wallet` for `key_id`.
    pub fn remove_co_creator_role(
        env: Env,
        key_id: Address,
        caller: Address,
        wallet: Address,
    ) -> Result<(), FeatureError> {
        caller.require_auth();
        assert_primary_creator(&env, &key_id, &caller)?;
        let role = remove_role_entry(&env, &key_id, &wallet).ok_or(FeatureError::NoCoCreatorSet)?;
        env.events().publish(
            events::co_creator_removed_topics(&key_id, &wallet),
            events::CoCreatorRemovedEvent {
                creator_id: key_id.clone(),
                co_creator: wallet.clone(),
            },
        );
        env.events().publish(
            events::co_creator_role_removed_topics(&key_id, &wallet),
            events::CoCreatorRoleRemovedEvent {
                key_id: key_id.clone(),
                wallet,
                role,
                ledger: env.ledger().sequence(),
            },
        );
        Ok(())
    }

    /// Returns the role of `wallet` for `key_id`, if any.
    pub fn get_co_creator_role(
        env: Env,
        key_id: Address,
        wallet: Address,
    ) -> Option<CoCreatorRole> {
        read_co_creator_role(&env, &key_id, &wallet)
    }

    /// Returns all co-creator wallets configured for `key_id`.
    pub fn get_co_creators(env: Env, key_id: Address) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&members_key(&key_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Configures the number of distinct approvals required for sensitive
    /// co-creator actions. `0` disables the M-of-N requirement.
    pub fn set_co_creator_threshold(
        env: Env,
        key_id: Address,
        caller: Address,
        threshold: u32,
    ) -> Result<(), FeatureError> {
        caller.require_auth();
        assert_primary_creator(&env, &key_id, &caller)?;
        let members: Vec<Address> = env
            .storage()
            .persistent()
            .get(&members_key(&key_id))
            .unwrap_or_else(|| Vec::new(&env));
        if threshold > members.len() {
            return Err(FeatureError::Unauthorized);
        }
        save(&env, threshold_key(&key_id), threshold);
        Ok(())
    }

    pub fn get_co_creator_threshold(env: Env, key_id: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&threshold_key(&key_id))
            .unwrap_or(0)
    }

    /// Records an approval for a pending sensitive action and returns its id.
    pub fn approve_co_creator_action(
        env: Env,
        key_id: Address,
        approver: Address,
    ) -> Result<u64, FeatureError> {
        approver.require_auth();
        read_co_creator_role(&env, &key_id, &approver).ok_or(FeatureError::Unauthorized)?;
        let action_key = CoCreatorKey::NextAction(key_id.clone());
        let action_id: u64 = env.storage().persistent().get(&action_key).unwrap_or(0);
        save(
            &env,
            CoCreatorKey::Approvals(key_id.clone(), action_id, approver.clone()),
            true,
        );
        Ok(action_id)
    }

    /// Returns the number of distinct co-creator approvals for `action_id`.
    pub fn get_co_creator_approval_count(env: Env, key_id: Address, action_id: u64) -> u32 {
        let members: Vec<Address> = env
            .storage()
            .persistent()
            .get(&members_key(&key_id))
            .unwrap_or_else(|| Vec::new(&env));
        let mut count = 0u32;
        for wallet in members.iter() {
            let key = CoCreatorKey::Approvals(key_id.clone(), action_id, wallet);
            if env
                .storage()
                .persistent()
                .get::<_, bool>(&key)
                .unwrap_or(false)
            {
                count += 1;
            }
        }
        count
    }

    /// Posts an announcement on behalf of a role-authorized co-creator.
    pub fn post_announcement_as_co_creator(
        env: Env,
        key_id: Address,
        caller: Address,
        title: String,
        content_hash: String,
        category: AnnouncementCategory,
    ) -> Result<u32, ContractError> {
        caller.require_auth();
        assert_co_creator_can(&env, &key_id, &caller, CreatorAction::Announcement)
            .map_err(|_| ContractError::Unauthorized)?;
        read_registered_creator_profile(&env, &key_id)?;

        let count_key = AnnouncementKey::Count(key_id.clone());
        let announcement_id: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);
        let next_count = announcement_id
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;
        let announcement = Announcement {
            announcement_id,
            key_id: key_id.clone(),
            title,
            content_hash,
            category,
            posted_at: env.ledger().timestamp(),
        };
        let record_key = AnnouncementKey::Record(key_id.clone(), announcement_id);
        env.storage().persistent().set(&record_key, &announcement);
        extend_key_ttl_to_full_window(&env, &record_key);
        env.storage().persistent().set(&count_key, &next_count);
        extend_key_ttl_to_full_window(&env, &count_key);
        env.events().publish(
            events::announcement_posted_topics(&key_id, announcement_id),
            events::AnnouncementPostedEvent {
                key_id,
                announcement_id,
                title: announcement.title.clone(),
                content_hash: announcement.content_hash.clone(),
                category: announcement.category,
                posted_at: announcement.posted_at,
            },
        );
        Ok(announcement_id)
    }

    /// Claims accrued royalties on behalf of a role-authorized co-creator.
    pub fn claim_royalties_as_co_creator(
        env: Env,
        key_id: Address,
        caller: Address,
    ) -> Result<i128, ContractError> {
        caller.require_auth();
        assert_co_creator_can(&env, &key_id, &caller, CreatorAction::Revenue)
            .map_err(|_| ContractError::Unauthorized)?;
        read_registered_creator_profile(&env, &key_id)?;
        let key = FeatureKey::RoyaltyBalance(key_id.clone());
        let amount: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if amount == 0 {
            return Err(ContractError::NoDividendClaimable);
        }
        env.storage().persistent().remove(&key);
        Ok(amount)
    }

    /// Returns whether `caller` may perform `action` for `key_id`.
    pub fn can_co_creator_perform(
        env: Env,
        key_id: Address,
        caller: Address,
        action: CreatorAction,
    ) -> bool {
        assert_co_creator_can(&env, &key_id, &caller, action).is_ok()
    }
}

/// Kept public so other feature modules can reuse the role gate when adding
/// new creator-scoped entrypoints.
pub use assert_co_creator_can as assert_creator_action_allowed;

/// Re-export for clients that import the role types from the crate root.
pub use CoCreatorRole as CreatorRole;
