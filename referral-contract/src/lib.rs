#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

/// Event name emitted when a referrer's tier is upgraded.
pub const REFERRAL_TIER_UPGRADED_EVENT_NAME: Symbol = symbol_short!("tier_upg");

/// Event name emitted when referral rewards are accrued.
pub const REFERRAL_REWARD_ACCRUED_EVENT_NAME: Symbol = symbol_short!("ref_rwd");

/// Event name emitted when referral rewards are claimed.
pub const REFERRAL_REWARDS_CLAIMED_EVENT_NAME: Symbol = symbol_short!("ref_clm");

/// Basis points scale (10,000 bps = 100%).
pub const BPS_SCALE: i128 = 10_000;

/// Default volume threshold for Bronze tier.
pub const DEFAULT_BRONZE_THRESHOLD: i128 = 0;
/// Default reward rate for Bronze tier: 500 bps (5.00%).
pub const DEFAULT_BRONZE_BPS: u32 = 500;

/// Default volume threshold for Silver tier.
pub const DEFAULT_SILVER_THRESHOLD: i128 = 100_000;
/// Default reward rate for Silver tier: 1,000 bps (10.00%).
pub const DEFAULT_SILVER_BPS: u32 = 1_000;

/// Default volume threshold for Gold tier.
pub const DEFAULT_GOLD_THRESHOLD: i128 = 1_000_000;
/// Default reward rate for Gold tier: 1,500 bps (15.00%).
pub const DEFAULT_GOLD_BPS: u32 = 1_500;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialised = 1,
    Unauthorized = 2,
    InvalidRate = 3,
    InvalidThreshold = 4,
    NotPositiveAmount = 5,
    ArithmeticOverflow = 6,
    NoRewardsPending = 7,
}

/// Referral tiers in ascending rank and reward rate.
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[contracttype]
#[repr(u32)]
pub enum ReferralTier {
    Bronze = 0,
    Silver = 1,
    Gold = 2,
}

/// Configuration for a referral tier.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TierConfig {
    pub threshold: i128,
    pub reward_rate_bps: u32,
}

/// Information about a referrer's current tier, cumulative volume, and next threshold.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralTierInfo {
    pub tier: ReferralTier,
    pub volume: i128,
    pub next_threshold: Option<i128>,
}

/// Event data payload emitted when a referrer's tier is upgraded.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralTierUpgraded {
    pub referrer: Address,
    pub old_tier: ReferralTier,
    pub new_tier: ReferralTier,
    pub volume: i128,
    pub reward_rate_bps: u32,
}

pub type ReferralTierUpgradedEvent = ReferralTierUpgraded;

/// Event data payload emitted when referral rewards are accrued.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralRewardAccrued {
    pub referrer: Address,
    pub amount: i128,
    pub tier: ReferralTier,
    pub reward_rate_bps: u32,
}

/// Event data payload emitted when referral rewards are claimed.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralRewardsClaimed {
    pub referrer: Address,
    pub amount: i128,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    FirstTradeStatus(Address),
    Initialised,
    Admin,
    ReferrerVolume(Address),
    ReferrerTier(Address),
    ReferrerRewards(Address),
    TierConfig(ReferralTier),
}

#[contract]
pub struct ReferralContract;

