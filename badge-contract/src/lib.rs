#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol, Vec};

#[cfg(test)]
mod test;

/// Event name for badge minting.
pub const BADGE_MINTED_EVENT_NAME: &str = "BadgeMinted";

/// Default persistent storage TTL thresholds (~30 days).
pub const BADGE_TTL_THRESHOLD: u32 = 100_000;
pub const BADGE_TTL_EXTEND_TO: u32 = 518_400;

/// Error codes returned by the badge contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BadgeError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    BadgeAlreadyEarned = 4,
    ContractAlreadyAuthorized = 5,
    ContractNotAuthorized = 6,
    ArithmeticOverflow = 7,
    NoAuthorisedContract = 8,
}

pub type ContractError = BadgeError;

/// Supported badge types for key platform milestone achievements.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BadgeType {
    FirstBuyer = 1,
    WhaleHolder = 2,
    GovernanceVoter = 3,
    LongTermHolder = 4,
    TopTrader = 5,
}

impl BadgeType {
    pub const FIRST_BUYER: BadgeType = BadgeType::FirstBuyer;
    pub const WHALE_HOLDER: BadgeType = BadgeType::WhaleHolder;
    pub const GOVERNANCE_VOTER: BadgeType = BadgeType::GovernanceVoter;
    pub const LONG_TERM_HOLDER: BadgeType = BadgeType::LongTermHolder;
    pub const TOP_TRADER: BadgeType = BadgeType::TopTrader;

    pub fn all() -> [BadgeType; 5] {
        [
            BadgeType::FirstBuyer,
            BadgeType::WhaleHolder,
            BadgeType::GovernanceVoter,
            BadgeType::LongTermHolder,
            BadgeType::TopTrader,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            BadgeType::FirstBuyer => "FIRST_BUYER",
            BadgeType::WhaleHolder => "WHALE_HOLDER",
            BadgeType::GovernanceVoter => "GOVERNANCE_VOTER",
            BadgeType::LongTermHolder => "LONG_TERM_HOLDER",
            BadgeType::TopTrader => "TOP_TRADER",
        }
    }
}

/// An earned badge record containing the badge type and mint timestamp.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BadgeRecord {
    pub badge_type: BadgeType,
    pub mint_timestamp: u64,
}

impl BadgeRecord {
    pub fn timestamp(&self) -> u64 {
        self.mint_timestamp
    }
}

/// Event payload emitted when a badge is minted.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BadgeMinted {
    pub wallet: Address,
    pub badge_type: BadgeType,
}

pub type BadgeMintedEvent = BadgeMinted;

/// Persistent and instance storage keys.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    PrimaryAuthorisedContract,
    Authorised(Address),
    AuthorisedList,
    Badge(Address, BadgeType),
    WalletBadges(Address),
    TotalBadgesMinted,
}

#[contract]
pub struct BadgeContract;

pub type KeyNftBadgeContract = BadgeContract;
pub type NftBadgeContract = BadgeContract;

fn has_badge_internal(env: &Env, wallet: &Address, badge_type: BadgeType) -> bool {
    let key = DataKey::Badge(wallet.clone(), badge_type);
    env.storage().persistent().has(&key)
}

fn is_authorised_internal(env: &Env, contract: &Address) -> bool {
    // Check primary authorised contract
    if let Some(primary) = env
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::PrimaryAuthorisedContract)
    {
        if primary == *contract {
            return true;
        }
    }
    // Check authorised set
    env.storage()
        .instance()
        .get::<_, bool>(&DataKey::Authorised(contract.clone()))
        .unwrap_or(false)
}

fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(BADGE_TTL_THRESHOLD, BADGE_TTL_EXTEND_TO);
}

#[contractimpl]
impl BadgeContract {
    /// Initializes the badge contract with an administrator.
    pub fn init(env: Env, admin: Address) -> Result<(), BadgeError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(BadgeError::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::AuthorisedList, &Vec::<Address>::new(&env));
        env.storage()
            .instance()
            .set(&DataKey::TotalBadgesMinted, &0u64);
        extend_instance_ttl(&env);

