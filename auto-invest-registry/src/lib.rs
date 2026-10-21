#![no_std]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::enum_variant_names)]

#[cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

#[cfg(test)]
mod test;

/// Event name emitted when a new auto-invest rule is registered.
pub const RULE_REGISTERED_EVENT_NAME: Symbol = symbol_short!("rule_reg");

/// Event name emitted when an existing auto-invest rule is updated.
pub const RULE_UPDATED_EVENT_NAME: Symbol = symbol_short!("rule_upd");

/// Maximum number of rules an individual wallet can register (50).
pub const MAX_RULES_PER_WALLET: u32 = 50;

/// Minimum valid rating score scaled by 100 (100 = 1.00).
pub const MIN_SCALED_RATING: u32 = 100;

/// Maximum valid rating score scaled by 100 (500 = 5.00).
pub const MAX_SCALED_RATING: u32 = 500;

/// Persistent storage TTL threshold (~6 days at 5s ledgers).
pub const REGISTRY_TTL_THRESHOLD: u32 = 100_000;

/// Persistent storage TTL extension target (~30 days at 5s ledgers).
pub const REGISTRY_TTL_EXTEND_TO: u32 = 518_400;

/// Errors returned by the auto-invest registry contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AutoInvestError {
    /// The specified rule ID was not found.
    NotFound = 1,
    /// Minimum rating must be between 100 and 500 (scaled by 100, e.g. 450 = 4.50).
    MinRatingOutOfRange = 2,
    /// Maximum purchase price must be strictly greater than zero.
    MaxPriceInvalid = 3,
    /// Maximum keys per key ID must be strictly greater than zero.
    MaxPerKeyZero = 4,
    /// Cannot update a deactivated rule.
    Deactivated = 5,
    /// Rule has already been deactivated.
    AlreadyInactive = 6,
    /// Wallet has reached the maximum allowed registered rules (50).
    TooManyRules = 7,
    /// Arithmetic overflow while incrementing rule counter.
    ArithmeticOverflow = 8,
}

/// Evaluation criteria for rule-based automated key purchases.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Criteria {
    /// Category filter (0 = wildcard / any category, or specific non-zero category ID).
    pub key_category: u32,
    /// Minimum required rating score, scaled by 100 (100..=500, matching average_score_scaled).
    pub min_rating: u32,
    /// Maximum purchase price in stroops / native tokens.
    pub max_price: i128,
    /// Maximum quantity of keys allowed to be held for this key ID.
    pub max_per_key: u32,
}

/// Stored rule configuration for an investor.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Rule {
    /// Unique rule identifier.
    pub id: u64,
    /// Wallet address that owns and registered the rule.
    pub owner: Address,
    /// Evaluation criteria for key purchases.
    pub criteria: Criteria,
    /// Whether the rule is active and eligible for evaluation.
    pub active: bool,
    /// Ledger timestamp when the rule was created.
    pub created_at: u64,
}

/// Result of evaluating key purchase eligibility.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CheckResult {
    /// True if the key satisfies all criteria of an active rule.
    pub eligible: bool,
    /// The ID of the matching active rule in registration order, if eligible.
    pub rule_id: Option<u64>,
}

/// Payload emitted for the `RuleRegistered` event.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RuleRegistered {
    pub rule_id: u64,
    pub wallet: Address,
    pub criteria: Criteria,
}

/// Payload emitted for the `RuleUpdated` event.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RuleUpdated {
    pub rule_id: u64,
    pub wallet: Address,
    pub criteria: Criteria,
}

/// Persistent storage keys used by the auto-invest registry contract.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    /// Global auto-incrementing rule ID counter.
    NextRuleId,
    /// Rule record by ID: `Rule(rule_id)` -> `Rule`.
    Rule(u64),
    /// List of rule IDs registered for a wallet: `WalletRules(wallet)` -> `Vec<u64>`.
    WalletRules(Address),
}

#[contract]
pub struct AutoInvestRegistryContract;

fn extend_ttl(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, REGISTRY_TTL_THRESHOLD, REGISTRY_TTL_EXTEND_TO);
}

