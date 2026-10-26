#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThresholdType {
    HourlyVolume,
    PriceImpact,
    SwapsPerBlock,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thresholds {
    pub max_hourly_volume: i128,
    pub max_single_swap_impact_bps: u32,
    pub max_swaps_per_block: u32,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    SwapContract,
    Thresholds,
    Suspended(Address),
    Volume(Address),
    Swaps(Address, u32),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CircuitBreakerError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidThreshold = 3,
    CircuitBreakerActive = 4,
    ArithmeticOverflow = 5,
}

#[contract]
pub struct CircuitBreakerContract;

fn require_admin(env: &Env, caller: &Address) -> Result<(), CircuitBreakerError> {
    caller.require_auth();
    if env.storage().instance().get::<_, Address>(&DataKey::Admin) != Some(caller.clone()) {
        return Err(CircuitBreakerError::Unauthorized);
    }
    Ok(())
}

#[contractimpl]
impl CircuitBreakerContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        swap_contract: Address,
        thresholds: Thresholds,
    ) -> Result<(), CircuitBreakerError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(CircuitBreakerError::AlreadyInitialized);
        }
        admin.require_auth();
        if thresholds.max_hourly_volume <= 0
            || thresholds.max_single_swap_impact_bps == 0
            || thresholds.max_swaps_per_block == 0
        {
            return Err(CircuitBreakerError::InvalidThreshold);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::SwapContract, &swap_contract);
        env.storage()
            .instance()
            .set(&DataKey::Thresholds, &thresholds);
        Ok(())
    }

    pub fn configure_thresholds(
        env: Env,
        caller: Address,
        thresholds: Thresholds,
    ) -> Result<(), CircuitBreakerError> {
        require_admin(&env, &caller)?;
        if thresholds.max_hourly_volume <= 0
            || thresholds.max_single_swap_impact_bps == 0
            || thresholds.max_swaps_per_block == 0
        {
            return Err(CircuitBreakerError::InvalidThreshold);
        }
        env.storage()
            .instance()
            .set(&DataKey::Thresholds, &thresholds);
        Ok(())
    }

    /// Returns false if this swap tripped the breaker. The caller must then
    /// abort its own swap; returning an error here would roll back the trip.
    pub fn record_swap(
        env: Env,
        caller: Address,
        key_id: Address,
        amount: i128,
        price_impact_bps: u32,
    ) -> Result<bool, CircuitBreakerError> {
        caller.require_auth();
        let swap_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::SwapContract)
            .ok_or(CircuitBreakerError::Unauthorized)?;
        if caller != swap_contract {
            return Err(CircuitBreakerError::Unauthorized);
        }
        if Self::is_suspended(env.clone(), key_id.clone()) {
            return Err(CircuitBreakerError::CircuitBreakerActive);
        }
        if amount <= 0 {
            return Err(CircuitBreakerError::InvalidThreshold);
        }
        let thresholds: Thresholds = env
            .storage()
            .instance()
            .get(&DataKey::Thresholds)
            .ok_or(CircuitBreakerError::InvalidThreshold)?;
        let now = env.ledger().timestamp();
        let hour = now / 3600;
        let prior_hour: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Volume(key_id.clone()))
            .map(|(stored_hour, _): (u64, i128)| stored_hour)
            .unwrap_or(hour);
        let prior_volume: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Volume(key_id.clone()))
            .and_then(|(stored_hour, volume): (u64, i128)| {
                if stored_hour == hour {
                    Some(volume)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let volume = if prior_hour == hour {
            prior_volume.checked_add(amount)
        } else {
            Some(amount)
        }
        .ok_or(CircuitBreakerError::ArithmeticOverflow)?;
        let sequence = env.ledger().sequence();
        let swaps_key = DataKey::Swaps(key_id.clone(), sequence);
        let swaps: u32 = env.storage().persistent().get(&swaps_key).unwrap_or(0);
        let next_swaps = swaps
            .checked_add(1)
            .ok_or(CircuitBreakerError::ArithmeticOverflow)?;
        env.storage()
            .persistent()
            .set(&DataKey::Volume(key_id.clone()), &(hour, volume));
        env.storage().persistent().set(&swaps_key, &next_swaps);

        let breach = if volume > thresholds.max_hourly_volume {
            Some((ThresholdType::HourlyVolume, volume))
        } else if price_impact_bps > thresholds.max_single_swap_impact_bps {
            Some((ThresholdType::PriceImpact, price_impact_bps as i128))
        } else if next_swaps > thresholds.max_swaps_per_block {
            Some((ThresholdType::SwapsPerBlock, next_swaps as i128))
        } else {
            None
        };
        if let Some((kind, observed)) = breach {
            env.storage()
                .persistent()
                .set(&DataKey::Suspended(key_id.clone()), &true);
            env.events().publish(
                (Symbol::new(&env, "CircuitBreakerTripped"), key_id, kind),
                observed,
            );
            return Ok(false);
        }
        Ok(true)
    }

    pub fn is_suspended(env: Env, key_id: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Suspended(key_id))
            .unwrap_or(false)
    }

    pub fn assert_trading_enabled(env: Env, key_id: Address) -> Result<(), CircuitBreakerError> {
        if Self::is_suspended(env, key_id) {
            Err(CircuitBreakerError::CircuitBreakerActive)
        } else {
            Ok(())
        }
    }

    pub fn resume_trading(
        env: Env,
        caller: Address,
        key_id: Address,
    ) -> Result<(), CircuitBreakerError> {
        require_admin(&env, &caller)?;
        env.storage()
            .persistent()
            .remove(&DataKey::Suspended(key_id));
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{
        CircuitBreakerContract, CircuitBreakerContractClient, CircuitBreakerError, Thresholds,
    };
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, CircuitBreakerContractClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let swapper = Address::generate(&env);
        let contract_id = env.register(CircuitBreakerContract, ());
        let client = CircuitBreakerContractClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &swapper,
            &Thresholds {
                max_hourly_volume: 100,
                max_single_swap_impact_bps: 200,
                max_swaps_per_block: 2,
            },
        );
        (env, client, admin, swapper)
    }

    #[test]
    fn each_threshold_suspends_trading_and_admin_can_resume() {
        let (env, client, admin, swapper) = setup();
        let volume_key = Address::generate(&env);
        assert!(!client.record_swap(&swapper, &volume_key, &101, &0));
        assert!(client.is_suspended(&volume_key));
        assert_eq!(
            client.try_assert_trading_enabled(&volume_key),
            Err(Ok(CircuitBreakerError::CircuitBreakerActive))
        );

        let impact_key = Address::generate(&env);
        assert!(!client.record_swap(&swapper, &impact_key, &1, &201));
        assert!(client.is_suspended(&impact_key));

        let rate_key = Address::generate(&env);
        assert!(client.record_swap(&swapper, &rate_key, &1, &0));
        assert!(client.record_swap(&swapper, &rate_key, &1, &0));
        assert!(!client.record_swap(&swapper, &rate_key, &1, &0));
        assert!(client.is_suspended(&rate_key));

        assert_eq!(
            client.try_resume_trading(&swapper, &volume_key),
            Err(Ok(CircuitBreakerError::Unauthorized))
        );
        client.resume_trading(&admin, &volume_key);
        assert!(!client.is_suspended(&volume_key));
    }
}
