#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HealthWeights {
    pub reserve_coverage: u32,
    pub concentration: u32,
    pub trade_activity: u32,
    pub age: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HealthBreakdown {
    pub reserve_score: u32,
    pub reserve_value: i128,
    pub concentration_score: u32,
    pub concentration_value: u32,
    pub activity_score: u32,
    pub activity_value: i128,
    pub age_score: u32,
    pub age_value: u64,
    pub total_score: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Feature {
    Borrowing,
    InsuranceCover,
}

#[contracttype]
enum DataKey {
    Admin,
    Weights,
    MinHealthFor(Feature),
    KeyReserve(Address),
    KeyTopHolderShare(Address),
    KeyTradeVolume7d(Address),
    KeyCreatedAt(Address),
    HealthThresholds,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum HealthScoreError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidWeights = 3,
    InsufficientHealth = 4,
    ArithmeticOverflow = 5,
}

#[contract]
pub struct KeyHealthScoreContract;

fn require_admin(env: &Env, caller: &Address) -> Result<(), HealthScoreError> {
    caller.require_auth();
    if env.storage().instance().get::<_, Address>(&DataKey::Admin) != Some(caller.clone()) {
        return Err(HealthScoreError::Unauthorized);
    }
    Ok(())
}

#[contractimpl]
impl KeyHealthScoreContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), HealthScoreError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(HealthScoreError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);

        // Set default weights
        let default_weights = HealthWeights {
            reserve_coverage: 30,
            concentration: 25,
            trade_activity: 25,
            age: 20,
        };
        env.storage()
            .instance()
            .set(&DataKey::Weights, &default_weights);

        // Set default minimum health requirements
        env.storage()
            .instance()
            .set(&DataKey::MinHealthFor(Feature::Borrowing), &60u32);
        env.storage()
            .instance()
            .set(&DataKey::MinHealthFor(Feature::InsuranceCover), &50u32);

        Ok(())
    }

    pub fn set_health_weights(
        env: Env,
        admin: Address,
        weights: HealthWeights,
    ) -> Result<(), HealthScoreError> {
        require_admin(&env, &admin)?;

        // Validate weights sum to 100
        let total = weights
            .reserve_coverage
            .checked_add(weights.concentration)
            .and_then(|v| v.checked_add(weights.trade_activity))
            .and_then(|v| v.checked_add(weights.age))
            .ok_or(HealthScoreError::ArithmeticOverflow)?;

        if total != 100 {
            return Err(HealthScoreError::InvalidWeights);
        }

        env.storage().instance().set(&DataKey::Weights, &weights);
        Ok(())
    }

    pub fn set_min_health_for(
        env: Env,
        admin: Address,
        feature: Feature,
        min_score: u32,
    ) -> Result<(), HealthScoreError> {
        require_admin(&env, &admin)?;
        if min_score > 100 {
            return Err(HealthScoreError::InvalidWeights);
        }
        env.storage()
            .instance()
            .set(&DataKey::MinHealthFor(feature), &min_score);
        Ok(())
    }

    pub fn health_score(env: Env, key_id: Address) -> u32 {
        let weights: HealthWeights =
            env.storage()
                .instance()
                .get(&DataKey::Weights)
                .unwrap_or(HealthWeights {
                    reserve_coverage: 25,
                    concentration: 25,
                    trade_activity: 25,
                    age: 25,
                });

        let reserve_score = Self::calculate_reserve_score(env.clone(), &key_id);
        let concentration_score = Self::calculate_concentration_score(env.clone(), &key_id);
        let activity_score = Self::calculate_activity_score(env.clone(), &key_id);
        let age_score = Self::calculate_age_score(env.clone(), &key_id);

        let total = (reserve_score * weights.reserve_coverage
            + concentration_score * weights.concentration
            + activity_score * weights.trade_activity
            + age_score * weights.age)
            / 100;

        total.min(100)
    }

    pub fn health_breakdown(env: Env, key_id: Address) -> HealthBreakdown {
        let weights: HealthWeights =
            env.storage()
                .instance()
                .get(&DataKey::Weights)
                .unwrap_or(HealthWeights {
                    reserve_coverage: 25,
                    concentration: 25,
                    trade_activity: 25,
                    age: 25,
                });

        let reserve_value: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::KeyReserve(key_id.clone()))
            .unwrap_or(0);
        let reserve_score = Self::calculate_reserve_score(env.clone(), &key_id);

        let concentration_value: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::KeyTopHolderShare(key_id.clone()))
            .unwrap_or(0);
        let concentration_score = Self::calculate_concentration_score(env.clone(), &key_id);

        let activity_value: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::KeyTradeVolume7d(key_id.clone()))
            .unwrap_or(0);
        let activity_score = Self::calculate_activity_score(env.clone(), &key_id);

        let age_value: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::KeyCreatedAt(key_id.clone()))
            .unwrap_or(0);
        let age_score = Self::calculate_age_score(env.clone(), &key_id);

        let total = (reserve_score * weights.reserve_coverage
            + concentration_score * weights.concentration
            + activity_score * weights.trade_activity
            + age_score * weights.age)
            / 100;

        HealthBreakdown {
            reserve_score,
            reserve_value,
            concentration_score,
            concentration_value,
            activity_score,
            activity_value,
            age_score,
            age_value,
            total_score: total.min(100),
        }
    }

    pub fn min_health_for(env: Env, feature: Feature) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::MinHealthFor(feature))
            .unwrap_or(50)
    }

    pub fn assert_health_for_feature(
        env: Env,
        key_id: Address,
        feature: Feature,
    ) -> Result<(), HealthScoreError> {
        let score = Self::health_score(env.clone(), key_id);
        let min_required = Self::min_health_for(env, feature);

        if score < min_required {
            return Err(HealthScoreError::InsufficientHealth);
        }

        Ok(())
    }

    // Helper functions to calculate component scores
    fn calculate_reserve_score(env: Env, key_id: &Address) -> u32 {
        let reserve: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::KeyReserve(key_id.clone()))
            .unwrap_or(0);

        // Simple scoring: reserve as % of some baseline
        // For demo: 100k = 100%, 0 = 0%
        if reserve >= 100_000 {
            100
        } else if reserve <= 0 {
            0
        } else {
            ((reserve * 100) / 100_000).min(100) as u32
        }
    }

    fn calculate_concentration_score(env: Env, key_id: &Address) -> u32 {
        let top_holder_share: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::KeyTopHolderShare(key_id.clone()))
            .unwrap_or(0);

        // Lower concentration is better
        // If top holder has >50%, score 0
        // If top holder has <10%, score 100
        if top_holder_share >= 50 {
            0
        } else if top_holder_share <= 10 {
            100
        } else {
            100 - ((top_holder_share - 10) * 100 / 40)
        }
    }

    fn calculate_activity_score(env: Env, key_id: &Address) -> u32 {
        let volume_7d: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::KeyTradeVolume7d(key_id.clone()))
            .unwrap_or(0);

        // Simple scoring: volume as % of baseline
        // For demo: 50k = 100%, 0 = 0%
        if volume_7d >= 50_000 {
            100
        } else if volume_7d <= 0 {
            0
        } else {
            ((volume_7d * 100) / 50_000).min(100) as u32
        }
    }

    fn calculate_age_score(env: Env, key_id: &Address) -> u32 {
        let created_at: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::KeyCreatedAt(key_id.clone()))
            .unwrap_or(0);

        if created_at == 0 {
            return 0;
        }

        let now = env.ledger().timestamp();
        let age = now.saturating_sub(created_at);

        // Older is better (more proven)
        // 90 days = 100%, 0 days = 0%
        let ninety_days = 90 * 24 * 3600;
        if age >= ninety_days {
            100
        } else {
            ((age * 100) / ninety_days).min(100) as u32
        }
    }

    // Admin function to update key data (for testing/demo)
    pub fn update_key_data(
        env: Env,
        admin: Address,
        key_id: Address,
        reserve: i128,
        top_holder_share_bps: u32,
        volume_7d: i128,
        created_at: u64,
    ) -> Result<(), HealthScoreError> {
        require_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&DataKey::KeyReserve(key_id.clone()), &reserve);
        env.storage().persistent().set(
            &DataKey::KeyTopHolderShare(key_id.clone()),
            &top_holder_share_bps,
        );
        env.storage()
            .persistent()
            .set(&DataKey::KeyTradeVolume7d(key_id.clone()), &volume_7d);
        env.storage()
            .persistent()
            .set(&DataKey::KeyCreatedAt(key_id.clone()), &created_at);

        let score = Self::health_score(env.clone(), key_id.clone());
        env.events().publish(("health_updated", key_id), score);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (Env, KeyHealthScoreContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(KeyHealthScoreContract, ());
        let client = KeyHealthScoreContractClient::new(&env, &contract_id);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_health_score_deterministic() {
        let (env, client, admin) = setup();
        let key_id = Address::generate(&env);
        let now = env.ledger().timestamp();

        client.update_key_data(&admin, &key_id, &100_000, &20, &50_000, &now);

        let score1 = client.health_score(&key_id);
        let score2 = client.health_score(&key_id);
        assert_eq!(score1, score2);
        assert!(score1 <= 100);
    }

    #[test]
    fn test_breakdown_components_sum_to_score() {
        let (env, client, admin) = setup();
        let key_id = Address::generate(&env);
        let now = env.ledger().timestamp();

        client.update_key_data(&admin, &key_id, &100_000, &20, &50_000, &now);

        let breakdown = client.health_breakdown(&key_id);
        let weights = HealthWeights {
            reserve_coverage: 30,
            concentration: 25,
            trade_activity: 25,
            age: 20,
        };

        let calculated = (breakdown.reserve_score * weights.reserve_coverage
            + breakdown.concentration_score * weights.concentration
            + breakdown.activity_score * weights.trade_activity
            + breakdown.age_score * weights.age)
            / 100;

        assert_eq!(breakdown.total_score, calculated.min(100));
    }

    #[test]
    fn test_feature_min_health_enforced() {
        let (env, client, admin) = setup();
        let key_id = Address::generate(&env);
        let now = env.ledger().timestamp();

        // Set low health data
        client.update_key_data(&admin, &key_id, &10_000, &60, &1_000, &now);

        let score = client.health_score(&key_id);
        let min_borrowing = client.min_health_for(&Feature::Borrowing);

        if score < min_borrowing {
            assert_eq!(
                client.try_assert_health_for_feature(&key_id, &Feature::Borrowing),
                Err(Ok(HealthScoreError::InsufficientHealth))
            );
        }
    }

    #[test]
    fn test_weights_must_sum_to_100() {
        let (_env, client, admin) = setup();
        let invalid_weights = HealthWeights {
            reserve_coverage: 30,
            concentration: 30,
            trade_activity: 30,
            age: 20, // Sum = 110
        };

        assert_eq!(
            client.try_set_health_weights(&admin, &invalid_weights),
            Err(Ok(HealthScoreError::InvalidWeights))
        );
    }

    #[test]
    fn test_only_admin_can_change_weights() {
        let (env, client, _admin) = setup();
        let non_admin = Address::generate(&env);
        let weights = HealthWeights {
            reserve_coverage: 25,
            concentration: 25,
            trade_activity: 25,
            age: 25,
        };

        assert_eq!(
            client.try_set_health_weights(&non_admin, &weights),
            Err(Ok(HealthScoreError::Unauthorized))
        );
    }

    #[test]
    fn test_reserve_coverage_affects_score() {
        let (env, client, admin) = setup();
        let key1 = Address::generate(&env);
        let key2 = Address::generate(&env);
        let now = env.ledger().timestamp();

        // High reserve
        client.update_key_data(&admin, &key1, &100_000, &20, &50_000, &now);
        // Low reserve
        client.update_key_data(&admin, &key2, &10_000, &20, &50_000, &now);

        let score1 = client.health_score(&key1);
        let score2 = client.health_score(&key2);

        assert!(score1 > score2);
    }

    #[test]
    fn test_concentration_affects_score() {
        let (env, client, admin) = setup();
        let key1 = Address::generate(&env);
        let key2 = Address::generate(&env);
        let now = env.ledger().timestamp();

        // Low concentration (good)
        client.update_key_data(&admin, &key1, &100_000, &10, &50_000, &now);
        // High concentration (bad)
        client.update_key_data(&admin, &key2, &100_000, &60, &50_000, &now);

        let score1 = client.health_score(&key1);
        let score2 = client.health_score(&key2);

        assert!(score1 > score2);
    }
}