fn validate_criteria(criteria: &Criteria) -> Result<(), AutoInvestError> {
    if criteria.min_rating < MIN_SCALED_RATING || criteria.min_rating > MAX_SCALED_RATING {
        return Err(AutoInvestError::MinRatingOutOfRange);
    }
    if criteria.max_price <= 0 {
        return Err(AutoInvestError::MaxPriceInvalid);
    }
    if criteria.max_per_key == 0 {
        return Err(AutoInvestError::MaxPerKeyZero);
    }
    Ok(())
}

fn get_wallet_rule_ids(env: &Env, wallet: &Address) -> Vec<u64> {
    let key = DataKey::WalletRules(wallet.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env))
}

fn load_rule(env: &Env, rule_id: u64) -> Option<Rule> {
    let key = DataKey::Rule(rule_id);
    env.storage().persistent().get(&key)
}

#[contractimpl]
impl AutoInvestRegistryContract {
    /// Registers a new auto-invest rule for `wallet`.
    ///
    /// Validates criteria, verifies wallet authorization, stores the rule in registration
    /// order, and emits a `RuleRegistered` event.
    ///
    /// Returns [`AutoInvestError::TooManyRules`] if the wallet already has [`MAX_RULES_PER_WALLET`] (50) rules.
    pub fn register_rule(
        env: Env,
        wallet: Address,
        criteria: Criteria,
    ) -> Result<u64, AutoInvestError> {
        wallet.require_auth();
        validate_criteria(&criteria)?;

        let mut wallet_rules = get_wallet_rule_ids(&env, &wallet);
        if wallet_rules.len() >= MAX_RULES_PER_WALLET {
            return Err(AutoInvestError::TooManyRules);
        }

        let current_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::NextRuleId)
            .unwrap_or(0);
        let next_id = current_id
            .checked_add(1)
            .ok_or(AutoInvestError::ArithmeticOverflow)?;

        let next_key = DataKey::NextRuleId;
        env.storage().persistent().set(&next_key, &next_id);
        extend_ttl(&env, &next_key);

        let rule = Rule {
            id: next_id,
            owner: wallet.clone(),
            criteria: criteria.clone(),
            active: true,
            created_at: env.ledger().timestamp(),
        };

        let rule_key = DataKey::Rule(next_id);
        env.storage().persistent().set(&rule_key, &rule);
        extend_ttl(&env, &rule_key);

        wallet_rules.push_back(next_id);
        let wallet_key = DataKey::WalletRules(wallet.clone());
        env.storage().persistent().set(&wallet_key, &wallet_rules);
        extend_ttl(&env, &wallet_key);

        env.events().publish(
            (RULE_REGISTERED_EVENT_NAME, wallet.clone()),
            RuleRegistered {
                rule_id: next_id,
                wallet,
                criteria,
            },
        );

