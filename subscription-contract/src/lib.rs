#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

/// Event name emitted when a subscription is created.
pub const SUBSCRIPTION_CREATED_EVENT_NAME: Symbol = symbol_short!("sub_crt");

/// Event name emitted when a subscription is renewed.
pub const SUBSCRIPTION_RENEWED_EVENT_NAME: Symbol = symbol_short!("sub_ren");

/// Event name emitted when a subscription expires.
pub const SUBSCRIPTION_EXPIRED_EVENT_NAME: Symbol = symbol_short!("sub_exp");

/// Seconds in a day.
pub const SECONDS_PER_DAY: u64 = 86_400;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    NotPositiveDuration = 1,
    SubscriptionNotFound = 2,
    ArithmeticOverflow = 3,
}

/// Subscription data storing expiry timestamp.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Subscription {
    pub expiry: u64,
}

/// Event data payload emitted when a subscription is created.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SubscriptionCreated {
    pub wallet: Address,
    pub key_id: u64,
    pub expiry: u64,
}

/// Event data payload emitted when a subscription is renewed.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SubscriptionRenewed {
    pub wallet: Address,
    pub key_id: u64,
    pub old_expiry: u64,
    pub new_expiry: u64,
}

/// Event data payload emitted when a subscription expires.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SubscriptionExpired {
    pub wallet: Address,
    pub key_id: u64,
    pub expiry: u64,
}

/// Information about a subscription including active status and days remaining.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SubscriptionInfo {
    pub active: bool,
    pub expiry: u64,
    pub days_remaining: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Subscription(Address, u64),
}

#[contract]
pub struct SubscriptionContract;

#[contractimpl]
impl SubscriptionContract {
    /// Creates a new subscription for a wallet to access a creator key's gated content.
    /// Locks subscription fee (in a real implementation, this would handle payment).
    /// Sets expiry timestamp based on current ledger time + duration_days.
    pub fn subscribe(env: Env, wallet: Address, key_id: u64, duration_days: u64) -> Result<u64, ContractError> {
        if duration_days == 0 {
            return Err(ContractError::NotPositiveDuration);
        }

        let current_ledger = env.ledger().sequence();
        let duration_seconds = duration_days
            .checked_mul(SECONDS_PER_DAY)
            .ok_or(ContractError::ArithmeticOverflow)?;
        let expiry = current_ledger
            .checked_add(duration_seconds)
            .ok_or(ContractError::ArithmeticOverflow)?;

        let subscription = Subscription { expiry };
        let key = DataKey::Subscription(wallet.clone(), key_id);
        env.storage().persistent().set(&key, &subscription);

        env.events().publish(
            (SUBSCRIPTION_CREATED_EVENT_NAME, wallet.clone(), key_id),
            SubscriptionCreated {
                wallet,
                key_id,
                expiry,
            },
        );

        Ok(expiry)
    }

    /// Renews an existing subscription, extending the expiry by the same duration
    /// from the current expiry (not from current time).
    pub fn renew_subscription(env: Env, wallet: Address, key_id: u64, duration_days: u64) -> Result<u64, ContractError> {
        if duration_days == 0 {
            return Err(ContractError::NotPositiveDuration);
        }

        let key = DataKey::Subscription(wallet.clone(), key_id);
        let subscription: Subscription = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::SubscriptionNotFound)?;

        let old_expiry = subscription.expiry;
        let duration_seconds = duration_days
            .checked_mul(SECONDS_PER_DAY)
            .ok_or(ContractError::ArithmeticOverflow)?;
        let new_expiry = old_expiry
            .checked_add(duration_seconds)
            .ok_or(ContractError::ArithmeticOverflow)?;

        let updated_subscription = Subscription { expiry: new_expiry };
        env.storage().persistent().set(&key, &updated_subscription);

        env.events().publish(
            (SUBSCRIPTION_RENEWED_EVENT_NAME, wallet.clone(), key_id),
            SubscriptionRenewed {
                wallet,
                key_id,
                old_expiry,
                new_expiry,
            },
        );