        Ok(())
    }

    /// Returns the contract admin address, if initialized.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    /// Updates the admin address. Requires authorization from current admin.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), BadgeError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(BadgeError::NotInitialized)?;
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &new_admin);
        extend_instance_ttl(&env);

        Ok(())
    }

    /// Sets or replaces the primary authorised contract. Admin-only.
    pub fn set_authorised_contract(env: Env, contract: Address) -> Result<(), BadgeError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(BadgeError::NotInitialized)?;
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::PrimaryAuthorisedContract, &contract);

        // Also ensure it is in the authorised set and list
        let mut list: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AuthorisedList)
            .unwrap_or_else(|| Vec::new(&env));
        let mut found = false;
        for c in list.iter() {
            if c == contract {
                found = true;
                break;
            }
        }
        if !found {
            env.storage()
                .instance()
                .set(&DataKey::Authorised(contract.clone()), &true);
            list.push_back(contract);
            env.storage()
                .instance()
                .set(&DataKey::AuthorisedList, &list);
        }

        extend_instance_ttl(&env);
        Ok(())
    }

    /// Adds an authorised contract address capable of minting badges. Admin-only.
    pub fn add_authorised_contract(env: Env, contract: Address) -> Result<(), BadgeError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(BadgeError::NotInitialized)?;
        admin.require_auth();

        if is_authorised_internal(&env, &contract) {
            return Err(BadgeError::ContractAlreadyAuthorized);
        }

        env.storage()
            .instance()
            .set(&DataKey::Authorised(contract.clone()), &true);

        // Set as primary if no primary exists
        if !env
            .storage()
            .instance()
            .has(&DataKey::PrimaryAuthorisedContract)
        {
            env.storage()
                .instance()
                .set(&DataKey::PrimaryAuthorisedContract, &contract);
        }

        let mut list: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AuthorisedList)
            .unwrap_or_else(|| Vec::new(&env));
        list.push_back(contract);
        env.storage()
            .instance()
            .set(&DataKey::AuthorisedList, &list);

        extend_instance_ttl(&env);
        Ok(())
    }

    /// Removes an authorised contract address. Admin-only.
    pub fn remove_authorised_contract(env: Env, contract: Address) -> Result<(), BadgeError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(BadgeError::NotInitialized)?;
        admin.require_auth();

        if !is_authorised_internal(&env, &contract) {
            return Err(BadgeError::ContractNotAuthorized);
        }

        env.storage()
            .instance()
            .remove(&DataKey::Authorised(contract.clone()));

        // Update list
        let list: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AuthorisedList)
            .unwrap_or_else(|| Vec::new(&env));

        let mut new_list = Vec::new(&env);
        for item in list.iter() {
            if item != contract {
                new_list.push_back(item);
            }
        }
        env.storage()
            .instance()
            .set(&DataKey::AuthorisedList, &new_list);

        // Clear primary if this was the primary
        if let Some(primary) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::PrimaryAuthorisedContract)
        {
            if primary == contract {
                if new_list.is_empty() {
                    env.storage()
                        .instance()
                        .remove(&DataKey::PrimaryAuthorisedContract);
                } else {
                    env.storage().instance().set(
                        &DataKey::PrimaryAuthorisedContract,
                        &new_list.get(0).unwrap(),
                    );
                }
            }
        }

        extend_instance_ttl(&env);
        Ok(())
    }

    /// Returns whether `contract` is an authorised contract.
    pub fn is_authorised(env: Env, contract: Address) -> bool {
        is_authorised_internal(&env, &contract)
    }

    /// Returns the primary authorised contract address, if configured.
    pub fn get_authorised_contract(env: Env) -> Option<Address> {
        env.storage()
            .instance()
            .get(&DataKey::PrimaryAuthorisedContract)
    }

    /// Returns all authorised contract addresses.
    pub fn get_authorised_contracts(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::AuthorisedList)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Mints an achievement badge to `wallet` for reaching a key milestone.
    ///
    /// Restricted to authorised contracts only. Enforces the requirement that
    /// each badge type may only be earned once per wallet. Emits `BadgeMinted`.
    ///
    /// # Errors
    /// - [`BadgeError::NoAuthorisedContract`] if no contract has been authorised.
    /// - [`BadgeError::BadgeAlreadyEarned`] if `wallet` already has this badge type.
    pub fn mint_badge(env: Env, wallet: Address, badge_type: BadgeType) -> Result<(), BadgeError> {
        let auth_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::PrimaryAuthorisedContract)
            .ok_or(BadgeError::NoAuthorisedContract)?;

        // Verify authorization of the authorised contract
        auth_contract.require_auth();

        // Enforce uniqueness: duplicate badge mint rejected with BadgeAlreadyEarned error
        if has_badge_internal(&env, &wallet, badge_type) {
            return Err(BadgeError::BadgeAlreadyEarned);
        }

        let now = env.ledger().timestamp();
        let record = BadgeRecord {
            badge_type,
            mint_timestamp: now,
        };

        // Store per-badge marker
        let badge_key = DataKey::Badge(wallet.clone(), badge_type);
        env.storage().persistent().set(&badge_key, &record);
        env.storage()
            .persistent()
            .extend_ttl(&badge_key, BADGE_TTL_THRESHOLD, BADGE_TTL_EXTEND_TO);

        // Append to wallet's badge list
        let wallet_key = DataKey::WalletBadges(wallet.clone());
        let mut badges: Vec<BadgeRecord> = env
            .storage()
            .persistent()
            .get(&wallet_key)
            .unwrap_or_else(|| Vec::new(&env));
        badges.push_back(record);
        env.storage().persistent().set(&wallet_key, &badges);
        env.storage().persistent().extend_ttl(
            &wallet_key,
            BADGE_TTL_THRESHOLD,
            BADGE_TTL_EXTEND_TO,
        );

        // Update total badges minted
        let total: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalBadgesMinted)
            .unwrap_or(0);
        let new_total = total.checked_add(1).ok_or(BadgeError::ArithmeticOverflow)?;
        env.storage()
            .instance()
            .set(&DataKey::TotalBadgesMinted, &new_total);
        extend_instance_ttl(&env);

        // Emit BadgeMinted event with wallet and badge_type
        env.events().publish(
            (Symbol::new(&env, BADGE_MINTED_EVENT_NAME), wallet.clone()),
            BadgeMinted { wallet, badge_type },
        );

        Ok(())
    }

    /// Mints a badge with an explicit authorised caller address.
    ///
    /// The `caller` must authorise the call (`caller.require_auth()`) and must
    /// be registered in the set of authorised contracts.
    ///
    /// # Errors
    /// - [`BadgeError::Unauthorized`] if `caller` is not an authorised contract.
    /// - [`BadgeError::BadgeAlreadyEarned`] if `wallet` already has this badge type.
    pub fn mint_badge_by(
        env: Env,
        caller: Address,
        wallet: Address,
        badge_type: BadgeType,
    ) -> Result<(), BadgeError> {
        caller.require_auth();

        if !is_authorised_internal(&env, &caller) {
            return Err(BadgeError::Unauthorized);
        }

        if has_badge_internal(&env, &wallet, badge_type) {
            return Err(BadgeError::BadgeAlreadyEarned);
        }

        let now = env.ledger().timestamp();
        let record = BadgeRecord {
            badge_type,
            mint_timestamp: now,
        };

        let badge_key = DataKey::Badge(wallet.clone(), badge_type);
        env.storage().persistent().set(&badge_key, &record);
        env.storage()
            .persistent()
            .extend_ttl(&badge_key, BADGE_TTL_THRESHOLD, BADGE_TTL_EXTEND_TO);

        let wallet_key = DataKey::WalletBadges(wallet.clone());
        let mut badges: Vec<BadgeRecord> = env
            .storage()
            .persistent()
            .get(&wallet_key)
            .unwrap_or_else(|| Vec::new(&env));
        badges.push_back(record);
        env.storage().persistent().set(&wallet_key, &badges);
        env.storage().persistent().extend_ttl(
            &wallet_key,
            BADGE_TTL_THRESHOLD,
            BADGE_TTL_EXTEND_TO,
        );

        let total: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalBadgesMinted)
            .unwrap_or(0);
        let new_total = total.checked_add(1).ok_or(BadgeError::ArithmeticOverflow)?;
        env.storage()
            .instance()
            .set(&DataKey::TotalBadgesMinted, &new_total);
        extend_instance_ttl(&env);

        env.events().publish(
            (Symbol::new(&env, BADGE_MINTED_EVENT_NAME), wallet.clone()),
            BadgeMinted { wallet, badge_type },
        );

        Ok(())
    }

    /// Returns the list of all earned badge types and their mint timestamps for `wallet`.
    pub fn get_badges(env: Env, wallet: Address) -> Vec<BadgeRecord> {
        let wallet_key = DataKey::WalletBadges(wallet.clone());
        let badges: Vec<BadgeRecord> = env
            .storage()
            .persistent()
            .get(&wallet_key)
            .unwrap_or_else(|| Vec::new(&env));

        if env.storage().persistent().has(&wallet_key) {
            env.storage().persistent().extend_ttl(
                &wallet_key,
                BADGE_TTL_THRESHOLD,
                BADGE_TTL_EXTEND_TO,
            );
        }

        badges
    }

    /// Returns whether `wallet` has earned the specified badge type.
    pub fn has_badge(env: Env, wallet: Address, badge_type: BadgeType) -> bool {
        has_badge_internal(&env, &wallet, badge_type)
    }

    /// Returns the mint timestamp for `wallet`'s badge of type `badge_type`, if earned.
    pub fn get_badge_timestamp(env: Env, wallet: Address, badge_type: BadgeType) -> Option<u64> {
        let key = DataKey::Badge(wallet, badge_type);
        let record: Option<BadgeRecord> = env.storage().persistent().get(&key);
        record.map(|r| r.mint_timestamp)
    }

    /// Returns the total number of badges earned by `wallet`.
    pub fn get_wallet_badge_count(env: Env, wallet: Address) -> u32 {
        let key = DataKey::WalletBadges(wallet);
        let badges: Vec<BadgeRecord> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));
        badges.len()
    }

    /// Returns the total count of badges minted across all wallets.
    pub fn get_total_badges(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::TotalBadgesMinted)
            .unwrap_or(0)
    }
}
