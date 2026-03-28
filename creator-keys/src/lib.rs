#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String};

pub mod events;

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
    InsufficientBalance = 9,
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

    /// Computes the fee split safely, returning `None` if multiplication or subtraction overflows.
    pub fn checked_compute_fee_split(
        total: i128,
        _creator_bps: u32,
        protocol_bps: u32,
    ) -> Option<(i128, i128)> {
        if total <= 0 {
            return Some((0, 0));
        }
        let protocol_amount = total
            .checked_mul(protocol_bps as i128)?
            .checked_div(BPS_MAX as i128)?;
        let creator_amount = total.checked_sub(protocol_amount)?;
        Some((creator_amount, protocol_amount))
    }
}

pub mod constants {
    use super::DataKey;

    pub mod storage {
        use super::DataKey;

        pub const FEE_CONFIG: DataKey = DataKey::FeeConfig;
        pub const KEY_PRICE: DataKey = DataKey::KeyPrice;
        pub const TREASURY_ADDRESS: DataKey = DataKey::TreasuryAddress;
    }

    pub mod creator_reads {
        pub const DETAILS: &str = "get_creator_details";
        pub const FEE_BPS: &str = "get_creator_fee_bps";
        pub const FEE_CONFIG: &str = "get_creator_fee_config";
        pub const FEE_RECIPIENT: &str = "get_creator_fee_recipient";
        pub const HOLDER_KEY_COUNT: &str = "get_holder_key_count";
        pub const PROFILE: &str = "get_creator";
        pub const SUPPLY: &str = "get_creator_supply";
        pub const TREASURY_SHARE: &str = "get_creator_treasury_share";
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

/// Stable, non-optional view of a holder's key count for a creator.
///
/// Returned by [`CreatorKeysContract::get_holder_key_count`] for indexer-friendly consumption.
/// When `creator_exists` is `false`, the creator is not registered and `key_count` is `0`.
/// When `creator_exists` is `true` but the holder has no keys, `key_count` is `0`.
#[derive(Clone)]
#[contracttype]
pub struct HolderKeyCountView {
    pub creator: Address,
    pub holder: Address,
    pub key_count: u32,
    pub creator_exists: bool,
}

/// Stable, non-optional view of a buy or sell quote.
///
/// Returned by [`CreatorKeysContract::get_buy_quote`] and [`CreatorKeysContract::get_sell_quote`].
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct QuoteResponse {
    pub price: i128,
    pub creator_fee: i128,
    pub protocol_fee: i128,
    pub total_amount: i128,
}

/// Shared result type for read-only quote methods.
pub type QuoteViewResult = Result<QuoteResponse, ContractError>;

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
    pub holder_count: u32,
    pub fee_recipient: Address,
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

/// Reads a registered creator profile, returning an error when the creator is missing.
///
/// Use this helper for methods that require an existing creator and should return
/// a structured contract error instead of a default value.
pub fn read_registered_creator_profile(
    env: &Env,
    creator: &Address,
) -> Result<CreatorProfile, ContractError> {
    read_creator_profile(env, creator).ok_or(ContractError::NotRegistered)
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

fn read_protocol_fee_config(env: &Env) -> Option<fee::FeeConfig> {
    env.storage()
        .persistent()
        .get(&constants::storage::FEE_CONFIG)
}

fn read_required_protocol_fee_config(env: &Env) -> Result<fee::FeeConfig, ContractError> {
    read_protocol_fee_config(env).ok_or(ContractError::FeeConfigNotSet)
}

/// Formats a quote response with overflow-safe total amount calculation.
///
/// Returns `Err(ContractError::Overflow)` if any addition or subtraction would overflow.
fn checked_format_quote_response(
    price: i128,
    creator_fee: i128,
    protocol_fee: i128,
    is_buy: bool,
) -> QuoteViewResult {
    let fees = creator_fee
        .checked_add(protocol_fee)
        .ok_or(ContractError::Overflow)?;

    let total_amount = if is_buy {
        price.checked_add(fees).ok_or(ContractError::Overflow)?
    } else {
        price.checked_sub(fees).ok_or(ContractError::Overflow)?
    };

    Ok(QuoteResponse {
        price,
        creator_fee,
        protocol_fee,
        total_amount,
    })
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
            creator: creator.clone(),
            handle,
            supply: 0,
            holder_count: 0,
            fee_recipient: creator.clone(),
        };

        env.storage().persistent().set(&key, &profile);
        env.events().publish(
            (events::REGISTER_EVENT_NAME, profile.creator.clone()),
            events::CreatorRegisteredEvent {
                creator: profile.creator.clone(),
                handle: profile.handle.clone(),
                supply: profile.supply,
                holder_count: profile.holder_count,
            },
        );

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
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;

        if payment < price {
            return Err(ContractError::InsufficientPayment);
        }

        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

        let balance_key = DataKey::KeyBalance(creator.clone(), buyer.clone());
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        if current_balance == 0 {
            profile.holder_count = profile
                .holder_count
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
        }

        profile.supply = profile
            .supply
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;

        let key = DataKey::Creator(creator.clone());
        env.storage().persistent().set(&key, &profile);

        let new_balance = current_balance
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&balance_key, &new_balance);

