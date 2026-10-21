//! Linear vesting release calculation with cliff enforcement (issue #916).
//!
//! Pure module — no `Env`, no storage — so the release math is unit-testable
//! in isolation, mirroring the `fee` module's `compute_fee_split` approach.

use soroban_sdk::contracttype;

/// Persisted vesting configuration for a single beneficiary.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestingConfig {
    pub total_allocation: i128,
    pub cliff_timestamp: u64,
    pub start_timestamp: u64,
    pub duration_secs: u64,
    pub claimed_amount: i128,
}

/// Aggregated read-only view returned by `get_vesting_info`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestingInfo {
    pub total_allocation: i128,
    pub vested_amount: i128,
    pub claimed_amount: i128,
    pub claimable_amount: i128,
    pub cliff_timestamp: u64,
    pub start_timestamp: u64,
    pub duration_secs: u64,
    pub is_fully_vested: bool,
}

/// Computes total unlocked so far (ignoring what's already claimed).
///
/// - `0` before `cliff_timestamp`.
/// - Linear from `start_timestamp` once past the cliff.
/// - Full allocation once `elapsed >= duration_secs`.
/// - `duration_secs == 0` fully vests exactly at the cliff.
pub fn vested_amount(config: &VestingConfig, current_timestamp: u64) -> i128 {
    if config.total_allocation <= 0 {
        return 0;
    }
    if current_timestamp < config.cliff_timestamp {
        return 0;
    }
    if config.duration_secs == 0 {
        return config.total_allocation;
    }

    let elapsed = current_timestamp.saturating_sub(config.start_timestamp);
    if elapsed >= config.duration_secs {
        return config.total_allocation;
    }

    (config.total_allocation.saturating_mul(i128::from(elapsed))) / i128::from(config.duration_secs)
}

/// Vested minus already claimed, floored at zero.
pub fn claimable_amount(config: &VestingConfig, current_timestamp: u64) -> i128 {
    vested_amount(config, current_timestamp)
        .saturating_sub(config.claimed_amount)
        .max(0)
}

/// Returns all vesting state in one call (issue #916 requirement).
pub fn get_vesting_info(config: &VestingConfig, current_timestamp: u64) -> VestingInfo {
    let vested = vested_amount(config, current_timestamp);
    let claimable = vested.saturating_sub(config.claimed_amount).max(0);
    VestingInfo {
        total_allocation: config.total_allocation,
        vested_amount: vested,
        claimed_amount: config.claimed_amount,
        claimable_amount: claimable,
        cliff_timestamp: config.cliff_timestamp,
        start_timestamp: config.start_timestamp,
        duration_secs: config.duration_secs,
        is_fully_vested: vested >= config.total_allocation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> VestingConfig {
        VestingConfig {
            total_allocation: 10_000,
            cliff_timestamp: 1_000,
            start_timestamp: 0,
            duration_secs: 10_000,
            claimed_amount: 0,
        }
    }

    #[test]
    fn test_vested_amount_returns_zero_before_cliff() {
        let config = base_config();
        assert_eq!(vested_amount(&config, 0), 0);
        assert_eq!(vested_amount(&config, 999), 0);
    }

    #[test]
    fn test_vested_amount_at_cliff_uses_linear_formula() {
        let config = base_config();
        assert_eq!(vested_amount(&config, 1_000), 1_000);
    }

    #[test]
    fn test_vested_amount_mid_vesting_is_linear() {
        let config = base_config();
        assert_eq!(vested_amount(&config, 5_000), 5_000);
        assert_eq!(vested_amount(&config, 7_500), 7_500);
    }

    #[test]
    fn test_vested_amount_fully_vested_after_duration() {
        let config = base_config();
        assert_eq!(vested_amount(&config, 10_000), 10_000);
        assert_eq!(vested_amount(&config, 999_999), 10_000);
    }

    #[test]
    fn test_vested_amount_zero_duration_fully_vests_at_cliff() {
        let config = VestingConfig {
            duration_secs: 0,
            ..base_config()
        };
        assert_eq!(vested_amount(&config, 0), 0);
        assert_eq!(vested_amount(&config, 1_000), 10_000);
    }

    #[test]
    fn test_vested_amount_zero_allocation_is_always_zero() {
        let config = VestingConfig {
            total_allocation: 0,
            ..base_config()
        };
        assert_eq!(vested_amount(&config, 10_000), 0);
    }

    #[test]
    fn test_claimable_amount_subtracts_already_claimed() {
        let config = VestingConfig {
            claimed_amount: 3_000,
            ..base_config()
        };
        assert_eq!(claimable_amount(&config, 5_000), 2_000);
    }

    #[test]
    fn test_claimable_amount_never_negative() {
        let config = VestingConfig {
            claimed_amount: 9_999,
            ..base_config()
        };
        assert_eq!(claimable_amount(&config, 0), 0);
    }

    #[test]
    fn test_get_vesting_info_pre_cliff() {
        let config = base_config();
        let info = get_vesting_info(&config, 500);
        assert_eq!(info.vested_amount, 0);
        assert_eq!(info.claimable_amount, 0);
        assert!(!info.is_fully_vested);
    }

    #[test]
    fn test_get_vesting_info_mid_vesting() {
        let config = VestingConfig {
            claimed_amount: 1_000,
            ..base_config()
        };
        let info = get_vesting_info(&config, 5_000);
        assert_eq!(info.vested_amount, 5_000);
        assert_eq!(info.claimable_amount, 4_000);
        assert!(!info.is_fully_vested);
    }

    #[test]
    fn test_get_vesting_info_fully_vested() {
        let config = VestingConfig {
            claimed_amount: 10_000,
            ..base_config()
        };
        let info = get_vesting_info(&config, 20_000);
        assert_eq!(info.claimable_amount, 0);
        assert!(info.is_fully_vested);
    }

    #[test]
    fn test_vested_amount_no_rounding_errors_at_exact_duration_fraction() {
        let config = VestingConfig {
            total_allocation: 9_999,
            cliff_timestamp: 0,
            start_timestamp: 0,
            duration_secs: 3,
            claimed_amount: 0,
        };
        assert_eq!(vested_amount(&config, 1), 3_333);
        assert_eq!(vested_amount(&config, 2), 6_666);
        assert_eq!(vested_amount(&config, 3), 9_999);
    }
}
