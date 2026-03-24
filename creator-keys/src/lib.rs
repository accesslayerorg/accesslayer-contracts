#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Creator(Address),
}

/// On-chain profile stored under `DataKey::Creator`.
#[derive(Clone)]
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
}

/// Event payload emitted when a new creator registers.
///
/// Kept separate from `CreatorProfile` so the indexing schema can evolve
/// independently of the storage layout.
///
/// # Event structure
///
/// | Field    | Type      | Description                                  |
/// |----------|-----------|----------------------------------------------|
/// | creator  | `Address` | Stellar address of the registered creator    |
/// | handle   | `String`  | Human-readable handle chosen by the creator  |
/// | supply   | `u32`     | Initial key supply (always `0` at creation)  |
/// | ledger   | `u32`     | Ledger sequence when registration occurred   |
///
/// **Topics**: `("register", creator_address)`
/// The creator address is included in the topic tuple so indexers can
/// subscribe to or filter events for a specific creator without parsing
/// the data body.
#[derive(Clone, Debug)]
#[contracttype]
pub struct CreatorRegistered {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
    pub ledger: u32,
}

#[contract]
pub struct CreatorKeysContract;

#[contractimpl]
impl CreatorKeysContract {
    /// Register a new creator on-chain.
    ///
    /// Emits a `("register", creator)` event with a [`CreatorRegistered`]
    /// data payload for downstream indexing.
    pub fn register_creator(env: Env, creator: Address, handle: String) {
        creator.require_auth();

        let key = DataKey::Creator(creator.clone());
        let profile = CreatorProfile {
            creator: creator.clone(),
            handle: handle.clone(),
            supply: 0,
        };

        env.storage().persistent().set(&key, &profile);

        env.events().publish(
            (symbol_short!("register"), creator.clone()),
            CreatorRegistered {
                creator,
                handle,
                supply: 0,
                ledger: env.ledger().sequence(),
            },
        );
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

    pub fn get_creator(env: Env, creator: Address) -> Option<CreatorProfile> {
        let key = DataKey::Creator(creator);
        env.storage().persistent().get(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Events, vec, Env, IntoVal, String};

    #[test]
    fn test_register_emits_rich_event() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);

        let creator = Address::generate(&env);
        let handle = String::from_str(&env, "alice");

        client.register_creator(&creator, &handle);

        let events = env.events().all();

        // Find our register event among all published events.
        let register_events: soroban_sdk::Vec<_> = events
            .iter()
            .filter(|(_, topics, _)| {
                let expected_topics = (symbol_short!("register"), creator.clone()).into_val(&env);
                *topics == expected_topics
            })
            .collect(&env);

        assert_eq!(
            register_events.len(),
            1,
            "expected exactly one register event"
        );
    }

    #[test]
    fn test_register_event_fields() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);

        let creator = Address::generate(&env);
        let handle = String::from_str(&env, "bob");

        client.register_creator(&creator, &handle);

        let events = env.events().all();

        // Extract the data payload from the register event.
        let (_, _, data) = events
            .iter()
            .find(|(_, topics, _)| {
                let expected_topics = (symbol_short!("register"), creator.clone()).into_val(&env);
                *topics == expected_topics
            })
            .expect("register event not found");

        let registered: CreatorRegistered = data.into_val(&env);

        assert_eq!(registered.creator, creator);
        assert_eq!(registered.handle, handle);
        assert_eq!(registered.supply, 0);
        // Ledger sequence is set by the test env; just verify it is present.
        assert!(registered.ledger >= 0);
    }
}