#[contractimpl]
impl ReferralContract {
    /// Initializes the referral contract.
    pub fn init(env: Env) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Initialised) {
            return Err(ContractError::AlreadyInitialised);
        }

        env.storage().instance().set(&DataKey::Initialised, &true);
        env.events().publish((symbol_short!("init"),), ());

        Ok(())
    }

    /// Initializes the referral contract with a designated admin.
    pub fn init_with_admin(env: Env, admin: Address) -> Result<(), ContractError> {
        Self::init(env.clone())?;
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Sets or updates the contract admin.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        if let Some(admin) = env.storage().instance().get::<_, Address>(&DataKey::Admin) {
            admin.require_auth();
        }
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }

    /// Returns the contract admin, if configured.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    /// Returns true if the contract has been initialised.
    pub fn get_initialised(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Initialised)
            .unwrap_or(false)
    }

    /// Returns true if it was indeed the first trade (and records it as true now).
    pub fn record_first_trade(env: Env, wallet: Address) -> bool {
        let key = DataKey::FirstTradeStatus(wallet.clone());
        if env.storage().persistent().has(&key) {
            false
        } else {
            env.storage().persistent().set(&key, &true);
            true
        }
    }

    /// Configures the volume threshold and reward rate bps for a tier.
    pub fn set_tier_config(
        env: Env,
        admin: Address,
        tier: ReferralTier,
        threshold: i128,
        reward_rate_bps: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        if let Some(current_admin) = env.storage().instance().get::<_, Address>(&DataKey::Admin) {
            if admin != current_admin {
                return Err(ContractError::Unauthorized);
            }
        } else {
            env.storage().instance().set(&DataKey::Admin, &admin);
        }

        if threshold < 0 {
            return Err(ContractError::InvalidThreshold);
        }
        if reward_rate_bps > BPS_SCALE as u32 {
            return Err(ContractError::InvalidRate);
        }

        let config = TierConfig {
            threshold,
            reward_rate_bps,
        };
        env.storage()
            .instance()
            .set(&DataKey::TierConfig(tier), &config);

        Ok(())
    }

    /// Returns the configuration for a given tier.
    pub fn get_tier_config(env: Env, tier: ReferralTier) -> TierConfig {
        get_tier_config_internal(&env, tier)
    }

    /// Returns the volume threshold for a given tier.
    pub fn get_tier_threshold(env: Env, tier: ReferralTier) -> i128 {
        get_tier_config_internal(&env, tier).threshold
    }

    /// Returns the reward rate bps for a given tier.
    pub fn get_tier_reward_rate_bps(env: Env, tier: ReferralTier) -> u32 {
        get_tier_config_internal(&env, tier).reward_rate_bps
    }

    /// Returns a referrer's current tier, cumulative volume, and next tier threshold.
    /// Tiers are ratchet-only and never downgrade.
    pub fn get_referral_tier(env: Env, referrer: Address) -> ReferralTierInfo {
        let volume: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::ReferrerVolume(referrer.clone()))
            .unwrap_or(0);

        let stored_ratchet: ReferralTier = env
            .storage()
            .persistent()
            .get(&DataKey::ReferrerTier(referrer))
            .unwrap_or(ReferralTier::Bronze);

        let earned_tier = compute_tier_from_volume(&env, volume);
        let current_tier = if earned_tier > stored_ratchet {
            earned_tier
        } else {
            stored_ratchet
        };

        let next_threshold = match current_tier {
            ReferralTier::Bronze => {
                Some(get_tier_config_internal(&env, ReferralTier::Silver).threshold)
            }
            ReferralTier::Silver => {
                Some(get_tier_config_internal(&env, ReferralTier::Gold).threshold)
            }
            ReferralTier::Gold => None,
        };

        ReferralTierInfo {
            tier: current_tier,
            volume,
            next_threshold,
        }
    }

    /// Returns the cumulative referred trade volume for a referrer.
    pub fn get_referrer_volume(env: Env, referrer: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::ReferrerVolume(referrer))
            .unwrap_or(0)
    }

    /// Records trade volume for a referrer, upgrading the tier if a threshold is crossed.
    /// Emits ReferralTierUpgraded when an upgrade occurs.
    pub fn record_trade(
        env: Env,
        referrer: Address,
        trade_volume: i128,
    ) -> Result<ReferralTier, ContractError> {
        if trade_volume <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let volume_key = DataKey::ReferrerVolume(referrer.clone());
        let current_volume: i128 = env.storage().persistent().get(&volume_key).unwrap_or(0);
        let new_volume = current_volume
            .checked_add(trade_volume)
            .ok_or(ContractError::ArithmeticOverflow)?;

        env.storage().persistent().set(&volume_key, &new_volume);

        let active_tier = maybe_upgrade_tier(&env, &referrer, new_volume);
        Ok(active_tier)
    }

    /// Convenience alias for record_trade.
    pub fn record_trade_volume(
        env: Env,
        referrer: Address,
        trade_volume: i128,
    ) -> Result<ReferralTier, ContractError> {
        Self::record_trade(env, referrer, trade_volume)
    }

    /// Adjusts cumulative volume (e.g. For trade cancellation/refunds).
    /// Guarantees ratchet-only behavior: Tiers NEVER downgrade regardless of volume drop.
    pub fn adjust_volume(
        env: Env,
        referrer: Address,
        delta: i128,
    ) -> Result<ReferralTierInfo, ContractError> {
        let volume_key = DataKey::ReferrerVolume(referrer.clone());
        let current_volume: i128 = env.storage().persistent().get(&volume_key).unwrap_or(0);
        let new_volume = current_volume
            .checked_add(delta)
            .ok_or(ContractError::ArithmeticOverflow)?;

        if new_volume < 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        env.storage().persistent().set(&volume_key, &new_volume);
        maybe_upgrade_tier(&env, &referrer, new_volume);

        Ok(Self::get_referral_tier(env, referrer))
    }

    /// Accrues referral reward from a fee amount based on the referrer's current tier rate.
    pub fn accrue_reward(
        env: Env,
        referrer: Address,
        fee_amount: i128,
    ) -> Result<i128, ContractError> {
        if fee_amount <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let tier_info = Self::get_referral_tier(env.clone(), referrer.clone());
        let rate_bps = get_tier_config_internal(&env, tier_info.tier).reward_rate_bps;

        let reward = (fee_amount
            .checked_mul(rate_bps as i128)
            .ok_or(ContractError::ArithmeticOverflow)?)
            / BPS_SCALE;

        if reward > 0 {
            let rewards_key = DataKey::ReferrerRewards(referrer.clone());
            let current_rewards: i128 = env.storage().persistent().get(&rewards_key).unwrap_or(0);
            let new_rewards = current_rewards
                .checked_add(reward)
                .ok_or(ContractError::ArithmeticOverflow)?;

            env.storage().persistent().set(&rewards_key, &new_rewards);

            env.events().publish(
                (REFERRAL_REWARD_ACCRUED_EVENT_NAME, referrer.clone()),
                ReferralRewardAccrued {
                    referrer,
                    amount: reward,
                    tier: tier_info.tier,
                    reward_rate_bps: rate_bps,
                },
            );
        }

        Ok(reward)
    }

    /// Records trade volume and accrues the referral fee in a single atomic call.
    pub fn accrue_referral_reward(
        env: Env,
        referrer: Address,
        trade_volume: i128,
        fee_amount: i128,
    ) -> Result<i128, ContractError> {
        Self::record_trade(env.clone(), referrer.clone(), trade_volume)?;
        Self::accrue_reward(env, referrer, fee_amount)
    }

    /// Calculates the potential reward for a given fee amount using the referrer's current tier rate.
    pub fn calculate_reward(env: Env, referrer: Address, fee_amount: i128) -> i128 {
        if fee_amount <= 0 {
            return 0;
        }
        let tier_info = Self::get_referral_tier(env.clone(), referrer);
        let rate_bps = get_tier_config_internal(&env, tier_info.tier).reward_rate_bps;
        (fee_amount * rate_bps as i128) / BPS_SCALE
    }

    /// Returns accumulated referral rewards for a referrer.
    pub fn get_referral_earnings(env: Env, referrer: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::ReferrerRewards(referrer))
            .unwrap_or(0)
    }

    /// Referrer claims all accumulated rewards, resetting the pending balance to 0.
    pub fn claim_referral_rewards(env: Env, referrer: Address) -> Result<i128, ContractError> {
        referrer.require_auth();

        let rewards_key = DataKey::ReferrerRewards(referrer.clone());
        let amount: i128 = env.storage().persistent().get(&rewards_key).unwrap_or(0);
        if amount <= 0 {
            return Err(ContractError::NoRewardsPending);
        }

        env.storage().persistent().set(&rewards_key, &0i128);

        env.events().publish(
            (REFERRAL_REWARDS_CLAIMED_EVENT_NAME, referrer.clone()),
            ReferralRewardsClaimed { referrer, amount },
        );

        Ok(amount)
    }
}