        Ok(next_id)
    }

    /// Updates the criteria of an existing active rule.
    ///
    /// The rule owner must authorize the call. The rule owner and ID are immutable.
    /// Updating a deactivated rule returns [`AutoInvestError::Deactivated`].
    /// Emits a `RuleUpdated` event upon success.
    pub fn update_rule(env: Env, rule_id: u64, criteria: Criteria) -> Result<(), AutoInvestError> {
        let rule_key = DataKey::Rule(rule_id);
        let mut rule: Rule = env
            .storage()
            .persistent()
            .get(&rule_key)
            .ok_or(AutoInvestError::NotFound)?;

        rule.owner.require_auth();

        if !rule.active {
            return Err(AutoInvestError::Deactivated);
        }

        validate_criteria(&criteria)?;

        rule.criteria = criteria.clone();
        env.storage().persistent().set(&rule_key, &rule);
        extend_ttl(&env, &rule_key);

        env.events().publish(
            (RULE_UPDATED_EVENT_NAME, rule.owner.clone()),
            RuleUpdated {
                rule_id,
                wallet: rule.owner,
                criteria,
            },
        );

        Ok(())
    }

    /// Deactivates an existing rule without deleting it.
    ///
    /// The rule owner must authorize the call. Once deactivated, the rule is excluded
    /// from check_rule evaluation. If already deactivated, returns
    /// [`AutoInvestError::AlreadyInactive`].
    pub fn deactivate_rule(env: Env, rule_id: u64) -> Result<(), AutoInvestError> {
        let rule_key = DataKey::Rule(rule_id);
        let mut rule: Rule = env
            .storage()
            .persistent()
            .get(&rule_key)
            .ok_or(AutoInvestError::NotFound)?;

        rule.owner.require_auth();

        if !rule.active {
            return Err(AutoInvestError::AlreadyInactive);
        }

        rule.active = false;
        env.storage().persistent().set(&rule_key, &rule);
        extend_ttl(&env, &rule_key);

        Ok(())
    }

    /// Evaluates key purchase eligibility using caller-supplied contextual market data.
    ///
    /// # Advisory Notice
    /// This function relies entirely on caller-supplied data (`category`, `rating`,
    /// `price`, and `current_holdings`) and is strictly **advisory** for automated execution
    /// services, not an on-chain security gate.
    ///
    /// # Category Matching
    /// Because the on-chain `creator-keys` contract does not store key categories,
    /// rules requiring a specific non-wildcard category (`key_category != 0`) cannot be
    /// resolved via any on-chain lookup; the caller must supply the category indexed from
    /// off-chain metadata. A rule with `key_category == 0` acts as a wildcard and matches
    /// any key category.
    ///
    /// Iterates through the wallet's rules in registration order (FIFO).
    /// Deactivated rules are excluded from evaluation. Returns the first matching rule,
    /// or `{ eligible: false, rule: None }` if no rule matches all criteria.
    #[allow(clippy::too_many_arguments)]
    pub fn check_rule_with_context(
        env: Env,
        wallet: Address,
        key_id: Address,
        category: u32,
        rating: u32,
        price: i128,
        current_holdings: u32,
    ) -> CheckResult {
        let _ = key_id;
        let rule_ids = get_wallet_rule_ids(&env, &wallet);
        for rule_id in rule_ids.iter() {
            if let Some(rule) = load_rule(&env, rule_id) {
                if !rule.active {
                    continue;
                }

                let c = &rule.criteria;
                let category_match = c.key_category == 0 || c.key_category == category;
                let rating_match = rating >= c.min_rating;
                let price_match = price <= c.max_price;
                let holdings_match = current_holdings < c.max_per_key;

                if category_match && rating_match && price_match && holdings_match {
                    return CheckResult {
                        eligible: true,
                        rule_id: Some(rule.id),
                    };
                }
            }
        }

        CheckResult {
            eligible: false,
            rule_id: None,
        }
    }

    /// Evaluates key purchase eligibility for `wallet` and `key_id` given key market data.
    ///
    /// Delegates directly to [`Self::check_rule_with_context`]. Relies on caller-supplied
    /// data and is strictly advisory, not a security gate.
    #[allow(clippy::too_many_arguments)]
    pub fn check_rule(
        env: Env,
        wallet: Address,
        key_id: Address,
        category: u32,
        rating: u32,
        price: i128,
        current_holdings: u32,
    ) -> CheckResult {
        Self::check_rule_with_context(
            env,
            wallet,
            key_id,
            category,
            rating,
            price,
            current_holdings,
        )
    }

    /// Returns all rules registered for `wallet`, in registration order.
    pub fn get_rules(env: Env, wallet: Address) -> Vec<Rule> {
        let rule_ids = get_wallet_rule_ids(&env, &wallet);
        let mut rules = Vec::new(&env);
        for rule_id in rule_ids.iter() {
            if let Some(rule) = load_rule(&env, rule_id) {
                rules.push_back(rule);
            }
        }
        rules
    }

    /// Returns a specific rule by its unique ID.
    pub fn get_rule(env: Env, rule_id: u64) -> Result<Rule, AutoInvestError> {
        load_rule(&env, rule_id).ok_or(AutoInvestError::NotFound)
    }
}
