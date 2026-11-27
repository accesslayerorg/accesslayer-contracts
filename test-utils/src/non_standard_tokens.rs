//! Non-standard SEP-41-like mock token contracts for compatibility testing.
//!
//! Provides mock contracts simulating:
//! - Fee-on-transfer tokens (`FeeOnTransferTokenContract`)
//! - Rebasing-balance tokens (`RebasingTokenContract`)
//! - Transfer-returns-without-moving / no-op tokens (`NoOpTransferTokenContract`)
//! - Panicking-on-transfer tokens (`PanickingTokenContract`)
//! - Unusual-decimal tokens (`UnusualDecimalTokenContract`)

pub mod fee_on_transfer {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

    #[contracttype]
    #[derive(Clone)]
    pub enum FeeTokenDataKey {
        Balance(Address),
        FeeBps,
        Decimals,
    }

    #[contract]
    pub struct FeeOnTransferTokenContract;

    #[contractimpl]
    impl FeeOnTransferTokenContract {
        /// Configure the transfer fee in basis points (default: 500 = 5%).
        pub fn set_fee_bps(env: Env, fee_bps: u32) {
            env.storage()
                .instance()
                .set(&FeeTokenDataKey::FeeBps, &fee_bps);
        }

        pub fn set_decimals(env: Env, decimals: u32) {
            env.storage()
                .instance()
                .set(&FeeTokenDataKey::Decimals, &decimals);
        }

        pub fn decimals(env: Env) -> u32 {
            env.storage()
                .instance()
                .get(&FeeTokenDataKey::Decimals)
                .unwrap_or(7)
        }

        pub fn mint(env: Env, to: Address, amount: i128) {
            let key = FeeTokenDataKey::Balance(to);
            let balance: i128 = env.storage().instance().get(&key).unwrap_or(0);
            env.storage().instance().set(&key, &(balance + amount));
        }

        pub fn balance(env: Env, id: Address) -> i128 {
            env.storage()
                .instance()
                .get(&FeeTokenDataKey::Balance(id))
                .unwrap_or(0)
        }

        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            let from_key = FeeTokenDataKey::Balance(from.clone());
            let to_key = FeeTokenDataKey::Balance(to.clone());
            let from_balance: i128 = env.storage().instance().get(&from_key).unwrap_or(0);
            if amount > from_balance {
                panic!("Insufficient balance");
            }
            let fee_bps: u32 = env
                .storage()
                .instance()
                .get(&FeeTokenDataKey::FeeBps)
                .unwrap_or(500); // 5% default fee
            let fee = (amount * i128::from(fee_bps)) / 10_000;
            let received = amount - fee;

            env.storage()
                .instance()
                .set(&from_key, &(from_balance - amount));
            let to_balance: i128 = env.storage().instance().get(&to_key).unwrap_or(0);
            env.storage()
                .instance()
                .set(&to_key, &(to_balance + received));
            env.events()
                .publish((Symbol::new(&env, "Transfer"), from, to), received);
        }
    }

    pub struct FeeOnTransferToken;

    impl FeeOnTransferToken {
        pub fn register(env: &Env) -> (FeeOnTransferTokenContractClient<'_>, Address) {
            let id = env.register(FeeOnTransferTokenContract, ());
            let client = FeeOnTransferTokenContractClient::new(env, &id);
            (client, id)
        }
    }
}

pub mod rebasing {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

    #[contracttype]
    #[derive(Clone)]
    pub enum RebasingTokenDataKey {
        Shares(Address),
        MultiplierBps,
        RebaseOnTransferBps,
        Decimals,
    }

    #[contract]
    pub struct RebasingTokenContract;

    #[contractimpl]
    impl RebasingTokenContract {
        /// Set the global balance multiplier in basis points (10_000 = 1.0x).
        pub fn set_multiplier_bps(env: Env, multiplier_bps: i128) {
            env.storage()
                .instance()
                .set(&RebasingTokenDataKey::MultiplierBps, &multiplier_bps);
        }

        /// Optionally trigger a multiplier shift on every transfer (0 = disabled).
        pub fn set_rebase_on_transfer_bps(env: Env, new_multiplier_bps: i128) {
            env.storage().instance().set(
                &RebasingTokenDataKey::RebaseOnTransferBps,
                &new_multiplier_bps,
            );
        }

        pub fn decimals(env: Env) -> u32 {
            env.storage()
                .instance()
                .get(&RebasingTokenDataKey::Decimals)
                .unwrap_or(7)
        }

