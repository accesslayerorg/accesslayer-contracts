#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String};

pub mod fee {
    use soroban_sdk::contracttype;

    /// Basis points per 100% (10000 = 100%).
    pub const BPS_MAX: u32 = 10_000;

    #[derive(Clone)]
    #[contracttype]
    pub struct FeeConfig {
        pub creator_bps: u32,
        pub protocol_bps: u32,
    }

    /// Computes the fee split for a given total amount.
    ///
    /// Returns `(creator_amount, protocol_amount)`. Remainder from integer division
    /// is assigned to the creator. Ensures creator_amount + protocol_amount == total.
    pub fn compute_fee_split(total: i128, _creator_bps: u32, protocol_bps: u32) -> (i128, i128) {
        if total <= 0 {
            return (0, 0);
        }
        let protocol_amount = (total * protocol_bps as i128) / BPS_MAX as i128;
        let creator_amount = total - protocol_amount;
        (creator_amount, protocol_amount)
    }
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Creator(Address),
    FeeConfig,
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

    pub fn set_fee_config(env: Env, admin: Address, creator_bps: u32, protocol_bps: u32) {
        admin.require_auth();
        if creator_bps + protocol_bps != fee::BPS_MAX {
            panic!("creator_bps + protocol_bps must equal 10000");
        }
        let config = fee::FeeConfig {
            creator_bps,
            protocol_bps,
        };
        env.storage().persistent().set(&DataKey::FeeConfig, &config);
    }

    pub fn get_fee_config(env: Env) -> Option<fee::FeeConfig> {
        env.storage().persistent().get(&DataKey::FeeConfig)
    }

    pub fn compute_fees_for_payment(env: Env, total: i128) -> (i128, i128) {
        let config: fee::FeeConfig = env
            .storage()
            .persistent()
            .get(&DataKey::FeeConfig)
            .unwrap_or_else(|| panic!("fee config not set"));
        fee::compute_fee_split(total, config.creator_bps, config.protocol_bps)
    }
}

#[cfg(test)]
mod tests {
    use super::fee;

    #[test]
    fn test_fee_split_90_10_1000() {
        let (creator, protocol) = fee::compute_fee_split(1000, 9000, 1000);
        assert_eq!(creator, 900);
        assert_eq!(protocol, 100);
        assert_eq!(creator + protocol, 1000);
    }

    #[test]
    fn test_fee_split_100_creator() {
        let (creator, protocol) = fee::compute_fee_split(1000, 10000, 0);
        assert_eq!(creator, 1000);
        assert_eq!(protocol, 0);
        assert_eq!(creator + protocol, 1000);
    }

    #[test]
    fn test_fee_split_100_protocol() {
        let (creator, protocol) = fee::compute_fee_split(1000, 0, 10000);
        assert_eq!(creator, 0);
        assert_eq!(protocol, 1000);
        assert_eq!(creator + protocol, 1000);
    }

    #[test]
    fn test_fee_split_remainder_to_creator() {
        // 999 * 1000 / 10000 = 99 (protocol floor), creator gets remainder
        let (creator, protocol) = fee::compute_fee_split(999, 9000, 1000);
        assert_eq!(creator, 900);
        assert_eq!(protocol, 99);
        assert_eq!(creator + protocol, 999);
    }

    #[test]
    fn test_fee_split_zero_total() {
        let (creator, protocol) = fee::compute_fee_split(0, 9000, 1000);
        assert_eq!(creator, 0);
        assert_eq!(protocol, 0);
    }

    #[test]
    fn test_fee_split_dust_total_one() {
        // 1 * 1000 / 10000 = 0 protocol, creator gets full amount
        let (creator, protocol) = fee::compute_fee_split(1, 9000, 1000);
        assert_eq!(creator, 1);
        assert_eq!(protocol, 0);
        assert_eq!(creator + protocol, 1);
    }

    #[test]
    fn test_fee_split_balance_conservation() {
        for total in [100_i128, 1, 999, 10000, 1234567] {
            let (creator, protocol) = fee::compute_fee_split(total, 9000, 1000);
            assert_eq!(creator + protocol, total, "total={}", total);
        }
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
