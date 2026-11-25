#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol, Vec};

#[cfg(test)]
mod test;

/// Supported TWAP calculation windows in seconds.
pub const WINDOW_1H: u64 = 3_600;
pub const WINDOW_4H: u64 = 14_400;
pub const WINDOW_24H: u64 = 86_400;

/// Default minimum observation count guard before TWAP calculation is considered valid.
pub const DEFAULT_MIN_OBSERVATIONS: u32 = 2;

/// Default maximum retention window (24 hours) for pruning historical observations.
pub const DEFAULT_MAX_WINDOW_SECONDS: u64 = 86_400;

/// Default persistent storage TTL thresholds (~30 days).
pub const ORACLE_TTL_THRESHOLD: u32 = 100_000;
pub const ORACLE_TTL_EXTEND_TO: u32 = 518_400;

/// Errors returned by the TWAP oracle contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TwapOracleError {
    /// Returned when minimum observation count threshold is not met or window is invalid.
    #[allow(non_camel_case_types)]
    TWAPInvalid = 1,
    /// Swap price must be strictly positive.
    NotPositiveAmount = 2,
    /// Requested window is outside the supported range or zero.
    UnsupportedWindow = 3,
    /// Caller is not authorized to perform this operation.
    Unauthorized = 4,
    /// Numerical calculation resulted in an arithmetic overflow.
    ArithmeticOverflow = 5,
    /// Smart contract is already initialized with an admin.
    AlreadyInitialized = 6,
}

impl TwapOracleError {
    #[allow(non_upper_case_globals)]
    pub const TwapInvalid: Self = Self::TWAPInvalid;
}

pub type ContractError = TwapOracleError;

/// A single price-time observation recorded during bonding curve swap execution.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TwapObservation {
    /// Ledger timestamp when the observation was recorded.
    pub timestamp: u64,
    /// Spot price observed at this timestamp.
    pub price: i128,
    /// Cumulative product of price * elapsed_time up to this observation.
    pub cumulative_price_time: i128,
}

/// Configuration parameters for TWAP calculation and storage retention.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TwapConfig {
    /// Minimum observation count required before TWAP is considered valid.
    pub min_observations: u32,
    /// Maximum time window in seconds beyond which old observations are pruned.
    pub max_window_seconds: u64,
}

/// Event payload emitted on each swap recording an updated cumulative price-time value.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TwapUpdatedEvent {
    pub key_id: Address,
    pub price: i128,
    pub cumulative_price_time: i128,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    /// Protocol admin address.
    Admin,
    /// Key-specific TWAP configuration: key_id -> TwapConfig.
    Config(Address),
    /// Key-specific observation history: key_id -> Vec<TwapObservation>.
    Observations(Address),
}

fn extend_ttl(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, ORACLE_TTL_THRESHOLD, ORACLE_TTL_EXTEND_TO);
}

#[contract]
pub struct TwapOracleContract;