        pub fn mint(env: Env, to: Address, amount: i128) {
            let mult: i128 = env
                .storage()
                .instance()
                .get(&RebasingTokenDataKey::MultiplierBps)
                .unwrap_or(10_000);
            let shares = (amount * 10_000) / mult;
            let key = RebasingTokenDataKey::Shares(to);
            let current: i128 = env.storage().instance().get(&key).unwrap_or(0);
            env.storage().instance().set(&key, &(current + shares));
        }

        pub fn balance(env: Env, id: Address) -> i128 {
            let mult: i128 = env
                .storage()
                .instance()
                .get(&RebasingTokenDataKey::MultiplierBps)
                .unwrap_or(10_000);
            let shares: i128 = env
                .storage()
                .instance()
                .get(&RebasingTokenDataKey::Shares(id))
                .unwrap_or(0);
            (shares * mult) / 10_000
        }

        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            let mut mult: i128 = env
                .storage()
                .instance()
                .get(&RebasingTokenDataKey::MultiplierBps)
                .unwrap_or(10_000);

            let from_key = RebasingTokenDataKey::Shares(from.clone());
            let to_key = RebasingTokenDataKey::Shares(to.clone());
            let from_shares: i128 = env.storage().instance().get(&from_key).unwrap_or(0);
            let shares_needed = (amount * 10_000) / mult;
            if shares_needed > from_shares {
                panic!("Insufficient balance");
            }
            env.storage()
                .instance()
                .set(&from_key, &(from_shares - shares_needed));
            let to_shares: i128 = env.storage().instance().get(&to_key).unwrap_or(0);
            env.storage()
                .instance()
                .set(&to_key, &(to_shares + shares_needed));

            let on_transfer_mult: i128 = env
                .storage()
                .instance()
                .get(&RebasingTokenDataKey::RebaseOnTransferBps)
                .unwrap_or(0);
            if on_transfer_mult > 0 {
                mult = on_transfer_mult;
                env.storage()
                    .instance()
                    .set(&RebasingTokenDataKey::MultiplierBps, &mult);
            }

            env.events()
                .publish((Symbol::new(&env, "Transfer"), from, to), amount);
        }
    }

    pub struct RebasingToken;

    impl RebasingToken {
        pub fn register(env: &Env) -> (RebasingTokenContractClient<'_>, Address) {
            let id = env.register(RebasingTokenContract, ());
            let client = RebasingTokenContractClient::new(env, &id);
            (client, id)
        }
    }
}

pub mod no_op {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

    #[contracttype]
    #[derive(Clone)]
    pub enum NoOpTokenDataKey {
        Balance(Address),
    }

    #[contract]
    pub struct NoOpTransferTokenContract;

    #[contractimpl]
    impl NoOpTransferTokenContract {
        pub fn decimals(_env: Env) -> u32 {
            7
        }

        pub fn mint(env: Env, to: Address, amount: i128) {
            let key = NoOpTokenDataKey::Balance(to);
            let balance: i128 = env.storage().instance().get(&key).unwrap_or(0);
            env.storage().instance().set(&key, &(balance + amount));
        }

        pub fn balance(env: Env, id: Address) -> i128 {
            env.storage()
                .instance()
                .get(&NoOpTokenDataKey::Balance(id))
                .unwrap_or(0)
        }

        /// Returns successfully without modifying any balances.
        pub fn transfer(_env: Env, _from: Address, _to: Address, _amount: i128) {}
    }

    pub struct NoOpTransferToken;

    impl NoOpTransferToken {
        pub fn register(env: &Env) -> (NoOpTransferTokenContractClient<'_>, Address) {
            let id = env.register(NoOpTransferTokenContract, ());
            let client = NoOpTransferTokenContractClient::new(env, &id);
            (client, id)
        }
    }
}

pub mod panicking {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

    #[contracttype]
    #[derive(Clone)]
    pub enum PanickingTokenDataKey {
        Balance(Address),
        PanicOnTransfer,
    }

    #[contract]
    pub struct PanickingTokenContract;

    #[contractimpl]
    impl PanickingTokenContract {
        pub fn decimals(_env: Env) -> u32 {
            7
        }

        pub fn set_panic_on_transfer(env: Env, should_panic: bool) {
            env.storage()
                .instance()
                .set(&PanickingTokenDataKey::PanicOnTransfer, &should_panic);
        }

