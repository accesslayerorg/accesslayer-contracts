#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Creator(Address),
}

#[derive(Clone)]
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
}

#[contract]
pub struct CreatorKeysContract;

#[contractimpl]
impl CreatorKeysContract {
    pub fn register_creator(env: Env, creator: Address, handle: String) {
        creator.require_auth();

        let key = DataKey::Creator(creator.clone());
        let profile = CreatorProfile {
            creator,
            handle,
            supply: 0,
        };

        env.storage().persistent().set(&key, &profile);
        env.events().publish((symbol_short!("register"),), key);
    }

    pub fn buy_key(env: Env, creator: Address, buyer: Address) -> u32 {
        buyer.require_auth();

        let key = DataKey::Creator(creator.clone());
        let mut profile: CreatorProfile = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("creator not registered"));

        profile.supply += 1;
        env.storage().persistent().set(&key, &profile);
        env.events()
            .publish((symbol_short!("buy"), creator, buyer), profile.supply);

        profile.supply
    }

    /// Sell a previously purchased key back, decrementing the creator's supply.
    ///
    /// Panics if the creator is not registered or if the current supply is zero.
    pub fn sell_key(env: Env, creator: Address, seller: Address) -> u32 {
        seller.require_auth();

        let key = DataKey::Creator(creator.clone());
        let mut profile: CreatorProfile = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("creator not registered"));

        if profile.supply == 0 {
            panic!("no keys to sell");
        }

        profile.supply -= 1;
        env.storage().persistent().set(&key, &profile);
        env.events()
            .publish((symbol_short!("sell"), creator, seller), profile.supply);

        profile.supply
    }

    pub fn get_creator(env: Env, creator: Address) -> Option<CreatorProfile> {
        let key = DataKey::Creator(creator);
        env.storage().persistent().get(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, String};

    fn setup_env() -> (Env, CreatorKeysContractClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);

        let creator = Address::generate(&env);
        let user = Address::generate(&env);

        let handle = String::from_str(&env, "alice");
        client.register_creator(&creator, &handle);

        (env, client, creator, user)
    }

    #[test]
    fn test_sell_key_decrements_supply() {
        let (_env, client, creator, user) = setup_env();

        // Buy two keys, then sell one.
        client.buy_key(&creator, &user);
        let supply_after_second_buy = client.buy_key(&creator, &user);
        assert_eq!(supply_after_second_buy, 2);

        let supply_after_sell = client.sell_key(&creator, &user);
        assert_eq!(supply_after_sell, 1);
    }

    #[test]
    fn test_sell_key_to_zero() {
        let (_env, client, creator, user) = setup_env();

        client.buy_key(&creator, &user);
        let supply = client.sell_key(&creator, &user);
        assert_eq!(supply, 0);
    }

    #[test]
    fn test_sell_key_supply_reflected_in_storage() {
        let (_env, client, creator, user) = setup_env();

        client.buy_key(&creator, &user);
        client.buy_key(&creator, &user);
        client.sell_key(&creator, &user);

        let profile = client.get_creator(&creator).unwrap();
        assert_eq!(profile.supply, 1);
    }

    #[test]
    #[should_panic(expected = "no keys to sell")]
    fn test_sell_key_panics_when_supply_is_zero() {
        let (_env, client, creator, user) = setup_env();

        // Creator is registered but has zero supply — sell must panic.
        client.sell_key(&creator, &user);
    }

    #[test]
    #[should_panic(expected = "creator not registered")]
    fn test_sell_key_panics_for_unregistered_creator() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);

        let unknown_creator = Address::generate(&env);
        let seller = Address::generate(&env);

        // No registration — sell must panic.
        client.sell_key(&unknown_creator, &seller);
    }

    #[test]
    fn test_buy_then_sell_then_buy_again() {
        let (_env, client, creator, user) = setup_env();

        client.buy_key(&creator, &user);
        client.sell_key(&creator, &user);
        let supply = client.buy_key(&creator, &user);

        assert_eq!(supply, 1, "supply should be 1 after buy-sell-buy cycle");
    }
}