#[contractimpl]
impl TwapOracleContract {
    /// Initializes the TWAP oracle contract with a protocol admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), TwapOracleError> {
        let admin_key = DataKey::Admin;
        if env.storage().persistent().has(&admin_key) {
            return Err(TwapOracleError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().persistent().set(&admin_key, &admin);
        extend_ttl(&env, &admin_key);
        Ok(())
    }

    /// Returns the protocol admin address if set.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }

    /// Updates the protocol admin address. Requires authorization from the current admin.
    pub fn set_admin(
        env: Env,
        current_admin: Address,
        new_admin: Address,
    ) -> Result<(), TwapOracleError> {
        current_admin.require_auth();
        let admin_key = DataKey::Admin;
        let stored_admin: Option<Address> = env.storage().persistent().get(&admin_key);
        if let Some(admin) = stored_admin {
            if admin != current_admin {
                return Err(TwapOracleError::Unauthorized);
            }
        }
        env.storage().persistent().set(&admin_key, &new_admin);
        extend_ttl(&env, &admin_key);
        Ok(())
    }

    /// Configures the minimum observation count guard and maximum window for a creator key.
    ///
    /// Requires authorization from the protocol admin.
    pub fn set_config(
        env: Env,
        admin: Address,
        key_id: Address,
        min_observations: u32,
        max_window_seconds: u64,
    ) -> Result<(), TwapOracleError> {
        admin.require_auth();
        let stored_admin: Option<Address> = env.storage().persistent().get(&DataKey::Admin);
        if let Some(expected_admin) = stored_admin {
            if admin != expected_admin {
                return Err(TwapOracleError::Unauthorized);
            }
        }

        if min_observations < 1 || max_window_seconds == 0 {
            return Err(TwapOracleError::TWAPInvalid);
        }

        let config = TwapConfig {
            min_observations,
            max_window_seconds,
        };
        let config_key = DataKey::Config(key_id.clone());
        env.storage().persistent().set(&config_key, &config);
        extend_ttl(&env, &config_key);
        Ok(())
    }

    /// Returns the active TWAP configuration for a creator key (or default if not customized).
    pub fn get_config(env: Env, key_id: Address) -> TwapConfig {
        env.storage()
            .persistent()
            .get(&DataKey::Config(key_id))
            .unwrap_or(TwapConfig {
                min_observations: DEFAULT_MIN_OBSERVATIONS,
                max_window_seconds: DEFAULT_MAX_WINDOW_SECONDS,
            })
    }

    /// Records price and accumulates `price * elapsed_time` on each bonding curve swap execution.
    ///
    /// - Accumulates cumulative price-time using time elapsed since the previous swap.
    /// - Protects against same-block wash trades by updating spot price without spurious time inflation.
    /// - Prunes observations strictly older than the maximum window boundary (e.g. 24h) to control ledger growth.
    /// - Emits `TWAPUpdated` event carrying updated cumulative values.
    pub fn record_swap(env: Env, key_id: Address, price: i128) -> Result<i128, TwapOracleError> {
        if price <= 0 {
            return Err(TwapOracleError::NotPositiveAmount);
        }

        let obs_key = DataKey::Observations(key_id.clone());
        let mut observations: Vec<TwapObservation> = env
            .storage()
            .persistent()
            .get(&obs_key)
            .unwrap_or(Vec::new(&env));

        let now = env.ledger().timestamp();
        let config = Self::get_config(env.clone(), key_id.clone());

        let cumulative_price_time: i128;

        if observations.is_empty() {
            cumulative_price_time = 0;
            let first_obs = TwapObservation {
                timestamp: now,
                price,
                cumulative_price_time: 0,
            };
            observations.push_back(first_obs);
        } else {
            let last_idx = observations.len() - 1;
            let last_obs = observations.get(last_idx).unwrap();

            if now == last_obs.timestamp {
                // Same-block swap update (wash trade protection):
                // Update price without time advancement or cumulative inflation.
                cumulative_price_time = last_obs.cumulative_price_time;
                let updated_obs = TwapObservation {
                    timestamp: now,
                    price,
                    cumulative_price_time,
                };
                observations.set(last_idx, updated_obs);
            } else {
                let time_delta = (now.saturating_sub(last_obs.timestamp)) as i128;
                let price_time = last_obs.price.saturating_mul(time_delta);
                cumulative_price_time = last_obs.cumulative_price_time.saturating_add(price_time);

                let new_obs = TwapObservation {
                    timestamp: now,
                    price,
                    cumulative_price_time,
                };
                observations.push_back(new_obs);

                // Storage pruning: prune observations beyond the maximum window boundary
                let cutoff = now.saturating_sub(config.max_window_seconds);
                let mut first_at_or_after = None;
                for (idx, obs) in observations.iter().enumerate() {
                    if obs.timestamp >= cutoff {
                        first_at_or_after = Some(idx);
                        break;
                    }
                }

                let keep_from_idx = match first_at_or_after {
                    Some(idx) => idx.saturating_sub(1),
                    None => (observations.len().saturating_sub(2)) as usize,
                };

                if keep_from_idx > 0 {
                    let mut pruned = Vec::new(&env);
                    for (idx, obs) in observations.iter().enumerate() {
                        if idx >= keep_from_idx {
                            pruned.push_back(obs);
                        }
                    }
                    observations = pruned;
                }
            }
        }

        env.storage().persistent().set(&obs_key, &observations);
        extend_ttl(&env, &obs_key);

        env.events().publish(
            (Symbol::new(&env, "TWAPUpdated"), key_id.clone()),
            TwapUpdatedEvent {
                key_id,
                price,
                cumulative_price_time,
                timestamp: now,
            },
        );

        Ok(cumulative_price_time)
    }

    /// Compatibility alias for `record_swap`.
    pub fn record_trade(env: Env, key_id: Address, price: i128) -> Result<i128, TwapOracleError> {
        Self::record_swap(env, key_id, price)
    }

    /// Compatibility alias for `record_swap`.
    pub fn update(env: Env, key_id: Address, price: i128) -> Result<i128, TwapOracleError> {
        Self::record_swap(env, key_id, price)
    }

    /// Returns TWAP for the requested window in seconds (e.g. 1h = 3600, 4h = 14400, 24h = 86400).
    ///
    /// - Enforces the minimum observation count guard before TWAP is considered valid.
    /// - Returns `TwapOracleError::TWAPInvalid` if the observation count threshold has not been met.
    /// - Continuous interpolation ensures price manipulation via brief outliers is diluted across the window.
    pub fn get_twap(
        env: Env,
        key_id: Address,
        window_seconds: u64,
    ) -> Result<i128, TwapOracleError> {
        if window_seconds == 0 {
            return Err(TwapOracleError::TWAPInvalid);
        }

        let config = Self::get_config(env.clone(), key_id.clone());
        if window_seconds > config.max_window_seconds {
            return Err(TwapOracleError::TWAPInvalid);
        }

        let obs_key = DataKey::Observations(key_id.clone());
        let observations: Vec<TwapObservation> = env
            .storage()
            .persistent()
            .get(&obs_key)
            .ok_or(TwapOracleError::TWAPInvalid)?;

        // Minimum observation count guard before TWAP is considered valid
        if observations.len() < config.min_observations {
            return Err(TwapOracleError::TWAPInvalid);
        }

        let now = env.ledger().timestamp();
        let latest_obs = observations.get(observations.len() - 1).unwrap();
        if now < latest_obs.timestamp {
            return Err(TwapOracleError::TWAPInvalid);
        }

        let target_time = now.saturating_sub(window_seconds);

        // Compute cumulative price-time at current ledger timestamp `now`
        let latest_delta = (now.saturating_sub(latest_obs.timestamp)) as i128;
        let latest_cumulative = latest_obs
            .cumulative_price_time
            .saturating_add(latest_obs.price.saturating_mul(latest_delta));

        // Compute cumulative price-time at `target_time`
        let first_obs = observations.get(0).unwrap();
        let target_cumulative = if target_time < first_obs.timestamp {
            let delta = (first_obs.timestamp.saturating_sub(target_time)) as i128;
            first_obs
                .cumulative_price_time
                .saturating_sub(first_obs.price.saturating_mul(delta))
        } else {
            let mut target_val = first_obs.cumulative_price_time;
            for i in (0..observations.len()).rev() {
                let obs = observations.get(i).unwrap();
                if obs.timestamp <= target_time {
                    let delta = (target_time.saturating_sub(obs.timestamp)) as i128;
                    target_val = obs
                        .cumulative_price_time
                        .saturating_add(obs.price.saturating_mul(delta));
                    break;
                }
            }
            target_val
        };

        let total_time = (now.saturating_sub(target_time)) as i128;
        if total_time <= 0 {
            return Ok(latest_obs.price);
        }

        let twap = latest_cumulative
            .saturating_sub(target_cumulative)
            .checked_div(total_time)
            .ok_or(TwapOracleError::ArithmeticOverflow)?;

        Ok(twap)
    }

    /// Returns the 1-hour TWAP (3,600 seconds).
    pub fn get_twap_1h(env: Env, key_id: Address) -> Result<i128, TwapOracleError> {
        Self::get_twap(env, key_id, WINDOW_1H)
    }

    /// Returns the 4-hour TWAP (14,400 seconds).
    pub fn get_twap_4h(env: Env, key_id: Address) -> Result<i128, TwapOracleError> {
        Self::get_twap(env, key_id, WINDOW_4H)
    }

    /// Returns the 24-hour TWAP (86,400 seconds).
    pub fn get_twap_24h(env: Env, key_id: Address) -> Result<i128, TwapOracleError> {
        Self::get_twap(env, key_id, WINDOW_24H)
    }

    /// Returns all stored observations for a creator key.
    pub fn get_observations(env: Env, key_id: Address) -> Vec<TwapObservation> {
        let obs_key = DataKey::Observations(key_id);
        env.storage()
            .persistent()
            .get(&obs_key)
            .unwrap_or(Vec::new(&env))
    }

    /// Returns the number of stored observations for a creator key.
    pub fn get_observation_count(env: Env, key_id: Address) -> u32 {
        let obs_key = DataKey::Observations(key_id);
        env.storage()
            .persistent()
            .get::<DataKey, Vec<TwapObservation>>(&obs_key)
            .map(|obs| obs.len())
            .unwrap_or(0)
    }

    /// Returns the latest recorded spot price for a creator key.
    pub fn get_latest_price(env: Env, key_id: Address) -> Result<i128, TwapOracleError> {
        let obs_key = DataKey::Observations(key_id);
        let observations: Vec<TwapObservation> = env
            .storage()
            .persistent()
            .get(&obs_key)
            .ok_or(TwapOracleError::TWAPInvalid)?;
        if observations.is_empty() {
            return Err(TwapOracleError::TWAPInvalid);
        }
        let last_obs = observations.get(observations.len() - 1).unwrap();
        Ok(last_obs.price)
    }

    /// Returns the current cumulative price-time for a creator key at the current ledger timestamp.
    pub fn get_cumulative_price_time(env: Env, key_id: Address) -> Result<i128, TwapOracleError> {
        let obs_key = DataKey::Observations(key_id);
        let observations: Vec<TwapObservation> = env
            .storage()
            .persistent()
            .get(&obs_key)
            .ok_or(TwapOracleError::TWAPInvalid)?;
        if observations.is_empty() {
            return Err(TwapOracleError::TWAPInvalid);
        }
        let now = env.ledger().timestamp();
        let last_obs = observations.get(observations.len() - 1).unwrap();
        let delta = (now.saturating_sub(last_obs.timestamp)) as i128;
        let cumulative = last_obs
            .cumulative_price_time
            .saturating_add(last_obs.price.saturating_mul(delta));
        Ok(cumulative)
    }
}
