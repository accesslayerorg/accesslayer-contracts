#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    AlreadyRegistered = 1,
    NotRegistered = 2,
    Overflow = 3,
    InsufficientPayment = 4,
    KeyPriceNotSet = 5,
    NotPositiveAmount = 6,
    FeeConfigNotSet = 7,
    InvalidFeeConfig = 8,
}

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
    pub fn validate_fee_bps(creator_bps: u32, protocol_bps: u32) -> bool {
        let Some(sum) = creator_bps.checked_add(protocol_bps) else {
            return false;
        };
        if sum != BPS_MAX {
            return false;
        }
        if protocol_bps > PROTOCOL_BPS_MAX {
            return false;
        }
        true
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

/// Stable, non-optional view of creator details.
///
/// Returned by [`CreatorKeysContract::get_creator_details`] for indexer-friendly consumption.
/// When `is_registered` is `false`, default values are returned for other fields.
#[derive(Clone)]
#[contracttype]
pub struct CreatorDetailsView {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
    pub is_registered: bool,
}
/// Stable, non-optional view of a creator's fee configuration.
///
/// Returned by [`CreatorKeysContract::get_creator_fee_config`] for indexer-friendly consumption.
/// When `is_registered` is `false`, the creator does not exist and both bps fields are `0`.
/// When `is_configured` is `false`, the creator exists but no global fee config has been set.
#[derive(Clone)]
#[contracttype]
pub struct CreatorFeeView {
    pub creator_bps: u32,
    pub protocol_bps: u32,
    pub is_registered: bool,
    pub is_configured: bool,
}

/// Stable protocol state version for read-only consumers.
///
/// Bump this value only when externally consumed protocol state semantics change.
pub const PROTOCOL_STATE_VERSION: u32 = 1;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Creator(Address),
    FeeConfig,
    KeyPrice,
    KeyBalance(Address, Address),
    TreasuryAddress,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
}

/// Reads a creator profile from storage, returning `None` for unregistered creators.
///
/// Use this helper wherever repeated creator read logic is needed to keep
/// missing-creator behavior consistent across the contract.
pub fn read_creator_profile(env: &Env, creator: &Address) -> Option<CreatorProfile> {
    let key = DataKey::Creator(creator.clone());
    env.storage()
        .persistent()
        .get::<DataKey, CreatorProfile>(&key)
}

/// Reads the key balance (supply) for a creator, returning `0` for unregistered creators.
///
/// Use this helper wherever repeated key balance read logic is needed to keep
/// missing-balance behavior consistent across the contract.
pub fn read_key_balance(env: &Env, creator: &Address) -> u32 {
    read_creator_profile(env, creator)
        .map(|p| p.supply)
        .unwrap_or(0)
}

#[contract]
pub struct CreatorKeysContract;

#[contractimpl]
impl CreatorKeysContract {
    pub fn register_creator(
        env: Env,
        creator: Address,
        handle: String,
    ) -> Result<(), ContractError> {
        creator.require_auth();

        let key = DataKey::Creator(creator.clone());
        if env.storage().persistent().has(&key) {
            return Err(ContractError::AlreadyRegistered);
        }

        let profile = CreatorProfile {
            creator,
            handle,
            supply: 0,
        };

        env.storage().persistent().set(&key, &profile);
        env.events().publish((symbol_short!("register"),), key);

        Ok(())
    }

    pub fn buy_key(
        env: Env,
        creator: Address,
        buyer: Address,
        payment: i128,
    ) -> Result<u32, ContractError> {
        buyer.require_auth();

        if payment <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let price: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::KeyPrice)
            .ok_or(ContractError::KeyPriceNotSet)?;

        if payment < price {
            return Err(ContractError::InsufficientPayment);
        }

        let mut profile: CreatorProfile =
            read_creator_profile(&env, &creator).ok_or(ContractError::NotRegistered)?;

        profile.supply = profile
            .supply
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;

        let key = DataKey::Creator(creator.clone());
        env.storage().persistent().set(&key, &profile);

