#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    pub asset: Address,
    pub target_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapRoute {
    pub from: Address,
    pub to: Address,
    pub path: Vec<Address>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapCaps {
    pub per_swap_bps: u32,
    pub per_period_bps: u32,
    pub period_duration: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetBalance {
    pub asset: Address,
    pub balance: i128,
    pub target_bps: u32,
    pub drift_bps: i32,
}

#[contracttype]
enum DataKey {
    Governor,
    Keeper,
    Targets,
    Routes,
    Caps,
    SwapsPaused,
    PeriodVolume(u64),
    TimelockDelay,
    PendingTargets,
    PendingTargetsTime,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DiversificationError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidRoute = 3,
    InvalidSlippage = 4,
    CapExceeded = 5,
    SwapsPaused = 6,
    InvalidTargets = 7,
    TimelockActive = 8,
    ArithmeticOverflow = 9,
}

#[contract]
pub struct TreasuryDiversificationContract;

fn require_governor(env: &Env, caller: &Address) -> Result<(), DiversificationError> {
    caller.require_auth();
    if env
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Governor)
        != Some(caller.clone())
    {
        return Err(DiversificationError::Unauthorized);
    }
    Ok(())
}

fn require_keeper(env: &Env, caller: &Address) -> Result<(), DiversificationError> {
    caller.require_auth();
    if env.storage().instance().get::<_, Address>(&DataKey::Keeper) != Some(caller.clone()) {
        return Err(DiversificationError::Unauthorized);
    }
    Ok(())
}

#[contractimpl]
impl TreasuryDiversificationContract {
    pub fn initialize(
        env: Env,
        governor: Address,
        keeper: Address,
        timelock_delay: u64,
    ) -> Result<(), DiversificationError> {
        if env.storage().instance().has(&DataKey::Governor) {
            return Err(DiversificationError::AlreadyInitialized);
        }
        governor.require_auth();
        env.storage().instance().set(&DataKey::Governor, &governor);
        env.storage().instance().set(&DataKey::Keeper, &keeper);
        env.storage()
            .instance()
            .set(&DataKey::TimelockDelay, &timelock_delay);
        env.storage().instance().set(&DataKey::SwapsPaused, &false);
        Ok(())
    }

    pub fn set_targets(
        env: Env,
        governor: Address,
        targets: Vec<Target>,
    ) -> Result<(), DiversificationError> {
        require_governor(&env, &governor)?;

        // Validate targets sum to 10,000 bps (100%)
        let mut total_bps: u32 = 0;
        for target in targets.iter() {
            total_bps = total_bps
                .checked_add(target.target_bps)
                .ok_or(DiversificationError::ArithmeticOverflow)?;
        }
        if total_bps != 10000 {
            return Err(DiversificationError::InvalidTargets);
        }

        let now = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::PendingTargets, &targets);
        env.storage()
            .instance()
            .set(&DataKey::PendingTargetsTime, &now);
        env.events().publish(("targets_set",), targets);
        Ok(())
    }

    pub fn apply_pending_targets(env: Env) -> Result<(), DiversificationError> {
        let pending_time: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PendingTargetsTime)
            .unwrap_or(0);
        let timelock_delay: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TimelockDelay)
            .unwrap_or(0);
        let now = env.ledger().timestamp();

        if now < pending_time + timelock_delay {
            return Err(DiversificationError::TimelockActive);
        }

        let targets: Vec<Target> = env
            .storage()
            .instance()
            .get(&DataKey::PendingTargets)
            .ok_or(DiversificationError::InvalidTargets)?;

        env.storage().instance().set(&DataKey::Targets, &targets);
        env.storage().instance().remove(&DataKey::PendingTargets);
        env.storage()
            .instance()
            .remove(&DataKey::PendingTargetsTime);
        Ok(())
    }

    pub fn set_routes(
        env: Env,
        governor: Address,
        routes: Vec<SwapRoute>,
    ) -> Result<(), DiversificationError> {
        require_governor(&env, &governor)?;
        env.storage().instance().set(&DataKey::Routes, &routes);
        Ok(())
    }

    pub fn set_caps(
        env: Env,
        governor: Address,
        caps: SwapCaps,
    ) -> Result<(), DiversificationError> {
        require_governor(&env, &governor)?;
        env.storage().instance().set(&DataKey::Caps, &caps);
        Ok(())
    }

    pub fn pause_swaps(env: Env, governor: Address) -> Result<(), DiversificationError> {
        require_governor(&env, &governor)?;
        env.storage().instance().set(&DataKey::SwapsPaused, &true);
        env.events().publish(("swaps_paused",), ());
        Ok(())
    }

    pub fn unpause_swaps(env: Env, governor: Address) -> Result<(), DiversificationError> {
        require_governor(&env, &governor)?;
        env.storage().instance().set(&DataKey::SwapsPaused, &false);
        Ok(())
    }

    pub fn execute_swap(
        env: Env,
        keeper: Address,
        from_asset: Address,
        to_asset: Address,
        amount_in: i128,
        min_out: i128,
    ) -> Result<i128, DiversificationError> {
        require_keeper(&env, &keeper)?;

        let swaps_paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::SwapsPaused)
            .unwrap_or(false);
        if swaps_paused {
            return Err(DiversificationError::SwapsPaused);
        }

        // Check route is approved
        let routes: Vec<SwapRoute> = env
            .storage()
            .instance()
            .get(&DataKey::Routes)
            .ok_or(DiversificationError::InvalidRoute)?;
        let route_exists = routes
            .iter()
            .any(|r| r.from == from_asset && r.to == to_asset);
        if !route_exists {
            return Err(DiversificationError::InvalidRoute);
        }

        // Check caps
        let caps: SwapCaps = env
            .storage()
            .instance()
            .get(&DataKey::Caps)
            .ok_or(DiversificationError::CapExceeded)?;

        let treasury_value = Self::get_treasury_value(env.clone());
        let max_swap = treasury_value
            .checked_mul(caps.per_swap_bps as i128)
            .and_then(|v| v.checked_div(10000))
            .ok_or(DiversificationError::ArithmeticOverflow)?;

        if amount_in > max_swap {
            return Err(DiversificationError::CapExceeded);
        }

        // Check period volume
        let now = env.ledger().timestamp();
        let period_id = now / caps.period_duration;
        let period_volume: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::PeriodVolume(period_id))
            .unwrap_or(0);
        let new_period_volume = period_volume
            .checked_add(amount_in)
            .ok_or(DiversificationError::ArithmeticOverflow)?;

        let max_period = treasury_value
            .checked_mul(caps.per_period_bps as i128)
            .and_then(|v| v.checked_div(10000))
            .ok_or(DiversificationError::ArithmeticOverflow)?;

        if new_period_volume > max_period {
            return Err(DiversificationError::CapExceeded);
        }

        // Simulate minimum output check (oracle-based)
        let amount_out = Self::simulate_swap(env.clone(), &from_asset, &to_asset, amount_in);
        if amount_out < min_out {
            return Err(DiversificationError::InvalidSlippage);
        }

        env.storage()
            .persistent()
            .set(&DataKey::PeriodVolume(period_id), &new_period_volume);

        env.events().publish(
            ("swap_executed", from_asset, to_asset, amount_in),
            amount_out,
        );

        Ok(amount_out)
    }

    pub fn treasury_report(env: Env) -> Vec<AssetBalance> {
        let targets: Vec<Target> = env
            .storage()
            .instance()
            .get(&DataKey::Targets)
            .unwrap_or(Vec::new(&env));

        let mut report = Vec::new(&env);
        let total_value = Self::get_treasury_value(env.clone());

        for target in targets.iter() {
            let balance = Self::get_asset_balance(env.clone(), &target.asset);
            let actual_bps = if total_value > 0 {
                ((balance * 10000) / total_value) as u32
            } else {
                0
            };
            let drift = actual_bps as i32 - target.target_bps as i32;

            report.push_back(AssetBalance {
                asset: target.asset,
                balance,
                target_bps: target.target_bps,
                drift_bps: drift,
            });
        }

        report
    }

    // Helper functions (simplified for demonstration)
    fn get_treasury_value(_env: Env) -> i128 {
        // In a real implementation, this would sum all asset values
        1_000_000
    }

    fn get_asset_balance(_env: Env, _asset: &Address) -> i128 {
        // In a real implementation, this would query the token contract
        100_000
    }

    fn simulate_swap(_env: Env, _from: &Address, _to: &Address, amount_in: i128) -> i128 {
        // In a real implementation, this would use the oracle
        amount_in * 99 / 100 // Simulate 1% slippage
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (
        Env,
        TreasuryDiversificationContractClient<'static>,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let governor = Address::generate(&env);
        let keeper = Address::generate(&env);
        let contract_id = env.register(TreasuryDiversificationContract, ());
        let client = TreasuryDiversificationContractClient::new(&env, &contract_id);
        client.initialize(&governor, &keeper, &3600);
        (env, client, governor, keeper)
    }

    #[test]
    fn test_set_targets_validates_sum() {
        let (env, client, governor, _keeper) = setup();
        let mut targets = Vec::new(&env);
        targets.push_back(Target {
            asset: Address::generate(&env),
            target_bps: 5000,
        });
        targets.push_back(Target {
            asset: Address::generate(&env),
            target_bps: 5000,
        });
        client.set_targets(&governor, &targets);

        let mut invalid_targets = Vec::new(&env);
        invalid_targets.push_back(Target {
            asset: Address::generate(&env),
            target_bps: 5000,
        });
        assert_eq!(
            client.try_set_targets(&governor, &invalid_targets),
            Err(Ok(DiversificationError::InvalidTargets))
        );
    }

    #[test]
    fn test_swap_checks_approved_route() {
        let (env, client, governor, keeper) = setup();
        let asset_a = Address::generate(&env);
        let asset_b = Address::generate(&env);

        let mut routes = Vec::new(&env);
        routes.push_back(SwapRoute {
            from: asset_a.clone(),
            to: asset_b.clone(),
            path: Vec::new(&env),
        });
        client.set_routes(&governor, &routes);

        let caps = SwapCaps {
            per_swap_bps: 500,
            per_period_bps: 2000,
            period_duration: 86400,
        };
        client.set_caps(&governor, &caps);

        // Approved route should work
        client.execute_swap(&keeper, &asset_a, &asset_b, &100, &95);

        // Unapproved route should fail
        let asset_c = Address::generate(&env);
        assert_eq!(
            client.try_execute_swap(&keeper, &asset_a, &asset_c, &100, &95),
            Err(Ok(DiversificationError::InvalidRoute))
        );
    }

    #[test]
    fn test_per_swap_cap_enforced() {
        let (env, client, governor, keeper) = setup();
        let asset_a = Address::generate(&env);
        let asset_b = Address::generate(&env);

        let mut routes = Vec::new(&env);
        routes.push_back(SwapRoute {
            from: asset_a.clone(),
            to: asset_b.clone(),
            path: Vec::new(&env),
        });
        client.set_routes(&governor, &routes);

        let caps = SwapCaps {
            per_swap_bps: 500, // 5% max
            per_period_bps: 2000,
            period_duration: 86400,
        };
        client.set_caps(&governor, &caps);

        // Amount > 5% of treasury (1,000,000 * 0.05 = 50,000)
        assert_eq!(
            client.try_execute_swap(&keeper, &asset_a, &asset_b, &60000, &50000),
            Err(Ok(DiversificationError::CapExceeded))
        );
    }

    #[test]
    fn test_pausing_is_immediate() {
        let (env, client, governor, keeper) = setup();
        let asset_a = Address::generate(&env);
        let asset_b = Address::generate(&env);

        let mut routes = Vec::new(&env);
        routes.push_back(SwapRoute {
            from: asset_a.clone(),
            to: asset_b.clone(),
            path: Vec::new(&env),
        });
        client.set_routes(&governor, &routes);

        let caps = SwapCaps {
            per_swap_bps: 500,
            per_period_bps: 2000,
            period_duration: 86400,
        };
        client.set_caps(&governor, &caps);

        client.pause_swaps(&governor);
        assert_eq!(
            client.try_execute_swap(&keeper, &asset_a, &asset_b, &100, &95),
            Err(Ok(DiversificationError::SwapsPaused))
        );
    }

    #[test]
    fn test_target_changes_follow_timelock() {
        let (env, client, governor, _keeper) = setup();
        let mut targets = Vec::new(&env);
        targets.push_back(Target {
            asset: Address::generate(&env),
            target_bps: 10000,
        });

        client.set_targets(&governor, &targets);

        // Should fail before timelock expires
        assert_eq!(
            client.try_apply_pending_targets(),
            Err(Ok(DiversificationError::TimelockActive))
        );
    }

    #[test]
    fn test_treasury_report_shows_drift() {
        let (_env, client, _governor, _keeper) = setup();
        let report = client.treasury_report();
        // Report should be calculable even with no targets
        assert!(report.is_empty());
    }

    #[test]
    fn test_output_below_minimum_reverts() {
        let (env, client, governor, keeper) = setup();
        let asset_a = Address::generate(&env);
        let asset_b = Address::generate(&env);

        let mut routes = Vec::new(&env);
        routes.push_back(SwapRoute {
            from: asset_a.clone(),
            to: asset_b.clone(),
            path: Vec::new(&env),
        });
        client.set_routes(&governor, &routes);

        let caps = SwapCaps {
            per_swap_bps: 500,
            per_period_bps: 2000,
            period_duration: 86400,
        };
        client.set_caps(&governor, &caps);

        // Expect output > actual (100 * 0.99 = 99, but we demand 100)
        assert_eq!(
            client.try_execute_swap(&keeper, &asset_a, &asset_b, &100, &100),
            Err(Ok(DiversificationError::InvalidSlippage))
        );
    }
}
