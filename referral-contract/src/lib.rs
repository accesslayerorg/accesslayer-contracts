#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Vec,
};

pub const REFERRAL_REWARD_DISTRIBUTED_EVENT_NAME: soroban_sdk::Symbol =
    symbol_short!("ref_dist");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialised = 1,
    ReferralAlreadyRegistered = 2,
    SelfReferral = 3,
    InvalidFeeAmount = 4,
    ArithmeticOverflow = 5,
}

#[contract]
pub struct ReferralContract;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    FirstTradeStatus(Address),
    Referral(Address),
    RewardBalance(Address),
    Initialised,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralRewardDistributed {
    pub tier: u32,
    pub referrer: Address,
    pub amount: i128,
}

#[contractimpl]
impl ReferralContract {
    pub fn init(env: Env) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Initialised) {
            return Err(ContractError::AlreadyInitialised);
        }

        env.storage().instance().set(&DataKey::Initialised, &true);
        env.events().publish((symbol_short!("init"),), ());

        Ok(())
    }

    pub fn get_initialised(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Initialised)
            .unwrap_or(false)
    }

    /// Register a wallet's immutable direct referrer.
    pub fn register_referral(
        env: Env,
        wallet: Address,
        referrer: Address,
    ) -> Result<(), ContractError> {
        wallet.require_auth();

        if wallet == referrer {
            return Err(ContractError::SelfReferral);
        }

        let key = DataKey::Referral(wallet.clone());
        if env.storage().persistent().has(&key) {
            return Err(ContractError::ReferralAlreadyRegistered);
        }

        env.storage().persistent().set(&key, &referrer);
        Ok(())
    }

    /// Returns the direct referrer followed by up to two successive referrers.
    pub fn get_referral_chain(env: Env, wallet: Address) -> Vec<Address> {
        let mut chain = Vec::new(&env);
        let mut current = wallet;

        for _ in 0..3 {
            let Some(referrer) = env
                .storage()
                .persistent()
                .get::<DataKey, Address>(&DataKey::Referral(current))
            else {
                break;
            };
            chain.push_back(referrer.clone());
            current = referrer;
        }

        chain
    }

    /// Accrue 40%, 20%, and 10% of the referral fee allocation to tiers 1-3.
    pub fn on_swap_fee(
        env: Env,
        wallet: Address,
        fee_amount: i128,
    ) -> Result<(), ContractError> {
        wallet.require_auth();
        if fee_amount < 0 {
            return Err(ContractError::InvalidFeeAmount);
        }

        let chain = Self::get_referral_chain(env.clone(), wallet);
        let percentages = [40_i128, 20_i128, 10_i128];

        for (index, referrer) in chain.iter().enumerate() {
            let amount = fee_amount
                .checked_mul(percentages[index as usize])
                .ok_or(ContractError::ArithmeticOverflow)?
                / 100;
            let key = DataKey::RewardBalance(referrer.clone());
            let balance = env
                .storage()
                .persistent()
                .get::<DataKey, i128>(&key)
                .unwrap_or(0);
            let updated_balance = balance
                .checked_add(amount)
                .ok_or(ContractError::ArithmeticOverflow)?;

            env.storage()
                .persistent()
                .set(&key, &updated_balance);
            env.events().publish(
                (
                    REFERRAL_REWARD_DISTRIBUTED_EVENT_NAME,
                    referrer.clone(),
                    index as u32 + 1,
                ),
                ReferralRewardDistributed {
                    tier: index as u32 + 1,
                    referrer,
                    amount,
                },
            );
        }

        Ok(())
    }

    /// Returns a referrer's total accrued reward allocation.
    pub fn get_reward_balance(env: Env, referrer: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::RewardBalance(referrer))
            .unwrap_or(0)
    }

    /// Returns true if it was indeed the first trade (and records it as true now).
    pub fn record_first_trade(env: Env, wallet: Address) -> bool {
        let key = DataKey::FirstTradeStatus(wallet);
        if env.storage().persistent().has(&key) {
            false
        } else {
            env.storage().persistent().set(&key, &true);
            true
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Events, Env, IntoVal, Symbol, TryFromVal};

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
    fn test_referral_chain_and_tier_rewards() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);
        let wallet = Address::generate(&env);
        let tier_one = Address::generate(&env);
        let tier_two = Address::generate(&env);
        let tier_three = Address::generate(&env);

        client.register_referral(&tier_two, &tier_three);
        client.register_referral(&tier_one, &tier_two);
        client.register_referral(&wallet, &tier_one);

        let chain = client.get_referral_chain(&wallet);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain.get(0), Some(tier_one.clone()));
        assert_eq!(chain.get(1), Some(tier_two.clone()));
        assert_eq!(chain.get(2), Some(tier_three.clone()));

        client.on_swap_fee(&wallet, &100);
        assert_eq!(client.get_reward_balance(&tier_one), 40);
        assert_eq!(client.get_reward_balance(&tier_two), 20);
        assert_eq!(client.get_reward_balance(&tier_three), 10);

        let mut distributed_events = 0;
        for (_, topics, data) in env.events().all().iter() {
            let event_name = Symbol::try_from_val(&env, &topics.get(0).unwrap()).unwrap();
            if event_name == REFERRAL_REWARD_DISTRIBUTED_EVENT_NAME {
                let event = ReferralRewardDistributed::try_from_val(&env, &data).unwrap();
                assert_eq!(
                    topics.get(1),
                    Some(event.referrer.clone().into_val(&env))
                );
                assert_eq!(topics.get(2), Some(event.tier.into_val(&env)));
                assert_eq!(event.referrer, chain.get(event.tier - 1).unwrap());
                assert_eq!(event.amount, [40, 20, 10][(event.tier - 1) as usize]);
                distributed_events += 1;
            }
        }
        assert_eq!(distributed_events, 3);
    }

    #[test]
    fn test_single_level_chain_only_accrues_tier_one() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);
        let wallet = Address::generate(&env);
        let referrer = Address::generate(&env);
        let unrelated = Address::generate(&env);

        client.register_referral(&wallet, &referrer);
        client.on_swap_fee(&wallet, &100);

        assert_eq!(client.get_reward_balance(&referrer), 40);
        assert_eq!(client.get_reward_balance(&unrelated), 0);
        assert_eq!(client.get_referral_chain(&wallet).len(), 1);
    }

    #[test]
    fn test_rejects_self_referral_and_duplicate_registration() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ReferralContract, ());
        let client = ReferralContractClient::new(&env, &contract_id);
        let wallet = Address::generate(&env);
        let referrer = Address::generate(&env);

        assert_eq!(
            client.try_register_referral(&wallet, &wallet),
            Err(Ok(ContractError::SelfReferral))
        );
        client.register_referral(&wallet, &referrer);
        assert_eq!(
            client.try_register_referral(&wallet, &referrer),
            Err(Ok(ContractError::ReferralAlreadyRegistered))
        );
    }
}