/// Internal helper: Reads tier config from instance storage, falling back to canonical defaults.
fn get_tier_config_internal(env: &Env, tier: ReferralTier) -> TierConfig {
    if let Some(config) = env.storage().instance().get(&DataKey::TierConfig(tier)) {
        config
    } else {
        match tier {
            ReferralTier::Bronze => TierConfig {
                threshold: DEFAULT_BRONZE_THRESHOLD,
                reward_rate_bps: DEFAULT_BRONZE_BPS,
            },
            ReferralTier::Silver => TierConfig {
                threshold: DEFAULT_SILVER_THRESHOLD,
                reward_rate_bps: DEFAULT_SILVER_BPS,
            },
            ReferralTier::Gold => TierConfig {
                threshold: DEFAULT_GOLD_THRESHOLD,
                reward_rate_bps: DEFAULT_GOLD_BPS,
            },
        }
    }
}

/// Computes the earned tier from cumulative trade volume.
fn compute_tier_from_volume(env: &Env, volume: i128) -> ReferralTier {
    let gold_threshold = get_tier_config_internal(env, ReferralTier::Gold).threshold;
    let silver_threshold = get_tier_config_internal(env, ReferralTier::Silver).threshold;

    if volume >= gold_threshold {
        ReferralTier::Gold
    } else if volume >= silver_threshold {
        ReferralTier::Silver
    } else {
        ReferralTier::Bronze
    }
}