        let balance_key = DataKey::KeyBalance(creator.clone(), buyer.clone());
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        let new_balance = current_balance
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&balance_key, &new_balance);

        env.events().publish(
            (symbol_short!("buy"), creator, buyer),
            (profile.supply, payment),
        );

        Ok(profile.supply)
    }

    pub fn get_key_balance(env: Env, creator: Address, wallet: Address) -> u32 {
        let key = DataKey::KeyBalance(creator, wallet);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    pub fn get_creator(env: Env, creator: Address) -> Result<CreatorProfile, ContractError> {
        read_creator_profile(&env, &creator).ok_or(ContractError::NotRegistered)
    }

    /// Read-only view: returns stable creator details.
    ///
    /// Returns a [`CreatorDetailsView`] regardless of registration status.
    /// When the creator is not registered, `is_registered` is `false` and
    /// default values are provided for other fields.
    pub fn get_creator_details(env: Env, creator: Address) -> CreatorDetailsView {
        let key = DataKey::Creator(creator.clone());
        match env
            .storage()
            .persistent()
            .get::<DataKey, CreatorProfile>(&key)
        {
            Some(profile) => CreatorDetailsView {
                creator: profile.creator,
                handle: profile.handle,
                supply: profile.supply,
                is_registered: true,
            },
            None => CreatorDetailsView {
                creator,
                handle: String::from_str(&env, ""),
                supply: 0,
                is_registered: false,
            },
        }
    }
    /// Read-only view: returns the protocol state version.
    ///
    /// Returns a stable scalar value for clients and indexers to detect
    /// protocol-state schema/semantics revisions without mutating contract state.
    pub fn get_protocol_state_version(_env: Env) -> u32 {
        PROTOCOL_STATE_VERSION
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
        read_creator_profile(&env, &creator).is_some()
    }

    pub fn set_fee_config(
        env: Env,
        admin: Address,
        creator_bps: u32,
        protocol_bps: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        if !fee::validate_fee_bps(creator_bps, protocol_bps) {
            return Err(ContractError::InvalidFeeConfig);
        }

        let config = fee::FeeConfig {
            creator_bps,
            protocol_bps,
        };
        env.storage().persistent().set(&DataKey::FeeConfig, &config);
        Ok(())
    }

    pub fn set_key_price(env: Env, admin: Address, price: i128) -> Result<(), ContractError> {
        admin.require_auth();
        if price <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        env.storage().persistent().set(&DataKey::KeyPrice, &price);
        Ok(())
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

    pub fn compute_fees_for_payment(env: Env, total: i128) -> Result<(i128, i128), ContractError> {
        let config: fee::FeeConfig = env
            .storage()
            .persistent()
            .get(&DataKey::FeeConfig)
            .ok_or(ContractError::FeeConfigNotSet)?;
        Ok(fee::compute_fee_split(
            total,
            config.creator_bps,
            config.protocol_bps,
        ))
    }

    /// Read-only view: returns the fee configuration for a specific creator.
    ///
    /// Returns a stable [`CreatorFeeView`] regardless of whether the creator is registered
    /// or a fee config has been set. When `is_registered` is `false`, the creator does not
    /// exist and both bps fields are `0`. When `is_configured` is `false`, no global fee
    /// config has been set. Use this method for indexers and read-only callers that need
    /// a non-optional result.
    pub fn get_creator_fee_config(env: Env, creator: Address) -> CreatorFeeView {
        let is_registered = read_creator_profile(&env, &creator).is_some();

        if !is_registered {
            return CreatorFeeView {
                creator_bps: 0,
                protocol_bps: 0,
                is_registered: false,
                is_configured: false,
            };
        }

        match env
            .storage()
            .persistent()
            .get::<DataKey, fee::FeeConfig>(&DataKey::FeeConfig)
        {
            Some(config) => CreatorFeeView {
                creator_bps: config.creator_bps,
                protocol_bps: config.protocol_bps,
                is_registered: true,
                is_configured: true,
            },
            None => CreatorFeeView {
                creator_bps: 0,
                protocol_bps: 0,
                is_registered: true,
                is_configured: false,
            },
        }
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
mod test;
