#![no_std]
pub mod quote_view_errors;

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String};

pub mod events;
pub mod bonding_curve;
use bonding_curve::CurvePreset;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
/// Contract error variants.
///
/// # Stability and Ordering
///
/// **IMPORTANT**: New error variants MUST be appended to the end of this enum and NEVER
/// inserted mid-enum. The numeric discriminant values are part of the contract's ABI and
/// are exposed to clients, indexers, and monitoring tools.
///
/// ## Consequences of Reordering
///
/// If a variant is inserted mid-enum or existing variants are reordered:
/// - Existing clients that match on numeric error codes will break
/// - Indexers and monitoring tools will misinterpret error types
/// - Historical error logs will become inconsistent with current definitions
/// - Contract upgrades will introduce silent behavioral changes
///
/// ## Safe Extension Pattern
///
/// ✅ **Correct**: Append new variants at the end
/// ```rust,ignore
/// pub enum ContractError {
///     AlreadyRegistered = 1,
///     NotRegistered = 2,
///     // ... existing variants ...
///     InvalidHandleCharacter = 14,
///     NewError = 15,  // ✅ Safe: appended at end
/// }
/// ```
///
/// ❌ **Incorrect**: Insert mid-enum
/// ```rust,ignore
/// pub enum ContractError {
///     AlreadyRegistered = 1,
///     NewError = 2,  // ❌ BREAKS ABI: shifts all subsequent variants
///     NotRegistered = 3,  // was 2, now 3 - breaks existing clients
///     // ...
/// }
/// ```
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
    SellUnderflow = 10,
    ProtocolFeeExceedsCap = 11,
    HandleTooShort = 12,
    HandleTooLong = 13,
    InvalidHandleCharacter = 14,
    ZeroAddress = 15,
    SlippageExceeded = 16,
    ProtocolPaused = 17,
    Unauthorized = 18,
}

pub mod fee {
    use crate::ContractError;

    use soroban_sdk::contracttype;

    /// Basis points per 100% (10000 = 100%).
    pub const BPS_MAX: u32 = 10_000;

    /// Maximum safe amount to prevent overflow in fee calculations.
    pub const MAX_SAFE_AMOUNT: i128 = i128::MAX / BPS_MAX as i128;

    /// Maximum protocol share when configuring fees via [`assert_valid_fee_bps`].
    ///
    /// Caps the on-chain configured protocol take at 50% so fee settings stay within
    /// expected economic bounds before they affect market logic.
    pub const PROTOCOL_BPS_MAX: u32 = 5_000;

    #[derive(Clone, Eq, PartialEq)]
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

    /// Shared guard for fee config updates that need structured contract errors.
    pub fn assert_valid_fee_bps(creator_bps: u32, protocol_bps: u32) -> Result<(), ContractError> {
        let Some(sum) = creator_bps.checked_add(protocol_bps) else {
            return Err(ContractError::InvalidFeeConfig);
        };
        if sum != BPS_MAX {
            return Err(ContractError::InvalidFeeConfig);
        }
        if protocol_bps > PROTOCOL_BPS_MAX {
            return Err(ContractError::ProtocolFeeExceedsCap);
        }
        Ok(())
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

    /// Safely applies a percentage-based fee to an amount.
    ///
    /// Returns `None` if the multiplication overflows. Rounding is performed via
    /// floor division towards zero.
    pub fn apply_percentage_fee(amount: i128, bps: u32) -> Option<i128> {
        if amount <= 0 {
            return Some(0);
        }
        checked_div_i128(amount.checked_mul(bps as i128)?, BPS_MAX as i128)
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
        let protocol_amount = apply_percentage_fee(total, protocol_bps)?;
        let creator_amount = checked_sub_i128(total, protocol_amount)?;
        Some((creator_amount, protocol_amount))
    }

    /// Performs checked integer multiplication for quote math helpers.
    pub fn checked_mul_i128(a: i128, b: i128) -> Option<i128> {
        a.checked_mul(b)
    }

    /// Performs checked integer division for quote math helpers.
    pub fn checked_div_i128(dividend: i128, divisor: i128) -> Option<i128> {
        if divisor == 0 {
            return None;
        }
        dividend.checked_div(divisor)
    }

    /// Performs checked integer subtraction for quote math helpers.
    pub fn checked_sub_i128(left: i128, right: i128) -> Option<i128> {
        left.checked_sub(right)
    }

    /// Performs checked integer addition for quote math helpers.
    pub fn checked_add_i128(left: i128, right: i128) -> Option<i128> {
        left.checked_add(right)
    }

    /// Computes the checked sum of creator and protocol fee components.
    ///
    /// Returns `None` if the addition would overflow. Use this helper wherever
    /// fee components are combined before being compared against a price or total,
    /// to keep the overflow guard consistent across buy and sell quote paths.
    ///
    /// # Naming convention
    ///
    /// Quote helpers in this module follow a `checked_*` prefix convention:
    /// - `checked_*` functions return `Option<T>` and propagate `None` on overflow.
    /// - `compute_*` functions return the result directly (may panic on overflow in
    ///   debug builds; use only where inputs are already validated).
    /// - `apply_*` functions apply a rate or percentage to a single amount.
    ///
    /// `checked_fee_sum` belongs to the `checked_*` family: it is the canonical
    /// helper for summing two fee components before they are used in total-amount
    /// arithmetic, replacing ad-hoc inline `checked_add` calls at each call site.
    pub fn checked_fee_sum(creator_fee: i128, protocol_fee: i128) -> Option<i128> {
        creator_fee.checked_add(protocol_fee)
    }
}

pub mod constants {
    use super::DataKey;
    use soroban_sdk::Address;

    pub mod storage {
        use super::{creator_key, key_balance_key, DataKey};
        use soroban_sdk::Address;

        pub const FEE_CONFIG: DataKey = DataKey::FeeConfig;
        pub const KEY_PRICE: DataKey = DataKey::KeyPrice;
        pub const TREASURY_ADDRESS: DataKey = DataKey::TreasuryAddress;
        pub const ADMIN_ADDRESS: DataKey = DataKey::AdminAddress;
        pub const PROTOCOL_FEE_RECIPIENT: DataKey = DataKey::ProtocolFeeRecipient;
        pub const PROTOCOL_FEE_RECIPIENT_BALANCE: DataKey = DataKey::ProtocolFeeRecipientBalance;
        pub const PROTOCOL_STATE_VERSION: DataKey = DataKey::ProtocolStateVersion;
        pub const PAUSED: DataKey = DataKey::Paused;

