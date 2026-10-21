#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Symbol, Vec};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Balance,
    Epoch,
    EpochHistory(u32),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochDistribution {
    pub epoch: u32,
    pub recipients: Vec<Address>,
    pub amounts: Vec<i128>,
}

#[contract]
pub struct TreasuryContract;

#[contractimpl]
impl TreasuryContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Balance, &0i128);
        env.storage().instance().set(&DataKey::Epoch, &0u32);
    }

    pub fn collect_fee(env: Env, key_id: String, amount: i128) {
        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let mut balance: i128 = env.storage().instance().get(&DataKey::Balance).unwrap_or(0);
        balance += amount;
        env.storage().instance().set(&DataKey::Balance, &balance);

        env.events()
            .publish((Symbol::new(&env, "FeeCollected"), key_id), amount);
    }

    pub fn get_treasury_balance(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::Balance).unwrap_or(0)
    }

    pub fn distribute(env: Env, recipients: Vec<Address>, amounts: Vec<i128>) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        admin.require_auth();

        if recipients.len() != amounts.len() {
            panic!("Recipients and amounts length mismatch");
        }

        let mut total_amount: i128 = 0;
        for amount in amounts.iter() {
            if amount <= 0 {
                panic!("Amount must be positive");
            }
            total_amount += amount;
        }

        let mut balance: i128 = env.storage().instance().get(&DataKey::Balance).unwrap_or(0);
        if total_amount > balance {
            panic!("Insufficient balance");
        }

        balance -= total_amount;
        env.storage().instance().set(&DataKey::Balance, &balance);

        let epoch: u32 = env.storage().instance().get(&DataKey::Epoch).unwrap_or(0);
        let distribution = EpochDistribution {
            epoch,
            recipients: recipients.clone(),
            amounts: amounts.clone(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::EpochHistory(epoch), &distribution);
        env.storage().instance().set(&DataKey::Epoch, &(epoch + 1));

        env.events()
            .publish((Symbol::new(&env, "FeeDistributed"), epoch), distribution);
    }

    pub fn get_distribution_history(env: Env, epoch: u32) -> Option<EpochDistribution> {
        env.storage()
            .persistent()
            .get(&DataKey::EpochHistory(epoch))
    }
}

#[cfg(test)]
mod test;