        env.events().publish(
            (events::BUY_EVENT_NAME, creator, buyer),
            (profile.supply, payment),
        );

        Ok(profile.supply)
    }

    pub fn sell_key(env: Env, creator: Address, seller: Address) -> Result<u32, ContractError> {
        seller.require_auth();

        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

        let balance_key = DataKey::KeyBalance(creator.clone(), seller);
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if current_balance == 0 {
            return Err(ContractError::InsufficientBalance);
        }

        let new_balance = current_balance
            .checked_sub(1)
            .ok_or(ContractError::Overflow)?;
        profile.supply = profile
            .supply
            .checked_sub(1)
            .ok_or(ContractError::Overflow)?;

        if new_balance == 0 {
            profile.holder_count = profile
                .holder_count
                .checked_sub(1)
                .ok_or(ContractError::Overflow)?;
        }

        let key = DataKey::Creator(creator);
        env.storage().persistent().set(&key, &profile);
        env.storage().persistent().set(&balance_key, &new_balance);

        Ok(profile.supply)
    }

    pub fn get_key_balance(env: Env, creator: Address, wallet: Address) -> u32 {
        let key = DataKey::KeyBalance(creator, wallet);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Read-only view: returns a stable view of a holder's key count for a creator.
    ///
    /// Returns a [`HolderKeyCountView`] regardless of creator registration status.
    /// When the creator is not registered, `creator_exists` is `false` and `key_count` is `0`.
    /// When the creator exists but the holder has no keys, `key_count` is `0`.
    /// This method is designed for indexer-friendly consumption and avoids panics.
    pub fn get_holder_key_count(env: Env, creator: Address, holder: Address) -> HolderKeyCountView {
        let creator_exists = read_creator_profile(&env, &creator).is_some();
        let key_count = if creator_exists {
            let key = DataKey::KeyBalance(creator.clone(), holder.clone());
            env.storage().persistent().get(&key).unwrap_or(0)
        } else {
            0
        };

        HolderKeyCountView {
            creator,
            holder,
            key_count,
            creator_exists,
        }
    }

    pub fn get_creator(env: Env, creator: Address) -> Result<CreatorProfile, ContractError> {
        read_registered_creator_profile(&env, &creator)
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

    /// Read-only view: returns the current supply for a registered creator.
    ///
    /// Fails with [`ContractError::NotRegistered`] if the creator does not exist.
    pub fn get_creator_supply(env: Env, creator: Address) -> Result<u32, ContractError> {
        let profile = read_registered_creator_profile(&env, &creator)?;
        Ok(profile.supply)
    }

    /// Read-only view: returns the number of unique holders for a creator.
    ///
    /// Returns `0` if the creator is not registered, avoiding panics for
    /// invalid lookups. Uses the stored creator profile holder count.
    pub fn get_creator_holder_count(env: Env, creator: Address) -> u32 {
        read_creator_profile(&env, &creator)
            .map(|profile| profile.holder_count)
            .unwrap_or(0)
    }

    /// Read-only view: returns whether a creator is registered in the contract.
    ///
    /// Returns `true` if a [`CreatorProfile`] exists for the given address,
    /// `false` otherwise. Does not mutate state.
    pub fn is_creator_registered(env: Env, creator: Address) -> bool {
        read_creator_profile(&env, &creator).is_some()
    }

    /// Read-only view: returns the creator fee recipient address.
    ///
    /// Fails with [`ContractError::NotRegistered`] if the creator is not registered.
    /// Reuses current creator storage access patterns.
    pub fn get_creator_fee_recipient(env: Env, creator: Address) -> Result<Address, ContractError> {
        let profile = read_registered_creator_profile(&env, &creator)?;
        Ok(profile.fee_recipient)
    }

    /// Read-only view: returns the configured creator fee rate in basis points.
    ///
    /// The returned value is the creator-facing share stored in the current protocol
    /// fee configuration, scoped to a registered creator lookup.
    pub fn get_creator_fee_bps(env: Env, creator: Address) -> Result<u32, ContractError> {
        let _profile = read_registered_creator_profile(&env, &creator)?;
        let config = read_required_protocol_fee_config(&env)?;
        Ok(config.creator_bps)
    }

    /// Read-only view: returns the creator treasury share for a registered creator.
    ///
    /// Access Layer currently stores creator treasury share as the creator-facing
    /// basis-point share in protocol fee configuration. This method provides a
    /// creator-scoped accessor without mutating state.
    pub fn get_creator_treasury_share(env: Env, creator: Address) -> Result<u32, ContractError> {
        Self::get_creator_fee_bps(env, creator)
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
        env.storage()
            .persistent()
            .set(&constants::storage::FEE_CONFIG, &config);
        Ok(())
    }

    pub fn set_key_price(env: Env, admin: Address, price: i128) -> Result<(), ContractError> {
        admin.require_auth();
        if price <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        env.storage()
            .persistent()
            .set(&constants::storage::KEY_PRICE, &price);
        Ok(())
    }

    pub fn get_fee_config(env: Env) -> Option<fee::FeeConfig> {
        read_protocol_fee_config(&env)
    }

    /// Sets the protocol treasury address.
    ///
    /// Only callable by an authorized admin. Stores the treasury address used
    /// for protocol fee routing.
    pub fn set_treasury_address(env: Env, admin: Address, treasury: Address) {
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&constants::storage::TREASURY_ADDRESS, &treasury);
    }

    /// Read-only view: returns the current protocol treasury address.
    ///
    /// Returns `None` if no treasury address has been configured.
    /// Use this method for indexers and read-only callers that need the current
    /// treasury routing target.
    pub fn get_treasury_address(env: Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&constants::storage::TREASURY_ADDRESS)
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
            .get::<DataKey, fee::FeeConfig>(&constants::storage::FEE_CONFIG)
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
        let config = read_required_protocol_fee_config(&env)?;
        fee::checked_compute_fee_split(total, config.creator_bps, config.protocol_bps)
            .ok_or(ContractError::Overflow)
    }