        pub fn creator_fee_balance(creator: &Address) -> DataKey {
            DataKey::CreatorFeeBalance(creator.clone())
        }

        pub fn creator(creator: &Address) -> DataKey {
            creator_key(creator)
        }

        pub fn key_balance(creator: &Address, holder: &Address) -> DataKey {
            key_balance_key(creator, holder)
        }
    }

    fn creator_key(creator: &Address) -> DataKey {
        DataKey::Creator(creator.clone())
    }

    fn key_balance_key(creator: &Address, holder: &Address) -> DataKey {
        DataKey::KeyBalance(creator.clone(), holder.clone())
    }

    pub mod creator_reads {
        pub const DETAILS: &str = "get_creator_details";
        pub const FEE_BPS: &str = "get_creator_fee_bps";
        pub const FEE_CONFIG: &str = "get_creator_fee_config";
        pub const FEE_RECIPIENT: &str = "get_creator_fee_recipient";
        pub const FEE_RECIPIENT_BALANCE: &str = "get_creator_fee_balance";
        pub const HOLDER_KEY_COUNT: &str = "get_holder_key_count";
        pub const PROFILE: &str = "get_creator";
        pub const SUPPLY: &str = "get_creator_supply";
        pub const TREASURY_SHARE: &str = "get_creator_treasury_share";
        pub const NAME: &str = "get_key_name";
        pub const SYMBOL: &str = "get_key_symbol";
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
/// Returned by [`CreatorKeysContract::get_creator_details`] and
/// [`CreatorKeysContract::get_creators_batch`] for indexer-friendly consumption.
/// When `is_registered` is `false`, default values are returned for all other fields,
/// including `registered_at: 0`.
///
/// # Field Stability
///
/// Fields are append-only. Do not reorder existing fields; the Soroban XDR encoder
/// serialises struct fields in declaration order and downstream indexers rely on
/// positional stability.
#[derive(Clone)]
#[contracttype]
pub struct CreatorDetailsView {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
    pub is_registered: bool,
    /// Ledger sequence number at the time the creator registered.
    ///
    /// Set to `env.ledger().sequence()` inside [`CreatorKeysContract::register_creator`].
    /// Returns `0` for unregistered addresses so callers never receive an `Option`.
    /// Clients can use this field to sort a marketplace grid chronologically without
    /// maintaining a separate off-chain index.
    pub registered_at: u32,
    /// The bonding curve preset for this creator. Only meaningful when `is_registered` is `true`.
    /// Returns `CurvePreset::Linear` for unregistered creators.
    pub curve_preset: CurvePreset,
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

/// Initial protocol state version for read-only consumers.
///
/// The actual version is stored in storage and incremented on config updates.
/// This constant is only the starting value.
pub const PROTOCOL_STATE_VERSION_INITIAL: u32 = 1;

/// Decimal precision used by creator key values.
///
/// Matches the standard Soroban token decimal convention (7 decimal places).
pub const KEY_DECIMALS: u32 = 7;
pub const HANDLE_LEN_MIN: u32 = 3;
pub const HANDLE_LEN_MAX: u32 = 32;

/// Canonical storage key schema for persistent protocol state.
///
/// For quote-related key usage and invariants, see
/// [`docs/quote-storage-keys.md`](../../docs/quote-storage-keys.md).
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Creator(Address),
    FeeConfig,
    KeyPrice,
    KeyBalance(Address, Address),
    TreasuryAddress,
    AdminAddress,
    ProtocolFeeRecipient,
    ProtocolFeeRecipientBalance,
    CreatorFeeBalance(Address),
    ProtocolStateVersion,
    Paused,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
    pub holder_count: u32,
    pub fee_recipient: Address,
    /// Ledger sequence number captured at registration time via `env.ledger().sequence()`.
    ///
    /// Stored as the last field so existing serialised profiles written before this
    /// field was added deserialise correctly — the Soroban persistent storage layer
    /// reads structs by field index, so appending is the only safe extension pattern.
    pub registered_at: u32,
    /// Bonding curve preset selected at registration. Immutable after creation.
    /// Appended as the last field for backward-compatible deserialization.
    pub curve_preset: CurvePreset,
}

/// Reads a creator profile from storage, returning `None` for unregistered creators.
///
/// Use this helper wherever repeated creator read logic is needed to keep
/// missing-creator behavior consistent across the contract.
pub fn read_creator_profile(env: &Env, creator: &Address) -> Option<CreatorProfile> {
    let key = constants::storage::creator(creator);
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

/// Reads an empty string for use as a default in read-only view methods.
///
/// Use this helper wherever an empty string is needed to maintain consistency
/// and reduce duplication of string allocation logic.
pub fn read_none_string(env: &Env) -> String {
    String::from_str(env, "")
}

/// Reads the handle for a creator, returning an empty string for unregistered creators.
///
/// Use this helper wherever repeated handle read logic is needed to maintain
/// missing-handle behavior consistency across the contract.
pub fn read_creator_handle(env: &Env, creator: &Address) -> String {
    read_creator_profile(env, creator)
        .map(|p| p.handle)
        .unwrap_or_else(|| read_none_string(env))
}

/// Reads accrued creator fee balance for a creator, returning `0` when none is stored.
pub fn read_creator_fee_recipient_balance(env: &Env, creator: &Address) -> i128 {
    let key = constants::storage::creator_fee_balance(creator);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Credits `amount` to the creator fee recipient balance for `creator`.
fn credit_creator_fee_recipient_balance(
    env: &Env,
    creator: &Address,
    amount: i128,
) -> Result<(), ContractError> {
    if amount <= 0 {
        return Ok(());
    }
    let key = constants::storage::creator_fee_balance(creator);
    let current = read_creator_fee_recipient_balance(env, creator);
    let updated = current.checked_add(amount).ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&key, &updated);
    Ok(())
}

fn is_valid_handle_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
}

fn validate_creator_handle(handle: &String) -> Result<(), ContractError> {
    let len = handle.len();
    if len < HANDLE_LEN_MIN {
        return Err(ContractError::HandleTooShort);
    }
    if len > HANDLE_LEN_MAX {
        return Err(ContractError::HandleTooLong);
    }

    let mut bytes = [0u8; HANDLE_LEN_MAX as usize];
    handle.copy_into_slice(&mut bytes[..len as usize]);
    if bytes[..len as usize]
        .iter()
        .any(|byte| !is_valid_handle_byte(*byte))
    {
        return Err(ContractError::InvalidHandleCharacter);
    }

    Ok(())
}

fn is_paused(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get::<DataKey, bool>(&constants::storage::PAUSED)
        .unwrap_or(false)
}

fn assert_not_paused(env: &Env) -> Result<(), ContractError> {
    if is_paused(env) {
        return Err(ContractError::ProtocolPaused);
    }
    Ok(())
}

fn assert_is_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
    let admin: Address = env
        .storage()
        .persistent()
        .get(&constants::storage::ADMIN_ADDRESS)
        .ok_or(ContractError::Unauthorized)?;
    if *caller != admin {
        return Err(ContractError::Unauthorized);
    }
    Ok(())
}

fn read_protocol_fee_config(env: &Env) -> Option<fee::FeeConfig> {
    env.storage()
        .persistent()
        .get(&constants::storage::FEE_CONFIG)
}

/// Validates that an address is not the Stellar zero address.
///
/// The zero address (`GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF`)
/// is the all-zero public key. Setting it as a fee recipient would silently
/// burn all protocol fees. This helper rejects it at the point of assignment.
fn validate_non_zero_address(env: &Env, addr: &Address) -> Result<(), ContractError> {
    let zero_str = String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero_addr = Address::from_string(&zero_str);
    if *addr == zero_addr {
        return Err(ContractError::ZeroAddress);
    }
    Ok(())
}

fn read_required_protocol_fee_config(env: &Env) -> Result<fee::FeeConfig, ContractError> {
    read_protocol_fee_config(env).ok_or(ContractError::FeeConfigNotSet)
}

fn read_protocol_fee_recipient_balance(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&constants::storage::PROTOCOL_FEE_RECIPIENT_BALANCE)
        .unwrap_or(0)
}

fn credit_protocol_fee_recipient_balance(env: &Env, amount: i128) -> Result<(), ContractError> {
    if amount <= 0 {
        return Ok(());
    }
    let updated = read_protocol_fee_recipient_balance(env)
        .checked_add(amount)
        .ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(
        &constants::storage::PROTOCOL_FEE_RECIPIENT_BALANCE,
        &updated,
    );
    Ok(())
}

fn assert_buy_price_slippage(price: i128, max_price: Option<i128>) -> Result<(), ContractError> {
    if let Some(max) = max_price {
        if price > max {
            return Err(ContractError::SlippageExceeded);
        }
    }
    Ok(())
}

fn compute_sell_proceeds(env: &Env, price: i128) -> Result<i128, ContractError> {
    let (creator_fee, protocol_fee) =
        CreatorKeysContract::compute_fees_for_payment(env.clone(), price)?;
    let fees = fee::checked_fee_sum(creator_fee, protocol_fee).ok_or(ContractError::Overflow)?;
    fee::checked_sub_i128(price, fees).ok_or(ContractError::SellUnderflow)
}

fn assert_sell_proceeds_slippage(
    env: &Env,
    min_proceeds: Option<i128>,
) -> Result<(), ContractError> {
    if let Some(min) = min_proceeds {
        let price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        let proceeds = compute_sell_proceeds(env, price)?;
        if proceeds < min {
            return Err(ContractError::SlippageExceeded);
        }
    }
    Ok(())
}

fn accrue_sell_protocol_fee(env: &Env) -> Result<(), ContractError> {
    if env
        .storage()
        .persistent()
        .get::<DataKey, Address>(&constants::storage::PROTOCOL_FEE_RECIPIENT)
        .is_none()
    {
        return Ok(());
    }

    let Some(price) = env
        .storage()
        .persistent()
        .get(&constants::storage::KEY_PRICE)
    else {
        return Ok(());
    };

    if read_protocol_fee_config(env).is_none() {
        return Ok(());
    }

    let (_, protocol_fee) = CreatorKeysContract::compute_fees_for_payment(env.clone(), price)?;
    credit_protocol_fee_recipient_balance(env, protocol_fee)
}

/// Resolves and validates the shared inputs required by read-only quote methods.
///
/// Reads the key price from storage and confirms the creator is registered.
/// Returns `(price)` on success, or the appropriate [`ContractError`] on failure.
fn resolve_quote_inputs(env: &Env, creator: &Address) -> Result<Option<i128>, ContractError> {
    let price: i128 = env
        .storage()
        .persistent()
        .get(&constants::storage::KEY_PRICE)
        .ok_or(ContractError::KeyPriceNotSet)?;

    if read_creator_profile(env, creator).is_none() {
        return Err(ContractError::NotRegistered);
    }

    normalize_quote_amount(price)
}

/// Normalizes quote amounts before fee math is applied.
///
/// Zero-value quote requests are treated as no-op quotes and return `None`.
/// Negative quote amounts are rejected consistently across buy and sell paths.
/// Amounts exceeding MAX_SAFE_AMOUNT are rejected to prevent overflow in fee calculations.
fn normalize_quote_amount(amount: i128) -> Result<Option<i128>, ContractError> {
    if amount < 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    if amount == 0 {
        return Ok(None);
    }

    if amount > fee::MAX_SAFE_AMOUNT {
        return Err(ContractError::Overflow);
    }

    Ok(Some(amount))
}

fn zero_quote_response() -> QuoteResponse {
    QuoteResponse {
        price: 0,
        creator_fee: 0,
        protocol_fee: 0,
        total_amount: 0,
    }
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
    let fees = fee::checked_fee_sum(creator_fee, protocol_fee).ok_or(ContractError::Overflow)?;

    let total_amount = if is_buy {
        price.checked_add(fees).ok_or(ContractError::Overflow)?
    } else {
        fee::checked_sub_i128(price, fees).ok_or(ContractError::SellUnderflow)?
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
    /// Registers a new creator profile. This is a contract initialization
    /// entrypoint; the contract has no single `initialize` call, so the
    /// init-time parameter validation lives on the individual setters.
    ///
    /// Parameter validation:
    /// - `creator`: must authorize the call (`require_auth`). A profile must not
    ///   already exist for this address, otherwise
    ///   [`ContractError::AlreadyRegistered`].
    /// - `handle`: validated by [`validate_creator_handle`] — below the minimum
    ///   length returns [`ContractError::HandleTooShort`], above the maximum
    ///   returns [`ContractError::HandleTooLong`], and any disallowed byte
    ///   returns [`ContractError::InvalidHandleCharacter`].
    pub fn register_creator(
        env: Env,
        creator: Address,
        handle: String,
        curve_preset: Option<CurvePreset>,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        assert_not_paused(&env)?;

        validate_creator_handle(&handle)?;

        let key = constants::storage::creator(&creator);
        // Creator profile storage is a single source of truth keyed by creator address.
        // Once written, this key's existence is the registration invariant.
        if env.storage().persistent().has(&key) {
            return Err(ContractError::AlreadyRegistered);
        }

        let preset = curve_preset.unwrap_or_default(); // Linear if omitted

        let profile = CreatorProfile {
            creator: creator.clone(),
            handle,
            supply: 0,
            holder_count: 0,
            fee_recipient: creator.clone(),
            registered_at: env.ledger().sequence(),
            curve_preset: preset,
        };

        let fee_config = read_protocol_fee_config(&env).unwrap_or(fee::FeeConfig {
            creator_bps: 0,
            protocol_bps: 0,
        });

        // Persist profile before event publication so indexers reading contract state
        // after this tx observe the same registration payload that was emitted.
        env.storage().persistent().set(&key, &profile);
        env.events().publish(
            events::register_event_topics(&profile.creator),
            events::CreatorRegisteredEvent {
                creator: profile.creator.clone(),
                handle: profile.handle.clone(),
                supply: profile.supply,
                holder_count: profile.holder_count,
                creator_bps: fee_config.creator_bps,
                protocol_bps: fee_config.protocol_bps,
                curve_preset: preset,
            },
        );

        Ok(())
    }

    pub fn buy_key(
    env: Env,
    creator: Address,
    buyer: Address,
    payment: i128,
    max_price: Option<i128>,
) -> Result<u32, ContractError> {
    buyer.require_auth();
    assert_not_paused(&env)?;

    if payment <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

    // NEW: compute price based on current supply and curve preset
    let price = bonding_curve::compute_price(profile.supply, 1, profile.curve_preset)
        .ok_or(ContractError::Overflow)?;

    assert_buy_price_slippage(price, max_price)?;

    if payment < price {
        return Err(ContractError::InsufficientPayment);
    }

    let balance_key = constants::storage::key_balance(&creator, &buyer);
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

    let key = constants::storage::creator(&creator);
    env.storage().persistent().set(&key, &profile);

    let new_balance = current_balance
        .checked_add(1)
        .ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&balance_key, &new_balance);

    if let Some(config) = read_protocol_fee_config(&env) {
        let (creator_fee, protocol_fee) =
            fee::checked_compute_fee_split(price, config.creator_bps, config.protocol_bps)
                .ok_or(ContractError::Overflow)?;
        credit_creator_fee_recipient_balance(&env, &creator, creator_fee)?;
        credit_protocol_fee_recipient_balance(&env, protocol_fee)?;
    }

    env.events().publish(
        events::buy_event_topics(&creator, &buyer),
        (profile.supply, payment),
    );

    Ok(profile.supply)
}

    pub fn sell_key(
    env: Env,
    creator: Address,
    seller: Address,
    min_proceeds: Option<i128>,
) -> Result<u32, ContractError> {
    seller.require_auth();
    assert_not_paused(&env)?;

    let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

    let balance_key = constants::storage::key_balance(&creator, &seller);
    let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
    if current_balance == 0 {
        return Err(ContractError::InsufficientBalance);
    }

    // NEW: compute sell price based on current supply (before decrement) and curve preset
    let price = bonding_curve::compute_price(profile.supply - 1, 1, profile.curve_preset)
        .ok_or(ContractError::Overflow)?;

    // Compute proceeds for slippage check
    let (creator_fee, protocol_fee) =
        Self::compute_fees_for_payment(env.clone(), price)?;
    let fees = fee::checked_fee_sum(creator_fee, protocol_fee).ok_or(ContractError::Overflow)?;
    let proceeds = fee::checked_sub_i128(price, fees).ok_or(ContractError::SellUnderflow)?;

    if let Some(min) = min_proceeds {
        if proceeds < min {
            return Err(ContractError::SlippageExceeded);
        }
    }

    let new_balance = current_balance
        .checked_sub(1)
        .ok_or(ContractError::SellUnderflow)?;
    profile.supply = profile
        .supply
        .checked_sub(1)
        .ok_or(ContractError::SellUnderflow)?;

    if new_balance == 0 {
        profile.holder_count = profile
            .holder_count
            .checked_sub(1)
            .ok_or(ContractError::SellUnderflow)?;
    }

    let key = constants::storage::creator(&creator);
    env.storage().persistent().set(&key, &profile);
    env.storage().persistent().set(&balance_key, &new_balance);

    // Accrue fees based on computed price
    if let Some(config) = read_protocol_fee_config(&env) {
        let (creator_fee, protocol_fee) =
            fee::checked_compute_fee_split(price, config.creator_bps, config.protocol_bps)
                .ok_or(ContractError::Overflow)?;
        credit_creator_fee_recipient_balance(&env, &creator, creator_fee)?;
        credit_protocol_fee_recipient_balance(&env, protocol_fee)?;
    }

    env.events()
        .publish((events::SELL_EVENT_NAME, creator, seller), profile.supply);

    Ok(profile.supply)
}

    /// Halts all state-changing operations (buy, sell, register_creator).
    ///
    /// Only the protocol admin may call this. Emits a `ProtocolPaused` event.
    /// Read-only view functions are unaffected and continue to work while paused.
    pub fn pause(env: Env, admin: Address) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&constants::storage::PAUSED, &true);
        env.events().publish((events::PAUSE_EVENT_NAME, admin), ());
        Ok(())
    }

    /// Resumes all state-changing operations after an emergency pause.
    ///
    /// Only the protocol admin may call this. Emits a `ProtocolUnpaused` event.
    pub fn unpause(env: Env, admin: Address) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&constants::storage::PAUSED, &false);
        env.events()
            .publish((events::UNPAUSE_EVENT_NAME, admin), ());
        Ok(())
    }

    /// Read-only view: returns whether the protocol is currently paused.
    pub fn get_is_paused(env: Env) -> bool {
        is_paused(&env)
    }

    pub fn get_key_balance(env: Env, creator: Address, wallet: Address) -> u32 {
        let key = constants::storage::key_balance(&creator, &wallet);
        // Read-only callers get `0` for unseen balances to avoid sparse-map lookups failing.
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
            let key = constants::storage::key_balance(&creator, &holder);
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
    /// default values are provided for other fields, including `registered_at: 0`.
    pub fn get_creator_details(env: Env, creator: Address) -> CreatorDetailsView {
        let key = constants::storage::creator(&creator);
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
                registered_at: profile.registered_at,
                curve_preset: profile.curve_preset,
            },
            None => CreatorDetailsView {
                creator,
                handle: read_none_string(&env),
                supply: 0,
                is_registered: false,
                registered_at: 0,
                curve_preset: CurvePreset::Linear, // Default for unregistered
            },
        }
    }

    /// Read-only batch view: returns [`CreatorDetailsView`] for each address in `creators`.
    ///
    /// Iterates the provided addresses in order and fetches each creator's profile
    /// from persistent storage. The output `Vec` is the same length as the input and
    /// preserves input order, so clients can zip the two slices without an extra sort.
    ///
    /// Unregistered addresses never cause the call to fail: they produce a default
    /// [`CreatorDetailsView`] with `is_registered: false` and `registered_at: 0`,
    /// matching the single-address behaviour of [`get_creator_details`].
    ///
    /// # Usage
    ///
    /// ```text
    /// let views = client.get_creators_batch(&vec![alice, bob, unknown]);
    /// // views[0] → alice's details (is_registered: true)
    /// // views[1] → bob's details   (is_registered: true)
    /// // views[2] → default view    (is_registered: false, registered_at: 0)
    /// ```
    pub fn get_creators_batch(
        env: Env,
        creators: soroban_sdk::Vec<Address>,
    ) -> soroban_sdk::Vec<CreatorDetailsView> {
        let mut results = soroban_sdk::Vec::new(&env);
        for creator in creators.iter() {
            let key = constants::storage::creator(&creator);
            let view = match env
                .storage()
                .persistent()
                .get::<DataKey, CreatorProfile>(&key)
            {
                Some(profile) => CreatorDetailsView {
                    creator: profile.creator,
                    handle: profile.handle,
                    supply: profile.supply,
                    is_registered: true,
                    registered_at: profile.registered_at,
                    curve_preset: profile.curve_preset,
                },
                None => CreatorDetailsView {
                    creator,
                    handle: read_none_string(&env),
                    supply: 0,
                    is_registered: false,
                    registered_at: 0,
                    curve_preset: CurvePreset::Linear, // Default for unregistered
                },
            };
            results.push_back(view);
        }
        results
    }
    /// Read-only view: returns the protocol state version.
    ///
    /// Returns a stable scalar value for clients and indexers to detect
    /// protocol-state schema/semantics revisions. The version is stored in
    /// storage and increments on config updates.
    pub fn get_protocol_state_version(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&constants::storage::PROTOCOL_STATE_VERSION)
            .unwrap_or(PROTOCOL_STATE_VERSION_INITIAL)
    }

    /// Read-only view: returns the decimal precision used by creator key values.
    ///
    /// Returns the fixed [`KEY_DECIMALS`] constant. Does not read or mutate contract state.
    pub fn get_key_decimals(_env: Env) -> u32 {
        KEY_DECIMALS
    }

    /// Read-only view: returns the display name for a creator's key.
    ///
    /// Returns the creator's handle for registered creators. Fails with
    /// [`ContractError::NotRegistered`] if the creator is not registered.
    pub fn get_key_name(env: Env, creator: Address) -> Result<String, ContractError> {
        let profile = read_registered_creator_profile(&env, &creator)?;
        Ok(profile.handle)
    }

    /// Read-only view: returns the ticker symbol for a creator's key.
    ///
    /// Returns the creator's handle for registered creators. Fails with
    /// [`ContractError::NotRegistered`] if the creator is not registered.
    pub fn get_key_symbol(env: Env, creator: Address) -> Result<String, ContractError> {
        let profile = read_registered_creator_profile(&env, &creator)?;
        Ok(profile.handle)
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

    /// Read-only view: returns accrued creator fee balance for the creator's fee recipient.
    ///
    /// Fails with [`ContractError::NotRegistered`] if the creator is not registered.
    /// Returns `0` when no buy has accrued fees yet.
    pub fn get_creator_fee_balance(env: Env, creator: Address) -> Result<i128, ContractError> {
        read_registered_creator_profile(&env, &creator)?;
        Ok(read_creator_fee_recipient_balance(&env, &creator))
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

    /// Read-only view: returns the curve preset for a registered creator.
///
/// Fails with [`ContractError::NotRegistered`] if the creator does not exist.
pub fn get_curve_preset(env: Env, creator: Address) -> Result<CurvePreset, ContractError> {
    let profile = read_registered_creator_profile(&env, &creator)?;
    Ok(profile.curve_preset)
}

    /// Read-only view: returns the configured protocol treasury share in basis points.
    ///
    /// This value is sourced from the current protocol fee configuration and is
    /// expressed in stable basis-point units.
    pub fn get_protocol_treasury_share_bps(env: Env) -> Result<u32, ContractError> {
        let config = read_required_protocol_fee_config(&env)?;
        Ok(config.protocol_bps)
    }

    /// Sets the global protocol/creator fee split. Contract initialization
    /// entrypoint.
    ///
    /// Parameter validation (via [`fee::assert_valid_fee_bps`]):
    /// - `admin`: must authorize the call (`require_auth`).
    /// - `creator_bps` + `protocol_bps`: must sum to exactly `BPS_MAX` (10_000),
    ///   otherwise [`ContractError::InvalidFeeConfig`].
    /// - `protocol_bps`: must not exceed `PROTOCOL_BPS_MAX`, otherwise
    ///   [`ContractError::ProtocolFeeExceedsCap`].
    pub fn set_fee_config(
        env: Env,
        admin: Address,
        creator_bps: u32,
        protocol_bps: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        fee::assert_valid_fee_bps(creator_bps, protocol_bps)?;

        let config = fee::FeeConfig {
            creator_bps,
            protocol_bps,
        };
        if env
            .storage()
            .persistent()
            .get::<DataKey, fee::FeeConfig>(&constants::storage::FEE_CONFIG)
            .as_ref()
            == Some(&config)
        {
            return Ok(());
        }
        env.storage()
            .persistent()
            .set(&constants::storage::FEE_CONFIG, &config);

        // Increment protocol state version on config update
        let current_version = env
            .storage()
            .persistent()
            .get(&constants::storage::PROTOCOL_STATE_VERSION)
            .unwrap_or(PROTOCOL_STATE_VERSION_INITIAL);
        let new_version = current_version
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;
        env.storage()
            .persistent()
            .set(&constants::storage::PROTOCOL_STATE_VERSION, &new_version);

        Ok(())
    }

    /// Sets the per-key price. Contract initialization entrypoint.
    ///
    /// Parameter validation:
    /// - `admin`: must authorize the call (`require_auth`).
    /// - `price`: must be strictly positive; zero or negative returns
    ///   [`ContractError::NotPositiveAmount`].
    pub fn set_key_price(env: Env, admin: Address, price: i128) -> Result<(), ContractError> {
        admin.require_auth();
        if price <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        if env
            .storage()
            .persistent()
            .get::<DataKey, i128>(&constants::storage::KEY_PRICE)
            .as_ref()
            == Some(&price)
        {
            return Ok(());
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
        if env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&constants::storage::TREASURY_ADDRESS)
            .as_ref()
            == Some(&treasury)
        {
            return;
        }
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

    /// Sets the protocol admin address.
    ///
    /// Only callable by an authorized admin. Stores the admin address used
    /// for protocol administration.
    pub fn set_protocol_admin(env: Env, admin: Address, new_admin: Address) {
        admin.require_auth();
        if env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&constants::storage::ADMIN_ADDRESS)
            .as_ref()
            == Some(&new_admin)
        {
            return;
        }
        env.storage()
            .persistent()
            .set(&constants::storage::ADMIN_ADDRESS, &new_admin);
    }

    /// Read-only view: returns the current protocol admin address.
    ///
    /// Returns `None` if no admin address has been configured.
    /// Use this method for indexers and read-only callers that need the current
    /// protocol admin address.
    pub fn get_protocol_admin(env: Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&constants::storage::ADMIN_ADDRESS)
    }

    /// Read-only view: returns the current protocol fee recipient address.
    ///
    /// Returns `None` if no protocol fee recipient address has been configured.
    /// Use this method for indexers and read-only callers that need the current
    /// protocol fee recipient address.
    pub fn get_protocol_fee_recipient(env: Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&constants::storage::PROTOCOL_FEE_RECIPIENT)
    }

    /// Read-only view: returns the accrued protocol fee balance for the configured recipient.
    ///
    /// Returns `0` when no protocol fees have been accrued from sell execution.
    pub fn get_protocol_recipient_balance(env: Env) -> i128 {
        read_protocol_fee_recipient_balance(&env)
    }

    /// Sets the protocol fee recipient address.
    ///
    /// Only callable by an authorized admin. Rejects the Stellar zero address
    /// to prevent silent fee burning.
    ///
    /// Parameter validation:
    /// - `admin`: must authorize the call (`require_auth`).
    /// - `recipient`: must not be the Stellar zero address, otherwise
    ///   [`ContractError::ZeroAddress`].
    pub fn set_protocol_fee_recipient(
        env: Env,
        admin: Address,
        recipient: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        validate_non_zero_address(&env, &recipient)?;

        if env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&constants::storage::PROTOCOL_FEE_RECIPIENT)
            .as_ref()
            == Some(&recipient)
        {
            return Ok(());
        }
        env.storage()
            .persistent()
            .set(&constants::storage::PROTOCOL_FEE_RECIPIENT, &recipient);
        Ok(())
    }

    /// Read-only view: returns whether protocol configuration has been initialized.
    ///
    /// Returns `true` once a protocol fee configuration has been stored and `false`
    /// otherwise. Does not mutate contract state.
    pub fn is_protocol_config_initialized(env: Env) -> bool {
        read_protocol_fee_config(&env).is_some()
    }

    /// Read-only view: returns the current protocol fee configuration.
    ///
    /// Returns a stable [`ProtocolFeeView`] regardless of whether a fee config has been set.
    /// When no config is stored, `is_configured` is `false` and both bps fields are `0`.
    /// Use this method for indexers and read-only callers that need a non-optional result.
    pub fn get_protocol_fee_view(env: Env) -> ProtocolFeeView {
        match read_protocol_fee_config(&env) {
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
    let profile = read_registered_creator_profile(&env, &creator)?;

    let price = bonding_curve::compute_price(profile.supply, 1, profile.curve_preset)
        .ok_or(ContractError::Overflow)?;

    if price == 0 {
        return Ok(zero_quote_response());
    }

    if price > fee::MAX_SAFE_AMOUNT {
        return Err(ContractError::Overflow);
    }

    let (creator_fee, protocol_fee) = Self::compute_fees_for_payment(env.clone(), price)?;
    checked_format_quote_response(price, creator_fee, protocol_fee, true)
}
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
    let profile = read_registered_creator_profile(&env, &creator)?;

    let balance = Self::get_key_balance(env.clone(), creator.clone(), holder);
    if balance == 0 {
        return Err(ContractError::InsufficientBalance);
    }

    let price = bonding_curve::compute_price(profile.supply - 1, 1, profile.curve_preset)
        .ok_or(ContractError::Overflow)?;

    if price == 0 {
        return Ok(zero_quote_response());
    }

    if price > fee::MAX_SAFE_AMOUNT {
        return Err(ContractError::Overflow);
    }

    let (creator_fee, protocol_fee) = Self::compute_fees_for_payment(env.clone(), price)?;
    checked_format_quote_response(price, creator_fee, protocol_fee, false)
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

    #[test]
    fn test_checked_mul_i128_success() {
        assert_eq!(fee::checked_mul_i128(100, 10), Some(1000));
    }

    #[test]
    fn test_checked_mul_i128_rejects_overflow() {
        assert_eq!(fee::checked_mul_i128(i128::MAX, 2), None);
        assert_eq!(fee::checked_mul_i128(i128::MIN, 2), None);
    }

    #[test]
    fn test_checked_div_i128_success() {
        assert_eq!(fee::checked_div_i128(100, 10), Some(10));
    }

    #[test]
    fn test_checked_div_i128_rejects_zero_divisor() {
        assert_eq!(fee::checked_div_i128(100, 0), None);
    }

    #[test]
    fn test_checked_sub_i128_success() {
        assert_eq!(fee::checked_sub_i128(100, 10), Some(90));
    }

    #[test]
    fn test_checked_sub_i128_underflow() {
        assert_eq!(fee::checked_sub_i128(i128::MIN, 1), None);
    }

    #[test]
    fn test_checked_add_i128_success() {
        assert_eq!(fee::checked_add_i128(100, 10), Some(110));
    }

    #[test]
    fn test_checked_add_i128_overflow() {
        assert_eq!(fee::checked_add_i128(i128::MAX, 1), None);
    }

    #[test]
    fn test_checked_add_i128_zero() {
        assert_eq!(fee::checked_add_i128(0, 0), Some(0));
        assert_eq!(fee::checked_add_i128(100, 0), Some(100));
        assert_eq!(fee::checked_add_i128(0, 100), Some(100));
    }

    #[test]
    fn test_checked_add_i128_negative_values() {
        assert_eq!(fee::checked_add_i128(-10, 20), Some(10));
        assert_eq!(fee::checked_add_i128(10, -20), Some(-10));
        assert_eq!(fee::checked_add_i128(-10, -10), Some(-20));
    }

    #[test]
    fn test_checked_add_i128_boundary_values() {
        assert_eq!(fee::checked_add_i128(i128::MAX, 0), Some(i128::MAX));
        assert_eq!(fee::checked_add_i128(i128::MIN, 0), Some(i128::MIN));
        assert_eq!(fee::checked_add_i128(0, i128::MAX), Some(i128::MAX));
        assert_eq!(fee::checked_add_i128(0, i128::MIN), Some(i128::MIN));
    }

    #[test]
    fn test_checked_add_i128_deterministic_error() {
        // Verify that overflow always returns None, never panics
        assert_eq!(fee::checked_add_i128(i128::MAX, i128::MAX), None);
        assert_eq!(fee::checked_add_i128(i128::MIN, i128::MIN), None);
    }

    #[test]
    fn test_checked_div_i128_rejects_overflow() {
        assert_eq!(fee::checked_div_i128(i128::MIN, -1), None);
    }

    /// Both operands at `i128::MAX / 2 + 1` must overflow.
    ///
    /// `(i128::MAX / 2 + 1) + (i128::MAX / 2 + 1) == i128::MAX + 1`, which
    /// exceeds i128 capacity, so the helper must return `None` rather than wrap.
    #[test]
    fn test_checked_add_i128_both_at_half_max_plus_one_overflows() {
        let half_plus_one = i128::MAX / 2 + 1;
        assert_eq!(fee::checked_add_i128(half_plus_one, half_plus_one), None);
    }

    /// Both operands at `i128::MAX / 2` must not overflow.
    ///
    /// `(i128::MAX / 2) + (i128::MAX / 2) == i128::MAX - 1`, which fits in i128,
    /// so the helper must return the correct sum just below the overflow boundary.
    #[test]
    fn test_checked_add_i128_both_at_half_max_succeeds() {
        let half = i128::MAX / 2;
        assert_eq!(fee::checked_add_i128(half, half), Some(half + half));
    }

    #[test]
    fn test_normalize_quote_amount_preserves_positive_amount() {
        assert_eq!(super::normalize_quote_amount(100), Ok(Some(100)));
    }

    #[test]
    fn test_normalize_quote_amount_maps_zero_to_noop() {
        assert_eq!(super::normalize_quote_amount(0), Ok(None));
    }

    #[test]
    fn test_normalize_quote_amount_rejects_negative_amount() {
        assert_eq!(
            super::normalize_quote_amount(-1),
            Err(super::ContractError::NotPositiveAmount)
        );
    }

    #[test]
    fn test_normalize_quote_amount_rejects_large_amount() {
        let large = super::fee::MAX_SAFE_AMOUNT + 1;
        assert_eq!(
            super::normalize_quote_amount(large),
            Err(super::ContractError::Overflow)
        );
    }

    #[test]
    fn test_checked_format_quote_response_buy_success() {
        let res = super::checked_format_quote_response(1000, 90, 10, true).unwrap();
        assert_eq!(res.price, 1000);
        assert_eq!(res.creator_fee, 90);
        assert_eq!(res.protocol_fee, 10);
        assert_eq!(res.total_amount, 1100);
    }

    #[test]
    fn test_checked_format_quote_response_sell_success() {
        let res = super::checked_format_quote_response(1000, 90, 10, false).unwrap();
        assert_eq!(res.price, 1000);
        assert_eq!(res.creator_fee, 90);
        assert_eq!(res.protocol_fee, 10);
        assert_eq!(res.total_amount, 900);
    }

    #[test]
    fn test_checked_format_quote_response_buy_overflow_fees() {
        let res = super::checked_format_quote_response(1000, i128::MAX, 1, true);
        assert_eq!(res, Err(super::ContractError::Overflow));
    }

    #[test]
    fn test_checked_format_quote_response_buy_overflow_total() {
        let res = super::checked_format_quote_response(i128::MAX, 1, 0, true);
        assert_eq!(res, Err(super::ContractError::Overflow));
    }

    #[test]
    fn test_checked_format_quote_response_sell_underflow_total() {
        let res = super::checked_format_quote_response(i128::MIN, 1, 0, false);
        assert_eq!(res, Err(super::ContractError::SellUnderflow));
    }

    #[test]
    fn test_apply_percentage_fee_success() {
        assert_eq!(fee::apply_percentage_fee(1000, 1000), Some(100));
        assert_eq!(fee::apply_percentage_fee(1000, 0), Some(0));
        assert_eq!(fee::apply_percentage_fee(1000, 10000), Some(1000));
    }

    #[test]
    fn test_apply_percentage_fee_zero_amount() {
        assert_eq!(fee::apply_percentage_fee(0, 1000), Some(0));
    }

    #[test]
    fn test_apply_percentage_fee_negative_amount() {
        assert_eq!(fee::apply_percentage_fee(-100, 1000), Some(0));
    }

    #[test]
    fn test_apply_percentage_fee_rounding() {
        // 999 * 1000 / 10000 = 99.9 -> 99
        assert_eq!(fee::apply_percentage_fee(999, 1000), Some(99));
    }

    #[test]
    fn test_apply_percentage_fee_overflow() {
        // Multiplication overflows before division
        assert_eq!(fee::apply_percentage_fee(i128::MAX, 2), None);
    }

    #[test]
    fn test_assert_valid_fee_bps() {
        // Valid scenarios
        assert_eq!(fee::assert_valid_fee_bps(10000, 0), Ok(()));
        assert_eq!(fee::assert_valid_fee_bps(5000, 5000), Ok(()));
        assert_eq!(fee::assert_valid_fee_bps(9000, 1000), Ok(()));

        // Invalid Sum
        assert_eq!(
            fee::assert_valid_fee_bps(9000, 2000),
            Err(super::ContractError::InvalidFeeConfig)
        );
        assert_eq!(
            fee::assert_valid_fee_bps(0, 0),
            Err(super::ContractError::InvalidFeeConfig)
        );

        // Protocol Cap Exceeded (PROTOCOL_BPS_MAX = 5000)
        assert_eq!(
            fee::assert_valid_fee_bps(4999, 5001),
            Err(super::ContractError::ProtocolFeeExceedsCap)
        );
        assert_eq!(
            fee::assert_valid_fee_bps(0, 10000),
            Err(super::ContractError::ProtocolFeeExceedsCap)
        );

        // Overflow
        assert_eq!(
            fee::assert_valid_fee_bps(u32::MAX, 1),
            Err(super::ContractError::InvalidFeeConfig)
        );
    }

    #[test]
    fn test_validate_fee_bps() {
        // Valid
        assert!(fee::validate_fee_bps(10000, 0));
        assert!(fee::validate_fee_bps(5000, 5000));
        assert!(fee::validate_fee_bps(9000, 1000));

        // Invalid Sum
        assert!(!fee::validate_fee_bps(9000, 2000));
        assert!(!fee::validate_fee_bps(0, 0));

        // Protocol Cap Exceeded
        assert!(!fee::validate_fee_bps(4999, 5001));

        // Overflow
        assert!(!fee::validate_fee_bps(u32::MAX, 1));
    }

    // --- checked_fee_sum unit tests ---

    /// Verifies that `checked_fee_sum` returns the correct sum for two ordinary
    /// positive fee components.
    #[test]
    fn test_checked_fee_sum_success() {
        assert_eq!(fee::checked_fee_sum(900, 100), Some(1000));
        assert_eq!(fee::checked_fee_sum(0, 0), Some(0));
        assert_eq!(fee::checked_fee_sum(500, 500), Some(1000));
    }

    /// Verifies that `checked_fee_sum` returns `None` when the addition would
    /// overflow `i128`, preventing silent wrapping in fee total calculations.
    #[test]
    fn test_checked_fee_sum_overflow_returns_none() {
        assert_eq!(fee::checked_fee_sum(i128::MAX, 1), None);
        assert_eq!(fee::checked_fee_sum(i128::MAX, i128::MAX), None);
    }

    /// Edge case: verifies `checked_fee_sum` at the boundary where one component
    /// is exactly `i128::MAX` and the other is zero — the only non-overflowing
    /// case at that boundary.
    #[test]
    fn test_checked_fee_sum_boundary_max_plus_zero() {
        assert_eq!(fee::checked_fee_sum(i128::MAX, 0), Some(i128::MAX));
        assert_eq!(fee::checked_fee_sum(0, i128::MAX), Some(i128::MAX));
        // One above the boundary must overflow
        assert_eq!(fee::checked_fee_sum(i128::MAX, 1), None);
    }

    // --- BPS truncation on small amounts ---

    /// Bps calculation on very small amounts produces zero due to integer division
    /// truncation. These tests document the behavior at the lower precision boundary.
    ///
    /// Formula: `amount * bps / 10_000` (floor division).
    /// When the product `amount * bps < 10_000`, the result truncates to zero.
    #[test]
    fn test_apply_percentage_fee_truncation_1_stroop() {
        // 1 * 1000 / 10_000 = 0.1 → truncated to 0
        // At 1 stroop with 10% bps, the fee is zero — value is silently lost.
        let result = fee::apply_percentage_fee(1, 1000);
        assert_eq!(result, Some(0), "1 stroop at 1000 bps truncates to 0");
    }

    #[test]
    fn test_apply_percentage_fee_truncation_10_stroops() {
        // 10 * 1000 / 10_000 = 1.0 → exactly 1
        // At 10 stroops with 10% bps, the fee is exactly 1.
        let result = fee::apply_percentage_fee(10, 1000);
        assert_eq!(result, Some(1), "10 stroops at 1000 bps yields 1");
    }

    #[test]
    fn test_apply_percentage_fee_truncation_100_stroops() {
        // 100 * 1000 / 10_000 = 10.0 → exactly 10
        let result = fee::apply_percentage_fee(100, 1000);
        assert_eq!(result, Some(10), "100 stroops at 1000 bps yields 10");
    }

    #[test]
    fn test_fee_split_truncation_1_stroop() {
        // 1 * 1000 / 10_000 = 0 protocol, 1 creator (remainder to creator)
        // Truncation causes the full amount to go to creator.
        let (creator, protocol) = fee::compute_fee_split(1, 9000, 1000);
        assert_eq!(protocol, 0, "1 stroop: protocol fee truncated to 0");
        assert_eq!(creator, 1, "1 stroop: creator gets full amount");
        assert_eq!(creator + protocol, 1, "conservation holds");
    }

    #[test]
    fn test_fee_split_truncation_10_stroops() {
        // 10 * 1000 / 10_000 = 1 protocol, 9 creator
        let (creator, protocol) = fee::compute_fee_split(10, 9000, 1000);
        assert_eq!(protocol, 1, "10 stroops: protocol fee is 1");
        assert_eq!(creator, 9, "10 stroops: creator gets 9");
        assert_eq!(creator + protocol, 10, "conservation holds");
    }

    #[test]
    fn test_fee_split_truncation_100_stroops() {
        // 100 * 1000 / 10_000 = 10 protocol, 90 creator
        let (creator, protocol) = fee::compute_fee_split(100, 9000, 1000);
        assert_eq!(protocol, 10, "100 stroops: protocol fee is 10");
        assert_eq!(creator, 90, "100 stroops: creator gets 90");
        assert_eq!(creator + protocol, 100, "conservation holds");
    }

    // --- Zero address validation ---

    #[test]
    fn test_validate_non_zero_address_rejects_zero() {
        use soroban_sdk::{Address, Env, String};
        let env = Env::default();
        let zero_str = String::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        );
        let zero_addr = Address::from_string(&zero_str);
        let result = super::validate_non_zero_address(&env, &zero_addr);
        assert_eq!(result, Err(super::ContractError::ZeroAddress));
    }

    #[test]
    fn test_validate_non_zero_address_accepts_valid() {
        use soroban_sdk::{testutils::Address as _, Address, Env};
        let env = Env::default();
        let valid = Address::generate(&env);
        let result = super::validate_non_zero_address(&env, &valid);
        assert_eq!(result, Ok(()));
    }
}

#[cfg(test)]
mod test;
