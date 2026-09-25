#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialised = 1,
}

#[contract]
pub struct ReferralContract;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    FirstTradeStatus(Address),
    Initialised,
}

#[contractimpl]
impl ReferralContract {
    pub fn init(env: Env) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Initialised) {
            return Err(ContractError::AlreadyInitialised);
        }
        
        env.storage().instance().set(&DataKey::Initialised, &true);
        env.events().publish((Symbol::new(&env, "ContractInitialised"),), ());
        
        Ok(())
    }

    pub fn get_initialised(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Initialised).unwrap_or(false)
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
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Events, Env, IntoVal};

    #[test]
    fn test_initialisation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ReferralContract);
        let client = ReferralContractClient::new(&env, &contract_id);

        assert_eq!(client.get_initialised(), false);

        // First init
        client.init();

        assert_eq!(client.get_initialised(), true);

        // Check event
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        let event = events.get(0).unwrap();
        assert_eq!(event.0, contract_id);
        let topics = event.1;
        assert_eq!(topics.len(), 1);
        let expected_topic = Symbol::new(&env, "ContractInitialised");
        assert_eq!(topics.get(0).unwrap(), expected_topic.into_val(&env));
    }

    #[test]
    fn test_double_initialisation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ReferralContract);
        let client = ReferralContractClient::new(&env, &contract_id);

        client.init();
        
        let result = client.try_init();
        assert_eq!(result, Err(Ok(ContractError::AlreadyInitialised)));
        
        // Ensure state wasn't overwritten by checking event count (should still be 1)
        assert_eq!(env.events().all().len(), 1);
    }
}
