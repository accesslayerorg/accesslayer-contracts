#![no_std]

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, Address, Env, Symbol,
};

#[contractclient(name = "FeeReceiverClient")]
pub trait FeeReceiver {
    fn receive_fee(env: Env, amount: i128);
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeSplit {
    pub treasury_bps: u32,
    pub lp_bps: u32,
    pub staking_bps: u32,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    SwapContract,
    Treasury,
    LpPool,
    StakingPool,
    Split,
    Balance(Address),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FeeError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidSplit = 3,
    InvalidAmount = 4,
    ArithmeticOverflow = 5,
}

#[contract]
pub struct ProtocolFeeContract;

fn validate_split(split: &FeeSplit) -> bool {
    split.treasury_bps <= 10_000
        && split.lp_bps <= 10_000
        && split.staking_bps <= 10_000
        && split
            .treasury_bps
            .checked_add(split.lp_bps)
            .and_then(|value| value.checked_add(split.staking_bps))
            == Some(10_000)
}

fn require_admin(env: &Env, caller: &Address) -> Result<(), FeeError> {
    caller.require_auth();
    if env.storage().instance().get::<_, Address>(&DataKey::Admin) != Some(caller.clone()) {
        return Err(FeeError::Unauthorized);
    }
    Ok(())
}

#[contractimpl]
impl ProtocolFeeContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        swap_contract: Address,
        treasury: Address,
        lp_pool: Address,
        staking_pool: Address,
    ) -> Result<(), FeeError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(FeeError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::SwapContract, &swap_contract);
        env.storage().instance().set(&DataKey::Treasury, &treasury);
        env.storage().instance().set(&DataKey::LpPool, &lp_pool);
        env.storage()
            .instance()
            .set(&DataKey::StakingPool, &staking_pool);
        env.storage().instance().set(
            &DataKey::Split,
            &FeeSplit {
                treasury_bps: 5_000,
                lp_bps: 2_500,
                staking_bps: 2_500,
            },
        );
        Ok(())
    }

    pub fn set_split(env: Env, caller: Address, split: FeeSplit) -> Result<(), FeeError> {
        require_admin(&env, &caller)?;
        if !validate_split(&split) {
            return Err(FeeError::InvalidSplit);
        }
        env.storage().instance().set(&DataKey::Split, &split);
        Ok(())
    }

    pub fn collect_fee(
        env: Env,
        caller: Address,
        key_id: Address,
        amount: i128,
    ) -> Result<(), FeeError> {
        caller.require_auth();
        let swap_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::SwapContract)
            .ok_or(FeeError::Unauthorized)?;
        if caller != swap_contract {
            return Err(FeeError::Unauthorized);
        }
        if amount <= 0 {
            return Err(FeeError::InvalidAmount);
        }
        let split: FeeSplit = env
            .storage()
            .instance()
            .get(&DataKey::Split)
            .ok_or(FeeError::InvalidSplit)?;
        let treasury_amount = amount
            .checked_mul(split.treasury_bps as i128)
            .ok_or(FeeError::ArithmeticOverflow)?
            / 10_000;
        let lp_amount = amount
            .checked_mul(split.lp_bps as i128)
            .ok_or(FeeError::ArithmeticOverflow)?
            / 10_000;
        let staking_amount = amount
            .checked_sub(treasury_amount)
            .and_then(|value| value.checked_sub(lp_amount))
            .ok_or(FeeError::ArithmeticOverflow)?;
        let destinations: [(DataKey, i128); 3] = [
            (DataKey::Treasury, treasury_amount),
            (DataKey::LpPool, lp_amount),
            (DataKey::StakingPool, staking_amount),
        ];
        for (destination_key, value) in destinations {
            let destination: Address = env
                .storage()
                .instance()
                .get(&destination_key)
                .ok_or(FeeError::Unauthorized)?;
            let current: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::Balance(destination.clone()))
                .unwrap_or(0);
            let updated = current
                .checked_add(value)
                .ok_or(FeeError::ArithmeticOverflow)?;
            env.storage()
                .persistent()
                .set(&DataKey::Balance(destination), &updated);
        }
        env.events().publish(
            (Symbol::new(&env, "FeeCollected"), key_id),
            (amount, treasury_amount, lp_amount, staking_amount),
        );
        Ok(())
    }

    pub fn get_balance(env: Env, destination: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(destination))
            .unwrap_or(0)
    }

    pub fn withdraw(env: Env, destination: Address) -> Result<i128, FeeError> {
        let configured = [
            env.storage()
                .instance()
                .get::<_, Address>(&DataKey::Treasury),
            env.storage().instance().get::<_, Address>(&DataKey::LpPool),
            env.storage()
                .instance()
                .get::<_, Address>(&DataKey::StakingPool),
        ];
        if !configured
            .iter()
            .any(|value| value.as_ref() == Some(&destination))
        {
            return Err(FeeError::Unauthorized);
        }
        let key = DataKey::Balance(destination.clone());
        let amount: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if amount > 0 {
            env.storage().persistent().remove(&key);
            FeeReceiverClient::new(&env, &destination).receive_fee(&amount);
        }
        Ok(amount)
    }
}

#[cfg(test)]
mod test {
    use super::{FeeError, FeeSplit, ProtocolFeeContract, ProtocolFeeContractClient};
    use soroban_sdk::{contract, contractimpl, testutils::Address as _, Address, Env};

    #[contract]
    struct MockReceiver;

    #[contractimpl]
    impl MockReceiver {
        pub fn receive_fee(_env: Env, _amount: i128) {}
    }

    #[test]
    fn collects_default_split_and_withdraws_to_destinations() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let swapper = Address::generate(&env);
        let treasury = env.register(MockReceiver, ());
        let lp = env.register(MockReceiver, ());
        let staking = env.register(MockReceiver, ());
        let contract_id = env.register(ProtocolFeeContract, ());
        let client = ProtocolFeeContractClient::new(&env, &contract_id);
        client.initialize(&admin, &swapper, &treasury, &lp, &staking);

        let key_id = Address::generate(&env);
        client.collect_fee(&swapper, &key_id, &101);
        assert_eq!(client.get_balance(&treasury), 50);
        assert_eq!(client.get_balance(&lp), 25);
        assert_eq!(client.get_balance(&staking), 26);
        assert_eq!(client.withdraw(&treasury), 50);
        assert_eq!(client.get_balance(&treasury), 0);
    }

    #[test]
    fn rejects_invalid_split_and_untrusted_collector() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let swapper = Address::generate(&env);
        let treasury = env.register(MockReceiver, ());
        let lp = env.register(MockReceiver, ());
        let staking = env.register(MockReceiver, ());
        let contract_id = env.register(ProtocolFeeContract, ());
        let client = ProtocolFeeContractClient::new(&env, &contract_id);
        client.initialize(&admin, &swapper, &treasury, &lp, &staking);
        let invalid_split = FeeSplit {
            treasury_bps: 5_000,
            lp_bps: 3_000,
            staking_bps: 3_000,
        };
        assert_eq!(
            client.try_set_split(&admin, &invalid_split),
            Err(Ok(FeeError::InvalidSplit))
        );
        let other = Address::generate(&env);
        let key_id = Address::generate(&env);
        assert_eq!(
            client.try_collect_fee(&other, &key_id, &100),
            Err(Ok(FeeError::Unauthorized))
        );
    }
}