        Ok(new_expiry)
    }

    /// Returns subscription information including active status, expiry, and days remaining.
    pub fn get_subscription(env: Env, wallet: Address, key_id: u64) -> SubscriptionInfo {
        let key = DataKey::Subscription(wallet.clone(), key_id);
        let current_ledger = env.ledger().sequence();

        if let Some(subscription) = env.storage().persistent().get::<_, Subscription>(&key) {
            let active = subscription.expiry > current_ledger;
            let days_remaining = if active {
                let remaining_seconds = subscription.expiry - current_ledger;
                remaining_seconds / SECONDS_PER_DAY
            } else {
                0
            };

            SubscriptionInfo {
                active,
                expiry: subscription.expiry,
                days_remaining,
            }
        } else {
            SubscriptionInfo {
                active: false,
                expiry: 0,
                days_remaining: 0,
            }
        }
    }

    /// Checks if a wallet has an active subscription to a creator key.
    /// Returns false if subscription doesn't exist or has expired.
    /// Intended to be called by gated content contracts for access control.
    pub fn check_subscription(env: Env, wallet: Address, key_id: u64) -> bool {
        let info = Self::get_subscription(env, wallet, key_id);
        info.active
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Env,
    };

    #[test]
    fn test_subscribe_creates_subscription() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;
        let duration_days = 30u64;

        let expiry = client.subscribe(&wallet, &key_id, &duration_days);
        
        // Expiry should be current ledger + 30 days
        let expected_expiry = env.ledger().sequence() + (30 * SECONDS_PER_DAY);
        assert_eq!(expiry, expected_expiry);

        // Check subscription info
        let info = client.get_subscription(&wallet, &key_id);
        assert!(info.active);
        assert_eq!(info.expiry, expected_expiry);
        assert_eq!(info.days_remaining, 30);
    }

    #[test]
    fn test_subscribe_with_zero_duration_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;

        let result = client.try_subscribe(&wallet, &key_id, &0);
        assert_eq!(result, Err(Ok(ContractError::NotPositiveDuration)));
    }

    #[test]
    fn test_subscribe_emits_event() {
        use soroban_sdk::TryFromVal;

        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;
        let duration_days = 30u64;

        client.subscribe(&wallet, &key_id, &duration_days);

        // Verify event was emitted
        let mut matching = soroban_sdk::Vec::new(&env);
        for (_, topics, data) in env.events().all().iter() {
            if let Some(first) = topics.get(0) {
                if Symbol::try_from_val(&env, &first) == Ok(SUBSCRIPTION_CREATED_EVENT_NAME) {
                    if let Ok(event) = SubscriptionCreated::try_from_val(&env, &data) {
                        matching.push_back(event);
                    }
                }
            }
        }

        assert_eq!(matching.len(), 1);
        let event = matching.get(0).unwrap();
        assert_eq!(event.wallet, wallet);
        assert_eq!(event.key_id, key_id);
    }

    #[test]
    fn test_renew_subscription_extends_expiry() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;
        let duration_days = 30u64;

        // Create initial subscription
        let initial_expiry = client.subscribe(&wallet, &key_id, &duration_days);
        
        // Renew subscription
        let new_expiry = client.renew_subscription(&wallet, &key_id, &duration_days);
        
        // New expiry should be old expiry + 30 days
        let expected_new_expiry = initial_expiry + (30 * SECONDS_PER_DAY);
        assert_eq!(new_expiry, expected_new_expiry);

        // Check subscription info
        let info = client.get_subscription(&wallet, &key_id);
        assert!(info.active);
        assert_eq!(info.expiry, expected_new_expiry);
        assert_eq!(info.days_remaining, 60);
    }

    #[test]
    fn test_renew_subscription_with_zero_duration_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;

        // Create initial subscription
        client.subscribe(&wallet, &key_id, &30);
        
        // Try to renew with zero duration
        let result = client.try_renew_subscription(&wallet, &key_id, &0);
        assert_eq!(result, Err(Ok(ContractError::NotPositiveDuration)));
    }

    #[test]
    fn test_renew_nonexistent_subscription_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;

        let result = client.try_renew_subscription(&wallet, &key_id, &30);
        assert_eq!(result, Err(Ok(ContractError::SubscriptionNotFound)));
    }

    #[test]
    fn test_renew_subscription_emits_event() {
        use soroban_sdk::TryFromVal;

        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;
        let duration_days = 30u64;

        // Create initial subscription
        let initial_expiry = client.subscribe(&wallet, &key_id, &duration_days);
        
        // Renew subscription
        client.renew_subscription(&wallet, &key_id, &duration_days);

        // Verify renewal event was emitted
        let mut matching = soroban_sdk::Vec::new(&env);
        for (_, topics, data) in env.events().all().iter() {
            if let Some(first) = topics.get(0) {
                if Symbol::try_from_val(&env, &first) == Ok(SUBSCRIPTION_RENEWED_EVENT_NAME) {
                    if let Ok(event) = SubscriptionRenewed::try_from_val(&env, &data) {
                        matching.push_back(event);
                    }
                }
            }
        }

        assert_eq!(matching.len(), 1);
        let event = matching.get(0).unwrap();
        assert_eq!(event.wallet, wallet);
        assert_eq!(event.key_id, key_id);
        assert_eq!(event.old_expiry, initial_expiry);
    }

    #[test]
    fn test_get_subscription_for_nonexistent_subscription() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;

        let info = client.get_subscription(&wallet, &key_id);
        assert!(!info.active);
        assert_eq!(info.expiry, 0);
        assert_eq!(info.days_remaining, 0);
    }

    #[test]
    fn test_check_subscription_active() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;

        client.subscribe(&wallet, &key_id, &30);
        
        assert!(client.check_subscription(&wallet, &key_id));
    }

    #[test]
    fn test_check_subscription_nonexistent() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;

        assert!(!client.check_subscription(&wallet, &key_id));
    }

    #[test]
    fn test_check_subscription_expired() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;

        // Create subscription with 1 day duration
        client.subscribe(&wallet, &key_id, &1);
        
        // Advance ledger by 2 days
        env.ledger().set(env.ledger().sequence() + (2 * SECONDS_PER_DAY));
        
        // Subscription should now be expired
        assert!(!client.check_subscription(&wallet, &key_id));
        
        let info = client.get_subscription(&wallet, &key_id);
        assert!(!info.active);
        assert_eq!(info.days_remaining, 0);
    }

    #[test]
    fn test_multiple_subscriptions_for_different_keys() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id_1 = 1u64;
        let key_id_2 = 2u64;

        client.subscribe(&wallet, &key_id_1, &30);
        client.subscribe(&wallet, &key_id_2, &60);

        assert!(client.check_subscription(&wallet, &key_id_1));
        assert!(client.check_subscription(&wallet, &key_id_2));

        let info_1 = client.get_subscription(&wallet, &key_id_1);
        let info_2 = client.get_subscription(&wallet, &key_id_2);
        
        assert_eq!(info_1.days_remaining, 30);
        assert_eq!(info_2.days_remaining, 60);
    }

    #[test]
    fn test_days_remaining_calculation() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(SubscriptionContract, ());
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let key_id = 1u64;

        client.subscribe(&wallet, &key_id, &10);
        
        // Initially should have 10 days
        let info = client.get_subscription(&wallet, &key_id);
        assert_eq!(info.days_remaining, 10);

        // Advance by 3 days
        env.ledger().set(env.ledger().sequence() + (3 * SECONDS_PER_DAY));
        
        // Should now have 7 days remaining
        let info = client.get_subscription(&wallet, &key_id);
        assert_eq!(info.days_remaining, 7);
    }
}