/// Checks if a new volume qualifies for a tier upgrade.
/// Ratchet-only: Tiers never downgrade. Emits ReferralTierUpgraded on upgrade.
fn maybe_upgrade_tier(env: &Env, referrer: &Address, volume: i128) -> ReferralTier {
    let tier_key = DataKey::ReferrerTier(referrer.clone());
    let stored_tier: ReferralTier = env
        .storage()
        .persistent()
        .get(&tier_key)
        .unwrap_or(ReferralTier::Bronze);

    let earned_tier = compute_tier_from_volume(env, volume);

    if earned_tier > stored_tier {
        env.storage().persistent().set(&tier_key, &earned_tier);
        let rate_bps = get_tier_config_internal(env, earned_tier).reward_rate_bps;

        env.events().publish(
            (REFERRAL_TIER_UPGRADED_EVENT_NAME, referrer.clone()),
            ReferralTierUpgraded {
                referrer: referrer.clone(),
                old_tier: stored_tier,
                new_tier: earned_tier,
                volume,
                reward_rate_bps: rate_bps,
            },
        );

        earned_tier
    } else {
        stored_tier
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
    fn test_initialisation() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        assert!(!client.get_initialised());
        client.init();
        assert!(client.get_initialised());
    }

    #[test]
    fn test_double_initialisation() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        client.init();
        let result = client.try_init();
        assert_eq!(result, Err(Ok(ContractError::AlreadyInitialised)));
    }

    #[test]
    fn test_record_first_trade() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        assert!(client.record_first_trade(&user));
        assert!(!client.record_first_trade(&user));
    }

    #[test]
    fn test_default_tier_is_bronze() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        let referrer = Address::generate(&env);
        let info = client.get_referral_tier(&referrer);

        assert_eq!(info.tier, ReferralTier::Bronze);
        assert_eq!(info.volume, 0);
        assert_eq!(info.next_threshold, Some(DEFAULT_SILVER_THRESHOLD));
    }

    #[test]
    fn test_tier_computed_from_cumulative_volume() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        let referrer = Address::generate(&env);

        // Sub-silver volume stays Bronze
        client.record_trade(&referrer, &50_000);
        let info = client.get_referral_tier(&referrer);
        assert_eq!(info.tier, ReferralTier::Bronze);
        assert_eq!(info.volume, 50_000);
        assert_eq!(info.next_threshold, Some(DEFAULT_SILVER_THRESHOLD));

        // Reaching silver threshold upgrades to Silver
        client.record_trade(&referrer, &50_000);
        let info = client.get_referral_tier(&referrer);
        assert_eq!(info.tier, ReferralTier::Silver);
        assert_eq!(info.volume, 100_000);
        assert_eq!(info.next_threshold, Some(DEFAULT_GOLD_THRESHOLD));

        // Volume between silver and gold stays Silver
        client.record_trade(&referrer, &400_000);
        let info = client.get_referral_tier(&referrer);
        assert_eq!(info.tier, ReferralTier::Silver);
        assert_eq!(info.volume, 500_000);
        assert_eq!(info.next_threshold, Some(DEFAULT_GOLD_THRESHOLD));

        // Crossing gold threshold upgrades to Gold
        client.record_trade(&referrer, &500_000);
        let info = client.get_referral_tier(&referrer);
        assert_eq!(info.tier, ReferralTier::Gold);
        assert_eq!(info.volume, 1_000_000);
        assert_eq!(info.next_threshold, None);
    }

    #[test]
    fn test_tier_direct_jump_from_bronze_to_gold() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        let referrer = Address::generate(&env);
        // Single trade of 2,000,000 jumps directly to Gold
        let tier = client.record_trade(&referrer, &2_000_000);
        assert_eq!(tier, ReferralTier::Gold);

        let info = client.get_referral_tier(&referrer);
        assert_eq!(info.tier, ReferralTier::Gold);
        assert_eq!(info.volume, 2_000_000);
        assert_eq!(info.next_threshold, None);
    }

    #[test]
    fn test_tiers_never_downgrade_regardless_of_volume_drop() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        let referrer = Address::generate(&env);
        client.record_trade(&referrer, &1_500_000);

        let info = client.get_referral_tier(&referrer);
        assert_eq!(info.tier, ReferralTier::Gold);

        // Volume drops below gold and below silver via adjustment (e.g. Refund/cancellation)
        client.adjust_volume(&referrer, &-1_450_000);
        let info_after = client.get_referral_tier(&referrer);

        assert_eq!(info_after.volume, 50_000);
        // Ratchet-only: Tier remains Gold even with volume at 50,000!
        assert_eq!(info_after.tier, ReferralTier::Gold);
        assert_eq!(info_after.next_threshold, None);
    }

    #[test]
    fn test_reward_rate_applied_correctly_per_tier_on_accrual() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        let referrer = Address::generate(&env);
        let fee = 10_000i128;

        // Bronze tier: 500 bps = 5% of 10,000 = 500
        let reward1 = client.accrue_reward(&referrer, &fee);
        assert_eq!(reward1, 500);
        assert_eq!(client.get_referral_earnings(&referrer), 500);

        // Upgrade to Silver
        client.record_trade(&referrer, &100_000);
        // Silver tier: 1000 bps = 10% of 10,000 = 1000
        let reward2 = client.accrue_reward(&referrer, &fee);
        assert_eq!(reward2, 1_000);
        assert_eq!(client.get_referral_earnings(&referrer), 1_500);

        // Upgrade to Gold
        client.record_trade(&referrer, &900_000);
        // Gold tier: 1500 bps = 15% of 10,000 = 1500
        let reward3 = client.accrue_reward(&referrer, &fee);
        assert_eq!(reward3, 1_500);
        assert_eq!(client.get_referral_earnings(&referrer), 3_000);
    }

    #[test]
    fn test_accrue_referral_reward_combined_flow() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        let referrer = Address::generate(&env);

        // Trade volume: 150_000 (crosses Silver threshold)
        // Fee: 20_000
        // Because volume crosses Silver before reward calculation, Silver rate (10%) applies: 2,000
        let reward = client.accrue_referral_reward(&referrer, &150_000, &20_000);
        assert_eq!(reward, 2_000);
        assert_eq!(client.get_referral_earnings(&referrer), 2_000);

        let tier_info = client.get_referral_tier(&referrer);
        assert_eq!(tier_info.tier, ReferralTier::Silver);
        assert_eq!(tier_info.volume, 150_000);
    }

    #[test]
    fn test_claim_referral_rewards() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        let referrer = Address::generate(&env);
        client.accrue_reward(&referrer, &10_000);

        assert_eq!(client.get_referral_earnings(&referrer), 500);
        let claimed = client.claim_referral_rewards(&referrer);
        assert_eq!(claimed, 500);
        assert_eq!(client.get_referral_earnings(&referrer), 0);

        // Second claim fails when no rewards pending
        let err = client.try_claim_referral_rewards(&referrer);
        assert_eq!(err, Err(Ok(ContractError::NoRewardsPending)));
    }

    #[test]
    fn test_custom_tier_configuration() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let referrer = Address::generate(&env);

        // Custom config: Silver at 50,000 volume with 2,000 bps (20%)
        client.set_tier_config(&admin, &ReferralTier::Silver, &50_000, &2_000);

        let cfg = client.get_tier_config(&ReferralTier::Silver);
        assert_eq!(cfg.threshold, 50_000);
        assert_eq!(cfg.reward_rate_bps, 2_000);

        // 50,000 now unlocks Silver
        client.record_trade(&referrer, &50_000);
        let info = client.get_referral_tier(&referrer);
        assert_eq!(info.tier, ReferralTier::Silver);

        // Reward uses 2,000 bps (20%)
        let reward = client.accrue_reward(&referrer, &10_000);
        assert_eq!(reward, 2_000);
    }

    #[test]
    fn test_input_validations() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let referrer = Address::generate(&env);

        // Zero or negative trade volume
        assert_eq!(
            client.try_record_trade(&referrer, &0),
            Err(Ok(ContractError::NotPositiveAmount))
        );
        assert_eq!(
            client.try_record_trade(&referrer, &-100),
            Err(Ok(ContractError::NotPositiveAmount))
        );

        // Zero or negative fee
        assert_eq!(
            client.try_accrue_reward(&referrer, &0),
            Err(Ok(ContractError::NotPositiveAmount))
        );
        assert_eq!(
            client.try_accrue_reward(&referrer, &-50),
            Err(Ok(ContractError::NotPositiveAmount))
        );

        // Negative threshold
        assert_eq!(
            client.try_set_tier_config(&admin, &ReferralTier::Silver, &-1, &1000),
            Err(Ok(ContractError::InvalidThreshold))
        );

        // Rate > 10,000 bps
        assert_eq!(
            client.try_set_tier_config(&admin, &ReferralTier::Silver, &100_000, &10_001),
            Err(Ok(ContractError::InvalidRate))
        );
    }

    #[test]
    fn test_tier_upgrade_emits_event() {
        use soroban_sdk::TryFromVal;

        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);

        let referrer = Address::generate(&env);

        // Sub-threshold trade does not trigger tier upgrade, so no tier_upg event
        client.record_trade(&referrer, &50_000);
        let mut count = 0;
        for (_, topics, _) in env.events().all().iter() {
            if let Some(first) = topics.get(0) {
                if Symbol::try_from_val(&env, &first) == Ok(REFERRAL_TIER_UPGRADED_EVENT_NAME) {
                    count += 1;
                }
            }
        }
        assert_eq!(count, 0);

        // Crossing Silver threshold triggers upgrade and emits ReferralTierUpgraded
        client.record_trade(&referrer, &50_000);
        let mut matching = soroban_sdk::Vec::new(&env);
        for (_, topics, data) in env.events().all().iter() {
            if let Some(first) = topics.get(0) {
                if Symbol::try_from_val(&env, &first) == Ok(REFERRAL_TIER_UPGRADED_EVENT_NAME) {
                    if let Ok(event) = ReferralTierUpgraded::try_from_val(&env, &data) {
                        matching.push_back(event);
                    }
                }
            }
        }

        assert_eq!(matching.len(), 1);
        let event = matching.get(0).unwrap();
        assert_eq!(event.referrer, referrer);
        assert_eq!(event.old_tier, ReferralTier::Bronze);
        assert_eq!(event.new_tier, ReferralTier::Silver);
        assert_eq!(event.volume, 100_000);
        assert_eq!(event.reward_rate_bps, DEFAULT_SILVER_BPS);

        // Upgrading to Gold emits an upgrade event for the latest invocation
        client.record_trade(&referrer, &900_000);
        let mut gold_matching = soroban_sdk::Vec::new(&env);
        for (_, topics, data) in env.events().all().iter() {
            if let Some(first) = topics.get(0) {
                if Symbol::try_from_val(&env, &first) == Ok(REFERRAL_TIER_UPGRADED_EVENT_NAME) {
                    if let Ok(event) = ReferralTierUpgraded::try_from_val(&env, &data) {
                        gold_matching.push_back(event);
                    }
                }
            }
        }

        assert_eq!(gold_matching.len(), 1);
        let gold_event = gold_matching.get(0).unwrap();
        assert_eq!(gold_event.referrer, referrer);
        assert_eq!(gold_event.old_tier, ReferralTier::Silver);
        assert_eq!(gold_event.new_tier, ReferralTier::Gold);
        assert_eq!(gold_event.volume, 1_000_000);
        assert_eq!(gold_event.reward_rate_bps, DEFAULT_GOLD_BPS);
    }
}