    /// Read-only view: returns the fee configuration for a specific creator.
    ///
    /// Returns a stable [`CreatorFeeView`] regardless of whether the creator is registered
    /// or a fee config has been set. When `is_registered` is `false`, the creator does not
    /// exist and both bps fields are `0`. When `is_configured` is `false`, no global fee
    /// config has been set. Use this method for indexers and read-only callers that need
    /// a non-optional result.
    pub fn get_creator_fee_config(env: Env, creator: Address) -> CreatorFeeView {
        let is_registered = read_registered_creator_profile(&env, &creator).is_ok();

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
            .get::<DataKey, fee::FeeConfig>(&constants::storage::FEE_CONFIG)
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

    /// Read-only view: returns a quote for buying a key.
    ///
    /// Returns a [`QuoteResponse`] containing the current price and fee breakdown.
    /// Fees are calculated based on the fixed key price.
    pub fn get_buy_quote(env: Env, creator: Address) -> Result<QuoteResponse, ContractError> {
        let price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;

        if read_creator_profile(&env, &creator).is_none() {
            return Err(ContractError::NotRegistered);
        }

        let (creator_fee, protocol_fee) = Self::compute_fees_for_payment(env.clone(), price)?;

        checked_format_quote_response(price, creator_fee, protocol_fee, true)
    }

    /// Read-only view: returns a quote for selling a key.
    ///
    /// Returns a [`QuoteResponse`] containing the current price and fee breakdown.
    /// Fees are calculated based on the fixed key price.
    /// Rejects with [`ContractError::InsufficientBalance`] if the holder has no keys.
    pub fn get_sell_quote(
        env: Env,
        creator: Address,
        holder: Address,
    ) -> Result<QuoteResponse, ContractError> {
        let price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;

        if read_creator_profile(&env, &creator).is_none() {
            return Err(ContractError::NotRegistered);
        }

        let balance = Self::get_key_balance(env.clone(), creator, holder);
        if balance == 0 {
            return Err(ContractError::InsufficientBalance);
        }

        let (creator_fee, protocol_fee) = Self::compute_fees_for_payment(env.clone(), price)?;

        checked_format_quote_response(price, creator_fee, protocol_fee, false)
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