        pub fn mint(env: Env, to: Address, amount: i128) {
            let key = PanickingTokenDataKey::Balance(to);
            let balance: i128 = env.storage().instance().get(&key).unwrap_or(0);
            env.storage().instance().set(&key, &(balance + amount));
        }

        pub fn balance(env: Env, id: Address) -> i128 {
            env.storage()
                .instance()
                .get(&PanickingTokenDataKey::Balance(id))
                .unwrap_or(0)
        }

        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            let should_panic: bool = env
                .storage()
                .instance()
                .get(&PanickingTokenDataKey::PanicOnTransfer)
                .unwrap_or(true);
            if should_panic {
                panic!("PanickingToken: transfer failed");
            }
            let from_key = PanickingTokenDataKey::Balance(from);
            let to_key = PanickingTokenDataKey::Balance(to);
            let from_balance: i128 = env.storage().instance().get(&from_key).unwrap_or(0);
            if amount > from_balance {
                panic!("Insufficient balance");
            }
            env.storage()
                .instance()
                .set(&from_key, &(from_balance - amount));
            let to_balance: i128 = env.storage().instance().get(&to_key).unwrap_or(0);
            env.storage()
                .instance()
                .set(&to_key, &(to_balance + amount));
        }
    }

    pub struct PanickingToken;

    impl PanickingToken {
        pub fn register(env: &Env) -> (PanickingTokenContractClient<'_>, Address) {
            let id = env.register(PanickingTokenContract, ());
            let client = PanickingTokenContractClient::new(env, &id);
            (client, id)
        }
    }
}

pub mod unusual_decimal {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

    #[contracttype]
    #[derive(Clone)]
    pub enum UnusualDecimalTokenDataKey {
        Balance(Address),
        Decimals,
    }

    #[contract]
    pub struct UnusualDecimalTokenContract;

    #[contractimpl]
    impl UnusualDecimalTokenContract {
        pub fn set_decimals(env: Env, decimals: u32) {
            env.storage()
                .instance()
                .set(&UnusualDecimalTokenDataKey::Decimals, &decimals);
        }

        pub fn decimals(env: Env) -> u32 {
            env.storage()
                .instance()
                .get(&UnusualDecimalTokenDataKey::Decimals)
                .unwrap_or(18)
        }

        pub fn mint(env: Env, to: Address, amount: i128) {
            let key = UnusualDecimalTokenDataKey::Balance(to);
            let balance: i128 = env.storage().instance().get(&key).unwrap_or(0);
            env.storage().instance().set(&key, &(balance + amount));
        }

        pub fn balance(env: Env, id: Address) -> i128 {
            env.storage()
                .instance()
                .get(&UnusualDecimalTokenDataKey::Balance(id))
                .unwrap_or(0)
        }

        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            let from_key = UnusualDecimalTokenDataKey::Balance(from.clone());
            let to_key = UnusualDecimalTokenDataKey::Balance(to.clone());
            let from_balance: i128 = env.storage().instance().get(&from_key).unwrap_or(0);
            if amount > from_balance {
                panic!("Insufficient balance");
            }
            env.storage()
                .instance()
                .set(&from_key, &(from_balance - amount));
            let to_balance: i128 = env.storage().instance().get(&to_key).unwrap_or(0);
            env.storage()
                .instance()
                .set(&to_key, &(to_balance + amount));
            env.events()
                .publish((Symbol::new(&env, "Transfer"), from, to), amount);
        }
    }

    pub struct UnusualDecimalToken;

    impl UnusualDecimalToken {
        pub fn register(env: &Env) -> (UnusualDecimalTokenContractClient<'_>, Address) {
            let id = env.register(UnusualDecimalTokenContract, ());
            let client = UnusualDecimalTokenContractClient::new(env, &id);
            (client, id)
        }
    }
}

pub use fee_on_transfer::{
    FeeOnTransferToken, FeeOnTransferTokenContract, FeeOnTransferTokenContractClient,
};
pub use no_op::{NoOpTransferToken, NoOpTransferTokenContract, NoOpTransferTokenContractClient};
pub use panicking::{PanickingToken, PanickingTokenContract, PanickingTokenContractClient};
pub use rebasing::{RebasingToken, RebasingTokenContract, RebasingTokenContractClient};
pub use unusual_decimal::{
    UnusualDecimalToken, UnusualDecimalTokenContract, UnusualDecimalTokenContractClient,
};
