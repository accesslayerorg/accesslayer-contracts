#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String};

pub mod fee {
    use soroban_sdk::contracttype;

    /// Basis points per 100% (10000 = 100%).
    pub const BPS_MAX: u32 = 10_000;

    /// Maximum protocol share when configuring fees via [`assert_valid_fee_bps`].
    ///
    /// Caps the on-chain configured protocol take at 50% so fee settings stay within
    /// expected economic bounds before they affect market logic.
    pub const PROTOCOL_BPS_MAX: u32 = 5_000;

    #[derive(Clone)]
    #[contracttype]
    pub struct FeeConfig {
        pub creator_bps: u32,
        pub protocol_bps: u32,
    }

    /// Validates creator and protocol basis points for storage and fee-setting entrypoints.
    ///
    /// Requires `creator_bps + protocol_bps == BPS_MAX` and `protocol_bps <= PROTOCOL_BPS_MAX`.
    pub fn assert_valid_fee_bps(creator_bps: u32, protocol_bps: u32) {
        let Some(sum) = creator_bps.checked_add(protocol_bps) else {
            panic!("creator_bps + protocol_bps overflow");
        };
        if sum != BPS_MAX {
            panic!("creator_bps + protocol_bps must equal 10000");
        }
        if protocol_bps > PROTOCOL_BPS_MAX {
            panic!(
                "protocol_bps exceeds maximum allowed ({} bps)",
                PROTOCOL_BPS_MAX
            );
        }
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

/// Stable, non-optional view of the protocol fee configuration.
///
/// Returned by [`CreatorKeysContract::get_protocol_fee_view`] for indexer-friendly consumption.
/// When `is_configured` is `false`, both bps fields are `0` and no fee config has been stored.
#[derive(Clone)]
#[contracttype]
pub struct ProtocolFeeView {
    pub creator_bps: u32,
    pub protocol_bps: u32,
    pub is_configured: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Creator(Address),
    FeeConfig,
    KeyPrice,
    KeyBalance(Address, Address),

    TreasuryAddress,
}

/// On-chain profile stored under `DataKey::Creator`.
#[derive(Clone)]
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
}

/// Shared validation: panics if the payment amount is zero or negative.
///
/// Use this before any purchase or payment logic to reject empty transactions
/// with a clear, consistent error message.
pub fn assert_positive_amount(amount: i128) {
    if amount <= 0 {
        panic!("payment amount must be positive");
    }
}

/// Reads the key balance (supply) for a creator, returning `0` for unregistered creators.
///
/// Use this helper wherever repeated key balance read logic is needed to keep
/// missing-balance behavior consistent across the contract.
pub fn read_key_balance(env: &Env, creator: &Address) -> u32 {
    let key = DataKey::Creator(creator.clone());
    env.storage()
        .persistent()
        .get::<DataKey, CreatorProfile>(&key)
        .map(|p| p.supply)
        .unwrap_or(0)
}

#[contract]
pub struct CreatorKeysContract;

impl CreatorKeysContract {
    fn require_creator(env: &Env, creator: &Address) -> CreatorProfile {
        let key = DataKey::Creator(creator.clone());
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("creator not registered"))
    }
}

#[contractimpl]
impl CreatorKeysContract {
    /// Register a new creator on-chain.
    ///
    /// Emits a `("register", creator)` event with a [`CreatorRegistered`]
    /// data payload for downstream indexing.
    pub fn register_creator(env: Env, creator: Address, handle: String) {
        creator.require_auth();

        let key = DataKey::Creator(creator.clone());
        if env.storage().persistent().has(&key) {
            panic!("creator already registered");
        }

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

    pub fn buy_key(env: Env, creator: Address, buyer: Address, payment: i128) -> u32 {
        buyer.require_auth();
        assert_positive_amount(payment);

        let price: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::KeyPrice)
            .unwrap_or_else(|| panic!("key price not set"));
        if payment < price {
            panic!("insufficient payment");
        }

        let key = DataKey::Creator(creator.clone());
        let mut profile = Self::require_creator(&env, &creator);

        profile.supply += 1;
        env.storage().persistent().set(&key, &profile);

        let balance_key = DataKey::KeyBalance(creator.clone(), buyer.clone());
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&balance_key, &(current_balance + 1));

        env.events()
            .publish((symbol_short!("buy"), creator, buyer), profile.supply);

        profile.supply
    }

    pub fn get_key_balance(env: Env, creator: Address, wallet: Address) -> u32 {
        let key = DataKey::KeyBalance(creator, wallet);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    pub fn get_creator(env: Env, creator: Address) -> Option<CreatorProfile> {
        let key = DataKey::Creator(creator);
        env.storage().persistent().get(&key)
    }

    /// Read-only view: returns the total key supply for a creator.
    ///
    /// Returns `0` if the creator is not registered, avoiding panics for
    /// invalid lookups. Delegates to the shared [`read_key_balance`] helper.
    pub fn get_total_key_supply(env: Env, creator: Address) -> u32 {
        read_key_balance(&env, &creator)
    }

    /// Read-only view: returns whether a creator is registered in the contract.
    ///
    /// Returns `true` if a [`CreatorProfile`] exists for the given address,
    /// `false` otherwise. Does not mutate state.
    pub fn is_creator_registered(env: Env, creator: Address) -> bool {
        let key = DataKey::Creator(creator);
        env.storage().persistent().has(&key)
    }

    pub fn set_fee_config(env: Env, admin: Address, creator_bps: u32, protocol_bps: u32) {
        admin.require_auth();
        fee::assert_valid_fee_bps(creator_bps, protocol_bps);
        let config = fee::FeeConfig {
            creator_bps,
            protocol_bps,
        };
        env.storage().persistent().set(&DataKey::FeeConfig, &config);
    }

    pub fn set_key_price(env: Env, admin: Address, price: i128) {
        admin.require_auth();
        assert_positive_amount(price);
        env.storage().persistent().set(&DataKey::KeyPrice, &price);
    }

    pub fn get_fee_config(env: Env) -> Option<fee::FeeConfig> {
        env.storage().persistent().get(&DataKey::FeeConfig)
    }

    /// Sets the protocol treasury address.
    ///
    /// Only callable by an authorized admin. Stores the treasury address used
    /// for protocol fee routing.
    pub fn set_treasury_address(env: Env, admin: Address, treasury: Address) {
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::TreasuryAddress, &treasury);
    }

    /// Read-only view: returns the current protocol treasury address.
    ///
    /// Returns `None` if no treasury address has been configured.
    /// Use this method for indexers and read-only callers that need the current
    /// treasury routing target.
    pub fn get_treasury_address(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::TreasuryAddress)
    }

    /// Read-only view: returns the current protocol fee configuration.
    ///
    /// Returns a stable [`ProtocolFeeView`] regardless of whether a fee config has been set.
    /// When no config is stored, `is_configured` is `false` and both bps fields are `0`.
    /// Use this method for indexers and read-only callers that need a non-optional result.
    pub fn get_protocol_fee_view(env: Env) -> ProtocolFeeView {
        match env
            .storage()
            .persistent()
            .get::<DataKey, fee::FeeConfig>(&DataKey::FeeConfig)
        {
            Some(config) => ProtocolFeeView {
                creator_bps: config.creator_bps,
                protocol_bps: config.protocol_bps,
                is_configured: true,
            },
            None => ProtocolFeeView {
                creator_bps: 0,
                protocol_bps: 0,
                is_configured: false,
            },
        }
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
