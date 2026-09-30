#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contract]
pub struct ReferralContract;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    FirstTradeStatus(Address),
}

#[contractimpl]
impl ReferralContract {
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
}
