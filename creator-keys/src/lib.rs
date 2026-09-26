#![no_std]
pub mod quote_view_errors;

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String, Vec};

pub mod events;

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
    NoDividendClaimable = 19,
    ZeroDistributionAmount = 20,
    NoKeyHolders = 21,
    AllocationLocked = 22,
    AlreadyClaimed = 23,
    SupplyCapExceeded = 24,
    InsufficientSupply = 25,
    SelfTransfer = 26,
    ZeroTransferAmount = 27,
    InsufficientTreasuryBalance = 28,
    BatchClaimExceedsLimit = 29,
    InvalidCoCreatorShare = 30,
    WhitelistOnly = 31,
    WhitelistTooLarge = 32,
    AirdropRecipientLimitExceeded = 33,
    InvalidReferrer = 34,
    WalletCapExceeded = 35,
    DiscountTierLimitExceeded = 36,
    // --- Staking and stake-receipt NFT ---
    InvalidLockPeriod = 37,
    StakePositionNotFound = 38,
    StakeStillLocked = 39,
    StakeNftNotFound = 40,
    StakeNftNotOwned = 41,
    InvalidTokenAmount = 42,
    SelfStakeNftTransfer = 43,
    SpenderNotAuthorized = 44,
    InsufficientAllowance = 45,
    // --- Vault rebalancing ---
    VaultWeightsNotSet = 46,
    InvalidTargetWeights = 47,
    TargetWeightsNotNormalized = 48,
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

    /// Computes the net buyback cost after deducting the protocol fee.
    ///
    /// Returns `None` if the fee computation overflows or the subtraction overflows.
    /// The result is `gross_price - protocol_fee` where `protocol_fee` is calculated
    /// via `apply_percentage_fee`. This mirrors the fee logic used for regular buys.
    pub fn compute_net_buyback_cost(gross_price: i128, protocol_fee_bps: u32) -> Option<i128> {
        let protocol_fee = apply_percentage_fee(gross_price, protocol_fee_bps)?;
        gross_price.checked_sub(protocol_fee)
    }
    ///
    /// Returns `None` if the fee computation or addition overflows. This helper
    /// exists so the buyback path shares the same bps math used in regular buys
    /// instead of reimplementing the protocol fee arithmetic inline.
    pub fn compute_buyback_cost(gross_price: i128, protocol_fee_bps: u32) -> Option<i128> {
        let protocol_fee = apply_percentage_fee(gross_price, protocol_fee_bps)?;
        gross_price.checked_add(protocol_fee)
    }

    /// Computes the net buyback cost after deducting the protocol fee.
    ///
    /// Takes the gross buyback price and subtracts the protocol fee portion,
    /// returning the net amount that remains after fee deduction. Uses the same
    /// `apply_percentage_fee` helper as the regular buy and buyback fee paths
    /// so the bps arithmetic stays consistent across the contract.
    ///
    /// Returns `None` if the fee computation or subtraction would underflow.
    ///
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

    /// Splits `total` into `(remainder, shared_amount)` by basis points.
    ///
    /// Remainder from integer division stays with the primary recipient so the
    /// two outputs always sum to `total`.
    pub fn checked_split_bps_amount(total: i128, share_bps: u32) -> Option<(i128, i128)> {
        if total <= 0 {
            return Some((0, 0));
        }
        let shared_amount = apply_percentage_fee(total, share_bps)?;
        let remainder = checked_sub_i128(total, shared_amount)?;
        Some((remainder, shared_amount))
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

    /// Safely accumulates a value into an accumulator, returning an error on overflow.
    ///
    /// This helper is used in quote accumulator paths (e.g., dividend distribution) where
    /// adding a per-key-net amount to the current accumulator must not overflow.
    /// Unlike `checked_fee_sum` which returns `Option<T>`, this returns a `ContractError`
    /// for use at call sites that need structured error handling.
    ///
    /// # Motivation
    ///
    /// Accumulator updates happen during dividend distribution and similar paths.
    /// The pattern `accumulator.checked_add(delta).ok_or(ContractError::Overflow)?`
    /// appears repeatedly. This helper centralizes the pattern and makes overflow
    /// handling explicit.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let new_accum = fee::checked_accumulate(current_accumulator, per_key_net)?;
    /// env.storage().persistent().set(&acc_key, &new_accum);
    /// ```
    pub fn checked_accumulate(current: i128, delta: i128) -> Result<i128, ContractError> {
        current.checked_add(delta).ok_or(ContractError::Overflow)
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
        pub const CURVE_SLOPE: DataKey = DataKey::CurveSlope;
        pub const TREASURY_BALANCE: DataKey = DataKey::TreasuryBalance;

        pub fn curve_preset(creator: &Address) -> DataKey {
            DataKey::CurvePreset(creator.clone())
        }

        pub fn creator_fee_balance(creator: &Address) -> DataKey {
            DataKey::CreatorFeeBalance(creator.clone())
        }

        pub fn co_creator(creator: &Address) -> DataKey {
            DataKey::CoCreator(creator.clone())
        }

        pub fn co_creator_fee_balance(creator: &Address, co_creator: &Address) -> DataKey {
            DataKey::CoCreatorFeeBalance(creator.clone(), co_creator.clone())
        }

        pub fn whitelist(creator: &Address) -> DataKey {
            DataKey::Whitelist(creator.clone())
        }

        pub fn creator(creator: &Address) -> DataKey {
            creator_key(creator)
        }

        pub fn holder_balance_key(creator_id: &Address, holder: &Address) -> DataKey {
            key_balance_key(creator_id, holder)
        }

        pub fn dividend_accumulator(creator: &Address) -> DataKey {
            DataKey::DividendPerKeyAccumulated(creator.clone())
        }

        pub fn holder_dividend_checkpoint(creator: &Address, holder: &Address) -> DataKey {
            DataKey::HolderDividendCheckpoint(creator.clone(), holder.clone())
        }

        pub fn holder_dividend_pending(creator: &Address, holder: &Address) -> DataKey {
            DataKey::HolderDividendPending(creator.clone(), holder.clone())
        }

        pub fn locked_allocation(creator: &Address) -> DataKey {
            DataKey::LockedAllocation(creator.clone())
        }

        pub fn max_supply(creator: &Address) -> DataKey {
            DataKey::MaxSupply(creator.clone())
        }

        pub fn max_keys_per_wallet(creator: &Address) -> DataKey {
            DataKey::MaxKeysPerWallet(creator.clone())
        }

        pub fn referral_fee_bps() -> DataKey {
            DataKey::ReferralFeeBps
        }

        pub fn discount_tiers() -> DataKey {
            DataKey::DiscountTiers
        }

        pub fn creator_volume(creator: &Address) -> DataKey {
            DataKey::CreatorVolume(creator.clone())
        }

        pub fn stake_position(creator: &Address, owner: &Address, stake_id: u32) -> DataKey {
            DataKey::StakePosition(creator.clone(), owner.clone(), stake_id)
        }

        pub fn next_stake_id(creator: &Address, owner: &Address) -> DataKey {
            DataKey::NextStakeId(creator.clone(), owner.clone())
        }

        pub fn staked_keys(creator: &Address, owner: &Address) -> DataKey {
            DataKey::StakedKeys(creator.clone(), owner.clone())
        }

        pub fn stake_nft(token_id: u64) -> DataKey {
            DataKey::StakeNft(token_id)
        }

        pub fn stake_nft_id(creator: &Address, stake_id: u32, owner: &Address) -> DataKey {
            DataKey::StakeNftId(creator.clone(), stake_id, owner.clone())
        }

        pub const NEXT_STAKE_NFT_ID: DataKey = DataKey::NextStakeNftId;
        pub const STAKE_NFT_TOTAL_SUPPLY: DataKey = DataKey::StakeNftTotalSupply;

        pub fn stake_nft_holder_count(owner: &Address) -> DataKey {
            DataKey::StakeNftHolderCount(owner.clone())
        }

        pub fn stake_nft_allowance(owner: &Address, spender: &Address) -> DataKey {
            DataKey::StakeNftAllowance(owner.clone(), spender.clone())
        }

        pub fn stake_nft_burned(burner: &Address, token_id: u64) -> DataKey {
            DataKey::StakeNftBurned(burner.clone(), token_id)
        }

        pub fn vault_target_weights(creator: &Address) -> DataKey {
            DataKey::VaultTargetWeights(creator.clone())
        }

        pub fn vault_allocations(creator: &Address) -> DataKey {
            DataKey::VaultAllocations(creator.clone())
        }

        pub fn vault_key_price(creator: &Address, key: &Address) -> DataKey {
            DataKey::VaultKeyPrice(creator.clone(), key.clone())
        }

        pub fn vault_tolerance_bps(creator: &Address) -> DataKey {
            DataKey::VaultToleranceBps(creator.clone())
        }

        pub const FEE_TIERS: DataKey = DataKey::FeeTiers;
        pub const FEE_VOLUME_BUCKETS: DataKey = DataKey::FeeVolumeBuckets;
        pub const ACTIVE_FEE_TIER_INDEX: DataKey = DataKey::ActiveFeeTierIndex;

        pub fn creator_curve_slope(creator: &Address) -> DataKey {
            DataKey::CreatorCurveSlope(creator.clone())
        }

        pub fn curve_reset_count(creator: &Address) -> DataKey {
            DataKey::CurveResetCount(creator.clone())
        }

        pub const GOVERNANCE_ADDRESS: DataKey = DataKey::GovernanceAddress;
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
        pub const CO_CREATOR: &str = "get_co_creator";
        pub const CO_CREATOR_FEE_BALANCE: &str = "get_co_creator_fee_balance";
        pub const HOLDER_KEY_COUNT: &str = "get_holder_key_count";
        pub const PROFILE: &str = "get_creator";
        pub const SUPPLY: &str = "get_creator_supply";
        pub const TREASURY_SHARE: &str = "get_creator_treasury_share";
        pub const NAME: &str = "get_key_name";
        pub const SYMBOL: &str = "get_key_symbol";
    }

    /// Default values for fee bounds used across validation paths and test fixtures.
    ///
    /// These constants represent the canonical starting point for a fee configuration.
    /// Keeping them here ensures a single source of truth: any adjustment to the
    /// default split only needs to happen in one place.
    pub mod fee_bounds {
        /// Default creator share in basis points (90%).
        pub const DEFAULT_CREATOR_BPS: u32 = 9_000;

        /// Default protocol share in basis points (10%).
        pub const DEFAULT_PROTOCOL_BPS: u32 = 1_000;
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

/// TTL extension for creator storage entries on each trade.
///
/// This value is added to the TTL of all creator-related storage keys
/// (creator config, supply, holder map, fee config) after every successful
/// buy or sell operation to prevent active creator state from expiring.
pub const CREATOR_TTL_LEDGERS: u32 = 6311520; // ~2 years at 5s per ledger
pub const HANDLE_LEN_MIN: u32 = 3;
pub const HANDLE_LEN_MAX: u32 = 32;
pub const MAX_WHITELIST_SIZE: u32 = 500;

/// Maximum number of recipient entries accepted by a single
/// [`CreatorKeysContract::airdrop_keys`] call.
///
/// Larger lists revert with [`ContractError::AirdropRecipientLimitExceeded`]
/// so a single airdrop cannot grow unbounded in storage writes.
pub const MAX_AIRDROP_RECIPIENTS: u32 = 50;

/// Default referral fee basis points (20% of protocol fee).
pub const DEFAULT_REFERRAL_FEE_BPS: u32 = 2000;

/// Maximum number of discount tiers allowed.
pub const MAX_DISCOUNT_TIERS: u32 = 5;

// --- Staking and stake receipt NFT ---

/// Token name reported by the SEP-41 `name` view for stake receipt NFTs.
pub const STAKE_NFT_NAME: &str = "Staked Creator Key Receipt";

/// Token symbol reported by the SEP-41 `symbol` view for stake receipt NFTs.
pub const STAKE_NFT_SYMBOL: &str = "stkKEY";

/// Stake receipt NFTs are indivisible, so the SEP-41 `decimals` view returns `0`.
pub const STAKE_NFT_DECIMALS: u32 = 0;

/// TTL extension for stake position and stake receipt storage on every mutation.
pub const STAKE_TTL_LEDGERS: u32 = CREATOR_TTL_LEDGERS;

// --- Dynamic fee tiers ---

/// Ledgers in the rolling volume window. 24h at ~5s per ledger.
pub const ROLLING_WINDOW_LEDGERS: u32 = 17_280;

/// Width of one volume bucket. 360 ledgers is ~30 minutes, so the 24h window
/// spans at most `ROLLING_WINDOW_LEDGERS / VOLUME_BUCKET_LEDGERS` (48) buckets.
pub const VOLUME_BUCKET_LEDGERS: u32 = 360;

/// Maximum number of dynamic fee tiers accepted by `set_fee_tiers`.
pub const MAX_FEE_TIERS: u32 = 5;

// --- Vault rebalancing ---

/// Maximum number of pool keys tracked by one creator's staking vault.
pub const MAX_VAULT_KEYS: u32 = 20;

/// Default rebalance drift tolerance in basis points (0.5% of vault value).
pub const DEFAULT_VAULT_TOLERANCE_BPS: u32 = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum CurvePreset {
    Linear = 0,
    Quadratic = 1,
    Flat = 2,
}

/// Canonical storage key schema for persistent protocol state.
///
/// For quote-related key usage and invariants, see
/// [`docs/quote-storage-keys.md`](../../docs/quote-storage-keys.md).
#[derive(Clone, Debug, PartialEq)]
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
    DividendPerKeyAccumulated(Address),
    HolderDividendCheckpoint(Address, Address),
    HolderDividendPending(Address, Address),
    LockedAllocation(Address),
    MaxSupply(Address),
    CurveSlope,
    CurvePreset(Address),
    TreasuryBalance,
    CoCreator(Address),
    CoCreatorFeeBalance(Address, Address),
    Whitelist(Address),
    MaxKeysPerWallet(Address),
    ReferralFeeBps,
    DiscountTiers,
    CreatorVolume(Address),
    StakePosition(Address, Address, u32),
    NextStakeId(Address, Address),
    StakedKeys(Address, Address),
    StakeNft(u64),
    StakeNftId(Address, u32, Address),
    NextStakeNftId,
    StakeNftTotalSupply,
    StakeNftHolderCount(Address),
    StakeNftAllowance(Address, Address),
    StakeNftBurned(Address, u64),
    VaultTargetWeights(Address),
    VaultAllocations(Address),
    VaultKeyPrice(Address, Address),
    VaultToleranceBps(Address),
    FeeTiers,
    FeeVolumeBuckets,
    ActiveFeeTierIndex,
    CreatorCurveSlope(Address),
    CurveResetCount(Address),
    GovernanceAddress,
}

/// Time-locked key allocation for creator self-vesting.
///
/// When a creator registers, they may optionally lock a portion of keys
/// that cannot be claimed until a specified ledger height is reached.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct LockedAllocation {
    pub amount: u32,
    pub unlock_ledger: u32,
    pub claimed: bool,
}

/// A single active staking position held by a wallet for one creator.
///
/// Positions are keyed by `(creator, owner, stake_id)` in
/// [`constants::storage::stake_position`], so `stake_id` only has to be unique for
/// one owner staking into one creator. `owner` is the *current* receipt owner, which
/// changes when the backing stake receipt NFT is transferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakePosition {
    pub stake_id: u32,
    pub amount: u32,
    pub unlock_ledger: u32,
}

/// Optional immutable collaborator split configured at creator registration.
///
/// `share_bps` is the co-creator's share of the creator fee, not of the full
/// trade price. It must be in the inclusive range `1..=9999`.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CoCreatorConfig {
    pub address: Address,
    pub share_bps: u32,
}

/// Required creator identity fields for registration.
///
/// Grouping these fields keeps the public contract entrypoint under Clippy's
/// argument-count threshold without changing validation or storage behavior for
/// any registration option.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RegisterCreatorParams {
    pub creator: Address,
    pub handle: String,
}

/// Single discount tier definition.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DiscountTier {
    /// Volume threshold in stroops; creator must reach or exceed this cumulative volume.
    pub threshold: i128,
    /// Protocol fee basis points applied when threshold is met.
    pub protocol_bps: u32,
}

/// Optional whitelist window configured at creator registration.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct WhitelistConfig {
    pub addresses: Vec<Address>,
    pub window_ledgers: u32,
}

/// Read-only status for a creator's whitelist window.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct WhitelistStatus {
    pub active: bool,
    pub expires_at_ledger: u32,
    pub remaining_ledgers: u32,
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
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ClaimResult {
    pub creator: Address,
    pub amount_claimed: i128,
}

/// One recipient of a creator key airdrop: the wallet to credit and how many
/// keys it receives.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AirdropEntry {
    pub address: Address,
    pub amount: u32,
}

/// Result of a successful [`CreatorKeysContract::airdrop_keys`] call.
///
/// `total_cost` is the full amount charged to the creator: the bonding curve
/// cost for every minted key plus the protocol fee on that cost.
/// `skipped_count` is the number of recipients skipped due to per-wallet cap.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AirdropSummary {
    pub total_keys: u32,
    pub total_cost: i128,
    pub recipient_count: u32,
    pub skipped_count: u32,
}

fn validate_whitelist_config(config: &WhitelistConfig) -> Result<(), ContractError> {
    if config.addresses.len() > MAX_WHITELIST_SIZE {
        return Err(ContractError::WhitelistTooLarge);
    }
    Ok(())
}

fn read_whitelist_config(env: &Env, creator: &Address) -> Option<WhitelistConfig> {
    env.storage()
        .persistent()
        .get::<DataKey, WhitelistConfig>(&constants::storage::whitelist(creator))
}

fn whitelist_status(env: &Env, profile: &CreatorProfile) -> WhitelistStatus {
    let Some(config) = read_whitelist_config(env, &profile.creator) else {
        return WhitelistStatus {
            active: false,
            expires_at_ledger: 0,
            remaining_ledgers: 0,
        };
    };
    let expires_at_ledger = profile.registered_at.saturating_add(config.window_ledgers);
    let current_ledger = env.ledger().sequence();
    let remaining_ledgers = expires_at_ledger.saturating_sub(current_ledger);
    WhitelistStatus {
        active: remaining_ledgers > 0,
        expires_at_ledger,
        remaining_ledgers,
    }
}

fn assert_whitelist_allows_buy(
    env: &Env,
    profile: &CreatorProfile,
    buyer: &Address,
) -> Result<(), ContractError> {
    let status = whitelist_status(env, profile);
    if !status.active {
        return Ok(());
    }
    let Some(config) = read_whitelist_config(env, &profile.creator) else {
        return Ok(());
    };
    for address in config.addresses.iter() {
        if address == *buyer {
            return Ok(());
        }
    }
    Err(ContractError::WhitelistOnly)
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
    read_creator_supply(env, creator)
}

/// Reads a creator's current key supply from persistent storage.
///
/// Returns `0` if the creator has not been registered (no supply record exists).
/// Centralises the supply read so the storage key format is defined once.
pub fn read_creator_supply(env: &Env, creator_id: &Address) -> u32 {
    read_creator_profile(env, creator_id)
        .map(|p| p.supply)
        .unwrap_or(0)
}

/// Writes an updated key supply back to persistent storage for a creator.
///
/// Reads the existing creator profile from storage, updates the `supply` field,
/// and persists the result under the standard creator storage key. This
/// centralises the write path so that buy, sell, and buyback all share the
/// same key-construction logic instead of building it inline.
///
/// # Panics
///
/// Panics if the creator profile does not exist in storage. Callers must
/// verify creator registration (e.g. via [`read_registered_creator_profile`])
/// before invoking this helper.
pub fn write_creator_supply(env: &Env, creator_id: &Address, supply: u32) {
    let key = constants::storage::creator(creator_id);
    let mut profile: CreatorProfile = env
        .storage()
        .persistent()
        .get(&key)
        .expect("write_creator_supply: creator profile not found");
    profile.supply = supply;
    env.storage().persistent().set(&key, &profile);
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

fn read_co_creator_config(env: &Env, creator: &Address) -> Option<CoCreatorConfig> {
    let key = constants::storage::co_creator(creator);
    env.storage()
        .persistent()
        .get::<DataKey, CoCreatorConfig>(&key)
}

fn validate_co_creator_config(env: &Env, config: &CoCreatorConfig) -> Result<(), ContractError> {
    validate_non_zero_address(env, &config.address)?;
    if !(1..fee::BPS_MAX).contains(&config.share_bps) {
        return Err(ContractError::InvalidCoCreatorShare);
    }
    Ok(())
}

/// Reads accrued fee balance for a creator's configured co-creator.
pub fn read_co_creator_fee_balance(env: &Env, creator: &Address, co_creator: &Address) -> i128 {
    let key = constants::storage::co_creator_fee_balance(creator, co_creator);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Reads the aggregate number of keys a holder has staked for a creator.
///
/// Returns `0` when the holder has no staking positions. The value is a stored
/// aggregate rather than a sum over positions because Soroban persistent storage
/// cannot be range-iterated.
pub fn read_staked_keys(env: &Env, creator: &Address, holder: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&constants::storage::staked_keys(creator, holder))
        .unwrap_or(0)
}

/// Writes the aggregate staked-key count for a `(creator, holder)` pair.
pub fn write_staked_keys(env: &Env, creator: &Address, holder: &Address, amount: u32) {
    env.storage()
        .persistent()
        .set(&constants::storage::staked_keys(creator, holder), &amount);
}

/// Reads the total number of keys a holder is entitled to for a creator.
///
/// Entitlement is the liquid [`DataKey::KeyBalance`] plus the [`DataKey::StakedKeys`]
/// aggregate. Staked keys remain in `profile.supply` and therefore keep earning
/// creator dividends, so all dividend math must use this value rather than the
/// liquid balance alone.
pub fn read_entitled_keys(env: &Env, creator: &Address, holder: &Address) -> u32 {
    let liquid: u32 = env
        .storage()
        .persistent()
        .get(&constants::storage::holder_balance_key(creator, holder))
        .unwrap_or(0);
    liquid.saturating_add(read_staked_keys(env, creator, holder))
}

fn credit_co_creator_fee_balance(
    env: &Env,
    creator: &Address,
    co_creator: &Address,
    amount: i128,
) -> Result<(), ContractError> {
    if amount <= 0 {
        return Ok(());
    }
    let key = constants::storage::co_creator_fee_balance(creator, co_creator);
    let current = read_co_creator_fee_balance(env, creator, co_creator);
    let updated = current.checked_add(amount).ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&key, &updated);
    Ok(())
}

fn credit_creator_fee(env: &Env, creator: &Address, amount: i128) -> Result<(), ContractError> {
    if amount <= 0 {
        return Ok(());
    }

    let Some(config) = read_co_creator_config(env, creator) else {
        return credit_creator_fee_recipient_balance(env, creator, amount);
    };

    let co_creator = config.address;
    let (creator_recipient_amount, co_creator_amount) =
        fee::checked_split_bps_amount(amount, config.share_bps).ok_or(ContractError::Overflow)?;
    credit_creator_fee_recipient_balance(env, creator, creator_recipient_amount)?;
    credit_co_creator_fee_balance(env, creator, &co_creator, co_creator_amount)?;

    if co_creator_amount > 0 {
        env.events().publish(
            events::co_creator_fee_earned_topics(creator, &co_creator),
            events::CoCreatorFeeEarned {
                creator_id: creator.clone(),
                co_creator,
                amount: co_creator_amount,
                ledger: env.ledger().sequence(),
            },
        );
    }

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

pub(crate) fn assert_not_paused(env: &Env) -> Result<(), ContractError> {
    if is_paused(env) {
        return Err(ContractError::ProtocolPaused);
    }
    Ok(())
}

pub(crate) fn assert_is_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
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

/// Reads the governance address, returning `None` when governance is unset.
pub fn read_governance_address(env: &Env) -> Option<Address> {
    env.storage()
        .persistent()
        .get::<DataKey, Address>(&constants::storage::GOVERNANCE_ADDRESS)
}

/// Authorises a caller that is either the protocol admin or the governance address.
///
/// Governance is a distinct principal from the admin so the two can be held by
/// different entities (an operations multisig and a governance executor, say).
/// When no governance address is configured only the admin passes. This is the
/// shared gate for the entrypoints the spec scopes to "admin or governance".
pub(crate) fn assert_is_admin_or_governance(
    env: &Env,
    caller: &Address,
) -> Result<(), ContractError> {
    if let Some(governance) = read_governance_address(env) {
        if *caller == governance {
            return Ok(());
        }
    }
    assert_is_admin(env, caller)
}

/// Reads the flat, stored fee configuration without any dynamic-tier adjustment.
///
/// Configuration views report what an admin set. Use
/// [`CreatorKeysContract::get_dynamic_fee_view`] or
/// [`CreatorKeysContract::get_current_fee`] for the fee a trade will actually pay.
fn read_effective_fee_config(env: &Env) -> Option<fee::FeeConfig> {
    effective_fee_config(env)
}

fn read_required_effective_fee_config(env: &Env) -> Result<fee::FeeConfig, ContractError> {
    read_effective_fee_config(env).ok_or(ContractError::FeeConfigNotSet)
}

/// Resolves the curve slope that applies to `creator`.
///
/// A creator that has been through [`CreatorKeysContract::reset_curve`] carries a
/// per-creator slope override, which takes precedence over the global slope. Every
/// other creator inherits the global slope, so resetting one creator cannot move
/// another creator's pricing.
fn resolve_curve_slope(env: &Env, creator: &Address) -> i128 {
    read_creator_slope(env, creator).unwrap_or_else(|| read_curve_slope(env))
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

/// Reads the accumulated treasury balance, returning `0` when none is stored.
pub fn read_treasury_balance(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&constants::storage::TREASURY_BALANCE)
        .unwrap_or(0)
}

/// Credits `amount` to the protocol treasury balance.
fn credit_treasury_balance(env: &Env, amount: i128) -> Result<(), ContractError> {
    if amount <= 0 {
        return Ok(());
    }
    let updated = read_treasury_balance(env)
        .checked_add(amount)
        .ok_or(ContractError::Overflow)?;
    env.storage()
        .persistent()
        .set(&constants::storage::TREASURY_BALANCE, &updated);
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

fn assert_buyback_total_cost_slippage(
    total_cost: i128,
    max_total_cost: Option<i128>,
) -> Result<(), ContractError> {
    if let Some(max) = max_total_cost {
        if total_cost > max {
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
    price: i128,
    min_proceeds: Option<i128>,
) -> Result<(), ContractError> {
    if let Some(min) = min_proceeds {
        let proceeds = compute_sell_proceeds(env, price)?;
        if proceeds < min {
            return Err(ContractError::SlippageExceeded);
        }
    }
    Ok(())
}

fn accrue_sell_trade_fees(env: &Env, creator: &Address, price: i128) -> Result<(), ContractError> {
    if read_effective_fee_config(env).is_none() {
        return Ok(());
    }

    let (creator_fee, protocol_fee) =
        CreatorKeysContract::compute_fees_for_payment(env.clone(), price)?;
    credit_creator_fee(env, creator, creator_fee)?;
    credit_treasury_balance(env, protocol_fee)?;

    if env
        .storage()
        .persistent()
        .get::<DataKey, Address>(&constants::storage::PROTOCOL_FEE_RECIPIENT)
        .is_some()
    {
        credit_protocol_fee_recipient_balance(env, protocol_fee)?;
    }

    Ok(())
}

/// Resolves and validates the shared inputs required by read-only quote methods.
///
/// Reads the key price and creator profile from storage, returning the
/// bonding-curve-adjusted price. Returns the appropriate [`ContractError`] on
/// failure. When the adjusted price is zero, returns `Ok(None)`.
fn resolve_quote_inputs(env: &Env, creator: &Address) -> Result<Option<i128>, ContractError> {
    let base_price: i128 = env
        .storage()
        .persistent()
        .get(&constants::storage::KEY_PRICE)
        .ok_or(ContractError::KeyPriceNotSet)?;

    let Some(normalized) = normalize_quote_amount(base_price)? else {
        return Ok(None);
    };

    let profile = read_registered_creator_profile(env, creator)?;
    let curve_price = compute_bonding_curve_price(env, creator, normalized, profile.supply)?;
    normalize_quote_amount(curve_price)
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

fn validate_buyback_amount(amount: u32) -> Result<(), ContractError> {
    if amount == 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    Ok(())
}

fn compute_buyback_base_price(unit_price: i128, amount: u32) -> Result<i128, ContractError> {
    unit_price
        .checked_mul(i128::from(amount))
        .ok_or(ContractError::Overflow)
}

fn read_curve_slope(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&constants::storage::CURVE_SLOPE)
        .unwrap_or(0)
}

fn compute_bonding_curve_price(
    env: &Env,
    creator: &Address,
    base_price: i128,
    supply: u32,
) -> Result<i128, ContractError> {
    let preset = env
        .storage()
        .persistent()
        .get(&constants::storage::curve_preset(creator))
        .unwrap_or(CurvePreset::Linear);

    match preset {
        CurvePreset::Flat => Ok(base_price),
        CurvePreset::Linear => {
            let slope = resolve_curve_slope(env, creator);
            let supply_component = slope
                .checked_mul(i128::from(supply))
                .ok_or(ContractError::Overflow)?;
            base_price
                .checked_add(supply_component)
                .ok_or(ContractError::Overflow)
        }
        CurvePreset::Quadratic => {
            let slope = resolve_curve_slope(env, creator);
            let supply_sq = (supply as i128)
                .checked_mul(supply as i128)
                .ok_or(ContractError::Overflow)?;
            let supply_component = slope
                .checked_mul(supply_sq)
                .ok_or(ContractError::Overflow)?;
            base_price
                .checked_add(supply_component)
                .ok_or(ContractError::Overflow)
        }
    }
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

fn read_dividend_accumulator(env: &Env, creator: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&constants::storage::dividend_accumulator(creator))
        .unwrap_or(0)
}

/// Settles pending dividends for a holder before their balance changes.
///
/// On a holder's first settlement the checkpoint is initialised to the current
/// accumulator so they earn nothing retroactively. Earned and pending amounts
/// use checked arithmetic to avoid overflow.
pub(crate) fn settle_holder_dividends(
    env: &Env,
    creator: &Address,
    holder: &Address,
) -> Result<(), ContractError> {
    let current_entitled = read_entitled_keys(env, creator, holder);
    let accumulator = read_dividend_accumulator(env, creator);
    let checkpoint_key = constants::storage::holder_dividend_checkpoint(creator, holder);
    // Default to current accumulator on first settlement so no retroactive earnings.
    let checkpoint: i128 = env
        .storage()
        .persistent()
        .get(&checkpoint_key)
        .unwrap_or(accumulator);

    let pending_key = constants::storage::holder_dividend_pending(creator, holder);
    let pending: i128 = env.storage().persistent().get(&pending_key).unwrap_or(0);

    let diff = accumulator
        .checked_sub(checkpoint)
        .ok_or(ContractError::Overflow)?;
    let earned = (current_entitled as i128)
        .checked_mul(diff)
        .ok_or(ContractError::Overflow)?;
    let new_pending = pending.checked_add(earned).ok_or(ContractError::Overflow)?;

    env.storage().persistent().set(&pending_key, &new_pending);
    env.storage()
        .persistent()
        .set(&checkpoint_key, &accumulator);
    Ok(())
}

fn compute_claimable_dividend(env: &Env, creator: &Address, holder: &Address) -> i128 {
    let accumulator = read_dividend_accumulator(env, creator);
    let checkpoint_key = constants::storage::holder_dividend_checkpoint(creator, holder);
    let checkpoint: i128 = env
        .storage()
        .persistent()
        .get(&checkpoint_key)
        .unwrap_or(accumulator);
    let pending_key = constants::storage::holder_dividend_pending(creator, holder);
    let pending: i128 = env.storage().persistent().get(&pending_key).unwrap_or(0);

    let entitled = read_entitled_keys(env, creator, holder);

    let diff = accumulator.saturating_sub(checkpoint);
    let earned = (entitled as i128).saturating_mul(diff);
    pending.saturating_add(earned)
}

pub(crate) fn extend_creator_ttl(env: &Env, creator: &Address) {
    let current_ledger = env.ledger().sequence();
    let extend_to = current_ledger + CREATOR_TTL_LEDGERS;
    let threshold = current_ledger;

    let creator_key = constants::storage::creator(creator);
    env.storage()
        .persistent()
        .extend_ttl(&creator_key, threshold, extend_to);

    let fee_balance_key = constants::storage::creator_fee_balance(creator);
    if env.storage().persistent().has(&fee_balance_key) {
        env.storage()
            .persistent()
            .extend_ttl(&fee_balance_key, threshold, extend_to);
    }

    let dividend_key = constants::storage::dividend_accumulator(creator);
    if env.storage().persistent().has(&dividend_key) {
        env.storage()
            .persistent()
            .extend_ttl(&dividend_key, threshold, extend_to);
    }

    let locked_key = constants::storage::locked_allocation(creator);
    if env.storage().persistent().has(&locked_key) {
        env.storage()
            .persistent()
            .extend_ttl(&locked_key, threshold, extend_to);
    }

    let max_supply_key = constants::storage::max_supply(creator);
    if env.storage().persistent().has(&max_supply_key) {
        env.storage()
            .persistent()
            .extend_ttl(&max_supply_key, threshold, extend_to);
    }

    let curve_preset_key = constants::storage::curve_preset(creator);
    if env.storage().persistent().has(&curve_preset_key) {
        env.storage()
            .persistent()
            .extend_ttl(&curve_preset_key, threshold, extend_to);
    }

    let co_creator_key = constants::storage::co_creator(creator);
    if env.storage().persistent().has(&co_creator_key) {
        env.storage()
            .persistent()
            .extend_ttl(&co_creator_key, threshold, extend_to);

        if let Some(config) = read_co_creator_config(env, creator) {
            let co_creator_balance_key =
                constants::storage::co_creator_fee_balance(creator, &config.address);
            if env.storage().persistent().has(&co_creator_balance_key) {
                env.storage().persistent().extend_ttl(
                    &co_creator_balance_key,
                    threshold,
                    extend_to,
                );
            }
        }
    }
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
    /// - `locked_allocation`: optional time-locked key allocation for creator self-vesting.
    ///   If provided, `unlock_ledger` must be strictly greater than current ledger.
    /// - `max_supply`: optional maximum supply cap. If provided, must be greater than zero.
    /// - `max_keys_per_wallet`: optional maximum keys per wallet cap. If provided, must be greater than zero.
    /// - `co_creator`: optional immutable collaborator split. If provided, `share_bps`
    ///   must be in the inclusive range `1..=9999`.
    #[allow(clippy::too_many_arguments)]
    pub fn register_creator(
        env: Env,
        params: RegisterCreatorParams,
        locked_allocation: Option<LockedAllocation>,
        max_supply: Option<u32>,
        max_keys_per_wallet: Option<u32>,
        curve_preset: Option<CurvePreset>,
        co_creator: Option<CoCreatorConfig>,
        whitelist: Option<WhitelistConfig>,
    ) -> Result<(), ContractError> {
        let RegisterCreatorParams { creator, handle } = params;

        creator.require_auth();
        assert_not_paused(&env)?;

        validate_creator_handle(&handle)?;
        if let Some(config) = co_creator.as_ref() {
            validate_co_creator_config(&env, config)?;
        }
        if let Some(config) = whitelist.as_ref() {
            validate_whitelist_config(config)?;
        }

        let key = constants::storage::creator(&creator);
        // Creator profile storage is a single source of truth keyed by creator address.
        // Once written, this key's existence is the registration invariant.
        if env.storage().persistent().has(&key) {
            return Err(ContractError::AlreadyRegistered);
        }

        let current_ledger = env.ledger().sequence();
        let mut supply = 0u32;

        // Handle locked allocation
        if let Some(alloc) = locked_allocation {
            if alloc.unlock_ledger <= current_ledger {
                return Err(ContractError::AllocationLocked);
            }
            if alloc.amount == 0 {
                return Err(ContractError::NotPositiveAmount);
            }
            supply = supply
                .checked_add(alloc.amount)
                .ok_or(ContractError::Overflow)?;

            let locked = LockedAllocation {
                amount: alloc.amount,
                unlock_ledger: alloc.unlock_ledger,
                claimed: false,
            };
            env.storage()
                .persistent()
                .set(&constants::storage::locked_allocation(&creator), &locked);
            env.events().publish(
                (events::ALLOCATION_LOCKED_EVENT_NAME, creator.clone()),
                events::AllocationLockedEvent {
                    creator_id: creator.clone(),
                    amount: alloc.amount,
                    unlock_ledger: alloc.unlock_ledger,
                },
            );
        }

        // Handle max supply cap
        if let Some(cap) = max_supply {
            if cap == 0 {
                return Err(ContractError::NotPositiveAmount);
            }
            if supply > cap {
                return Err(ContractError::SupplyCapExceeded);
            }
            env.storage()
                .persistent()
                .set(&constants::storage::max_supply(&creator), &cap);
        }

        // Handle max keys per wallet cap
        if let Some(cap) = max_keys_per_wallet {
            if cap == 0 {
                return Err(ContractError::NotPositiveAmount);
            }
            env.storage()
                .persistent()
                .set(&constants::storage::max_keys_per_wallet(&creator), &cap);
        }

        // Handle curve preset
        let preset = curve_preset.unwrap_or(CurvePreset::Linear);
        let preset_key = constants::storage::curve_preset(&creator);
        env.storage().persistent().set(&preset_key, &preset);

        if let Some(config) = co_creator {
            env.storage()
                .persistent()
                .set(&constants::storage::co_creator(&creator), &config);
        }

        if let Some(config) = whitelist {
            env.storage()
                .persistent()
                .set(&constants::storage::whitelist(&creator), &config);
        }

        let profile = CreatorProfile {
            creator: creator.clone(),
            handle,
            supply,
            holder_count: 0,
            fee_recipient: creator.clone(),
            registered_at: current_ledger,
        };

        let fee_config = read_protocol_fee_config(&env).unwrap_or(fee::FeeConfig {
            creator_bps: 0,
            protocol_bps: 0,
        });

        // Persist profile before event publication so indexers reading contract state
        // after this tx observe the same registration payload that was emitted.
        env.storage().persistent().set(&key, &profile);
        // Set initial TTL for creator storage
        let extend_to = current_ledger + CREATOR_TTL_LEDGERS;
        env.storage()
            .persistent()
            .extend_ttl(&key, current_ledger, extend_to);
        env.storage()
            .persistent()
            .extend_ttl(&preset_key, current_ledger, extend_to);
        let co_creator_key = constants::storage::co_creator(&creator);
        if env.storage().persistent().has(&co_creator_key) {
            env.storage()
                .persistent()
                .extend_ttl(&co_creator_key, current_ledger, extend_to);
        }
        let whitelist_key = constants::storage::whitelist(&creator);
        if env.storage().persistent().has(&whitelist_key) {
            env.storage()
                .persistent()
                .extend_ttl(&whitelist_key, current_ledger, extend_to);
        }

        env.events().publish(
            events::register_event_topics(&profile.creator),
            events::CreatorRegisteredEvent {
                creator: profile.creator.clone(),
                handle: profile.handle.clone(),
                supply: profile.supply,
                holder_count: profile.holder_count,
                creator_bps: fee_config.creator_bps,
                protocol_bps: fee_config.protocol_bps,
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
        Self::buy_key_with_referrer(env, creator, buyer, payment, max_price, None)
    }

    pub fn buy_key_with_referrer(
        env: Env,
        creator: Address,
        buyer: Address,
        payment: i128,
        max_price: Option<i128>,
        referrer: Option<Address>,
    ) -> Result<u32, ContractError> {
        buyer.require_auth();
        assert_not_paused(&env)?;

        if payment <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        if let Some(referrer_addr) = referrer.as_ref() {
            if *referrer_addr == buyer {
                return Err(ContractError::InvalidReferrer);
            }
        }

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;

        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;
        assert_whitelist_allows_buy(&env, &profile, &buyer)?;
        let price = compute_bonding_curve_price(&env, &creator, base_price, profile.supply)?;

        assert_buy_price_slippage(price, max_price)?;

        if payment < price {
            return Err(ContractError::InsufficientPayment);
        }

        // Check max supply cap if set
        if let Some(max_supply) = env
            .storage()
            .persistent()
            .get::<DataKey, u32>(&constants::storage::max_supply(&creator))
        {
            if profile.supply >= max_supply {
                return Err(ContractError::SupplyCapExceeded);
            }
        }

        let balance_key = constants::storage::holder_balance_key(&creator, &buyer);
        // Missing balance entries are treated as zero to keep storage sparse.
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        // Check max keys per wallet cap
        if let Some(cap) = env
            .storage()
            .persistent()
            .get::<DataKey, u32>(&constants::storage::max_keys_per_wallet(&creator))
        {
            let post_buy_balance = current_balance
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
            if post_buy_balance > cap {
                return Err(ContractError::WalletCapExceeded);
            }
        }

        // Settle dividends before balance changes so earnings are captured at old balance.
        settle_holder_dividends(&env, &creator, &buyer)?;

        if current_balance == 0 {
            profile.holder_count = profile
                .holder_count
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
        }

        // Persist holder_count before write_creator_supply reads the profile.
        let key = constants::storage::creator(&creator);
        env.storage().persistent().set(&key, &profile);

        profile.supply = profile
            .supply
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;

        // Supply and holder_count must always move together with buyer balance writes.
        write_creator_supply(&env, &creator, profile.supply);

        let new_balance = current_balance
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;
        // Balance key is scoped by (creator, holder) so creator positions cannot collide.
        env.storage().persistent().set(&balance_key, &new_balance);

        // Record this trade's gross volume before the fee split is computed, so the
        // trade is priced at the tier its own volume earns. Any later failure reverts
        // the whole transaction, including this write.
        apply_trade_volume(&env, price)?;

        if let Some(config) = read_effective_fee_config(&env) {
            let (creator_fee, protocol_fee) =
                fee::checked_compute_fee_split(price, config.creator_bps, config.protocol_bps)
                    .ok_or(ContractError::Overflow)?;

            credit_creator_fee(&env, &creator, creator_fee)?;

            // Split protocol fee between treasury and referrer only when a referrer is provided
            if let Some(referrer_addr) = referrer {
                let referral_fee_bps = env
                    .storage()
                    .persistent()
                    .get::<DataKey, u32>(&constants::storage::referral_fee_bps())
                    .unwrap_or(DEFAULT_REFERRAL_FEE_BPS);

                let (treasury_amount, referral_amount) =
                    fee::checked_split_bps_amount(protocol_fee, referral_fee_bps)
                        .ok_or(ContractError::Overflow)?;

                credit_treasury_balance(&env, treasury_amount)?;
                credit_protocol_fee_recipient_balance(&env, treasury_amount)?;

                if referral_amount > 0 {
                    // Store referral fee as pending balance for referrer
                    let referrer_balance_key =
                        constants::storage::holder_balance_key(&creator, &referrer_addr);
                    let referrer_current: u32 = env
                        .storage()
                        .persistent()
                        .get(&referrer_balance_key)
                        .unwrap_or(0);
                    let referrer_new = referrer_current
                        .checked_add(referral_amount as u32)
                        .ok_or(ContractError::Overflow)?;
                    env.storage()
                        .persistent()
                        .set(&referrer_balance_key, &referrer_new);

                    env.events().publish(
                        events::referral_fee_earned_topics(&creator, &referrer_addr),
                        events::ReferralFeeEarnedEvent {
                            creator_id: creator.clone(),
                            buyer: buyer.clone(),
                            referrer: referrer_addr,
                            amount: referral_amount,
                            ledger: env.ledger().sequence(),
                        },
                    );
                }
            } else {
                // No referrer: full protocol fee goes to treasury and recipient
                credit_treasury_balance(&env, protocol_fee)?;
                credit_protocol_fee_recipient_balance(&env, protocol_fee)?;
            }
        }

        let buy_event_data = events::KeysBoughtEvent {
            buyer: buyer.clone(),
            creator_id: creator.clone(),
            quantity: 1,
            price_paid: price,
            ledger: env.ledger().sequence(),
        };

        env.events()
            .publish(events::buy_event_topics(&creator, &buyer), buy_event_data);

        // Extend TTL for creator storage after successful buy
        extend_creator_ttl(&env, &creator);

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

        let balance_key = constants::storage::holder_balance_key(&creator, &seller);
        // Missing balance entries are interpreted as zero and rejected consistently.
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if current_balance == 0 {
            return Err(ContractError::InsufficientBalance);
        }

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        let sell_supply = profile
            .supply
            .checked_sub(1)
            .ok_or(ContractError::SellUnderflow)?;
        let price = compute_bonding_curve_price(&env, &creator, base_price, sell_supply)?;

        // Settle dividends before balance changes so earnings are captured at old balance.
        settle_holder_dividends(&env, &creator, &seller)?;

        assert_sell_proceeds_slippage(&env, price, min_proceeds)?;

        // A sell is market activity too, so it feeds the same rolling volume window
        // as a buy. Recorded before the fee split so the tier reflects this trade.
        apply_trade_volume(&env, price)?;

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

        // Persist holder_count before write_creator_supply reads the profile.
        let key = constants::storage::creator(&creator);
        env.storage().persistent().set(&key, &profile);

        // Supply and holder balance are updated together to preserve
        // supply/holder_count invariants for subsequent reads.
        write_creator_supply(&env, &creator, profile.supply);
        env.storage().persistent().set(&balance_key, &new_balance);
        accrue_sell_trade_fees(&env, &creator, price)?;

        env.events().publish(
            (events::SELL_EVENT_NAME, creator.clone(), seller),
            profile.supply,
        );

        // Extend TTL for creator storage after successful sell
        extend_creator_ttl(&env, &creator);

        Ok(profile.supply)
    }

    /// Creator-only buyback that burns keys from the creator's own held balance.
    ///
    /// The creator pays the current gross buyback cost plus protocol fee, while the
    /// creator fee is waived (creator cannot pay themselves a fee). The protocol fee
    /// still applies. To preserve the contract's supply/balance invariants,
    /// the burned amount is decremented from the creator wallet's existing key balance.
    pub fn buyback(
        env: Env,
        creator: Address,
        caller: Address,
        amount: u32,
        payment: i128,
        max_total_cost: Option<i128>,
    ) -> Result<u32, ContractError> {
        caller.require_auth();
        assert_not_paused(&env)?;

        if caller != creator {
            return Err(ContractError::Unauthorized);
        }
        if payment <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        validate_buyback_amount(amount)?;

        let base_price_stored: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;
        let curve_price =
            compute_bonding_curve_price(&env, &creator, base_price_stored, profile.supply)?;
        let base_price = compute_buyback_base_price(curve_price, amount)?;
        let config = read_required_effective_fee_config(&env)?;
        let protocol_fee = fee::apply_percentage_fee(base_price, config.protocol_bps)
            .ok_or(ContractError::Overflow)?;
        let total_cost = fee::compute_buyback_cost(base_price, config.protocol_bps)
            .ok_or(ContractError::Overflow)?;

        assert_buyback_total_cost_slippage(total_cost, max_total_cost)?;
        if payment < total_cost {
            return Err(ContractError::InsufficientPayment);
        }
        if amount > profile.supply {
            return Err(ContractError::InsufficientSupply);
        }

        let balance_key = constants::storage::holder_balance_key(&creator, &caller);
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if current_balance < amount {
            return Err(ContractError::InsufficientBalance);
        }

        let new_balance = current_balance
            .checked_sub(amount)
            .ok_or(ContractError::SellUnderflow)?;
        profile.supply = profile
            .supply
            .checked_sub(amount)
            .ok_or(ContractError::SellUnderflow)?;

        if current_balance > 0 && new_balance == 0 {
            profile.holder_count = profile
                .holder_count
                .checked_sub(1)
                .ok_or(ContractError::SellUnderflow)?;
        }

        let key = constants::storage::creator(&creator);
        env.storage().persistent().set(&key, &profile);
        env.storage().persistent().set(&balance_key, &new_balance);
        credit_protocol_fee_recipient_balance(&env, protocol_fee)?;

        env.events().publish(
            events::buyback_event_topics(&creator),
            events::KeysBoughtBackEvent {
                creator,
                amount,
                price_paid: total_cost,
                new_supply: profile.supply,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(profile.supply)
    }

    /// Creator-only airdrop that mints keys to a list of recipient wallets.
    ///
    /// The creator pays the bonding curve cost for every key across all
    /// recipients plus the protocol fee on that total; the creator fee is
    /// waived (the creator cannot pay themselves a fee). Supply increases by
    /// the total airdropped amount, moving the curve exactly as if the keys
    /// had been bought one by one.
    ///
    /// At most [`MAX_AIRDROP_RECIPIENTS`] recipient entries are accepted per
    /// call. All recipients are credited atomically: if any validation fails,
    /// no keys are minted.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `caller` is not `creator`.
    /// - [`ContractError::AirdropRecipientLimitExceeded`] if `recipients`
    ///   holds more than [`MAX_AIRDROP_RECIPIENTS`] entries.
    /// - [`ContractError::NotPositiveAmount`] if `recipients` is empty, an
    ///   entry's `amount` is zero, or `payment` is not positive.
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    /// - [`ContractError::KeyPriceNotSet`] if no key price is configured.
    /// - [`ContractError::SupplyCapExceeded`] if minting would push supply
    ///   over the creator's max supply cap.
    /// - [`ContractError::InsufficientPayment`] if `payment` does not cover
    ///   the total curve cost plus protocol fee.
    pub fn airdrop_keys(
        env: Env,
        creator: Address,
        caller: Address,
        recipients: Vec<AirdropEntry>,
        payment: i128,
    ) -> Result<AirdropSummary, ContractError> {
        caller.require_auth();
        assert_not_paused(&env)?;

        if caller != creator {
            return Err(ContractError::Unauthorized);
        }
        if recipients.len() > MAX_AIRDROP_RECIPIENTS {
            return Err(ContractError::AirdropRecipientLimitExceeded);
        }
        if recipients.is_empty() || payment <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;
        let max_supply = env
            .storage()
            .persistent()
            .get::<DataKey, u32>(&constants::storage::max_supply(&creator));
        let max_keys_per_wallet = env
            .storage()
            .persistent()
            .get::<DataKey, u32>(&constants::storage::max_keys_per_wallet(&creator));

        // First pass: validate every entry and price the whole airdrop before
        // any storage write, so a failing call cannot leave partial state.
        let mut new_supply = profile.supply;
        let mut total_keys: u32 = 0;
        let mut total_cost: i128 = 0;
        for entry in recipients.iter() {
            if entry.amount == 0 {
                return Err(ContractError::NotPositiveAmount);
            }
            // Check if recipient is already at per-wallet cap
            let balance_key = constants::storage::holder_balance_key(&creator, &entry.address);
            let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
            if let Some(cap) = max_keys_per_wallet {
                if current_balance >= cap {
                    continue; // Skip this recipient - they're already at cap
                }
            }
            for _ in 0..entry.amount {
                if let Some(cap) = max_supply {
                    if new_supply >= cap {
                        return Err(ContractError::SupplyCapExceeded);
                    }
                }
                let price = compute_bonding_curve_price(&env, &creator, base_price, new_supply)?;
                total_cost = total_cost
                    .checked_add(price)
                    .ok_or(ContractError::Overflow)?;
                new_supply = new_supply.checked_add(1).ok_or(ContractError::Overflow)?;
                total_keys = total_keys.checked_add(1).ok_or(ContractError::Overflow)?;
            }
        }

        let mut protocol_fee: i128 = 0;
        if let Some(config) = read_effective_fee_config(&env) {
            protocol_fee = fee::apply_percentage_fee(total_cost, config.protocol_bps)
                .ok_or(ContractError::Overflow)?;
        }
        let required_payment = total_cost
            .checked_add(protocol_fee)
            .ok_or(ContractError::Overflow)?;
        if payment < required_payment {
            return Err(ContractError::InsufficientPayment);
        }

        // Second pass: credit balances. Entries are applied sequentially so a
        // wallet listed twice accumulates both amounts and is counted as a new
        // holder at most once. Skip recipients already at per-wallet cap.
        let mut skipped_count: u32 = 0;
        for entry in recipients.iter() {
            let balance_key = constants::storage::holder_balance_key(&creator, &entry.address);
            let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);

            // Check if recipient is already at per-wallet cap
            if let Some(cap) = max_keys_per_wallet {
                if current_balance >= cap {
                    skipped_count = skipped_count
                        .checked_add(1)
                        .ok_or(ContractError::Overflow)?;
                    continue; // Skip this recipient - they're already at cap
                }
            }

            // Settle dividends before balance changes so earnings are captured at old balance.
            settle_holder_dividends(&env, &creator, &entry.address)?;

            if current_balance == 0 {
                profile.holder_count = profile
                    .holder_count
                    .checked_add(1)
                    .ok_or(ContractError::Overflow)?;
            }
            let new_balance = current_balance
                .checked_add(entry.amount)
                .ok_or(ContractError::Overflow)?;
            env.storage().persistent().set(&balance_key, &new_balance);
        }

        profile.supply = new_supply;
        let key = constants::storage::creator(&creator);
        env.storage().persistent().set(&key, &profile);

        if protocol_fee > 0 {
            credit_protocol_fee_recipient_balance(&env, protocol_fee)?;
            credit_treasury_balance(&env, protocol_fee)?;
        }

        let summary = AirdropSummary {
            total_keys,
            total_cost: required_payment,
            recipient_count: recipients.len().saturating_sub(skipped_count),
            skipped_count,
        };

        env.events().publish(
            events::keys_airdropped_topics(&creator),
            events::KeysAirdroppedEvent {
                creator_id: creator.clone(),
                total_keys: summary.total_keys,
                total_cost: summary.total_cost,
                recipient_count: summary.recipient_count,
                skipped_count: summary.skipped_count,
                ledger: env.ledger().sequence(),
            },
        );

        extend_creator_ttl(&env, &creator);

        Ok(summary)
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
        let key = constants::storage::holder_balance_key(&creator, &wallet);
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
            let key = constants::storage::holder_balance_key(&creator, &holder);
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
            },
            None => CreatorDetailsView {
                creator,
                handle: read_none_string(&env),
                supply: 0,
                is_registered: false,
                registered_at: 0,
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
                },
                None => CreatorDetailsView {
                    creator,
                    handle: read_none_string(&env),
                    supply: 0,
                    is_registered: false,
                    registered_at: 0,
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
    /// Does not mutate the contract state. Returns the creator's handle for
    /// registered creators. Fails with [`ContractError::NotRegistered`] if
    /// the creator is not registered.
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
    /// invalid lookups. Delegates to the shared [`read_creator_supply`] helper.
    pub fn get_total_key_supply(env: Env, creator: Address) -> u32 {
        read_creator_supply(&env, &creator)
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

    /// Read-only view: returns a creator's whitelist window status.
    ///
    /// Returns inactive defaults for unregistered creators or creators without
    /// a configured whitelist. Does not mutate state.
    pub fn get_whitelist_status(env: Env, creator: Address) -> WhitelistStatus {
        let Some(profile) = read_creator_profile(&env, &creator) else {
            return WhitelistStatus {
                active: false,
                expires_at_ledger: 0,
                remaining_ledgers: 0,
            };
        };
        whitelist_status(&env, &profile)
    }

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

    /// Read-only view: returns the optional immutable co-creator config.
    ///
    /// Returns `None` when the creator was registered without a co-creator split.
    pub fn get_co_creator(env: Env, creator: Address) -> Option<CoCreatorConfig> {
        read_co_creator_config(&env, &creator)
    }

    /// Read-only view: returns accrued co-creator fee balance for a creator.
    ///
    /// Fails with [`ContractError::NotRegistered`] if the creator is not registered.
    /// Returns `0` when no co-creator fees have accrued for the address.
    pub fn get_co_creator_fee_balance(
        env: Env,
        creator: Address,
        co_creator: Address,
    ) -> Result<i128, ContractError> {
        read_registered_creator_profile(&env, &creator)?;
        Ok(read_co_creator_fee_balance(&env, &creator, &co_creator))
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

    /// Read-only view: returns the configured protocol treasury share in basis points.
    ///
    /// This value is sourced from the current protocol fee configuration and is
    /// expressed in stable basis-point units.
    pub fn get_protocol_treasury_share_bps(env: Env) -> Result<u32, ContractError> {
        let config = read_required_protocol_fee_config(&env)?;
        Ok(config.protocol_bps)
    }

    /// Read-only view: returns the stored protocol fee basis points value.
    ///
    /// Does not mutate contract state. Fails with
    /// [`ContractError::FeeConfigNotSet`] if no fee configuration has been stored.
    pub fn get_protocol_fee_bps(env: Env) -> Result<u32, ContractError> {
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
        let old_config = read_protocol_fee_config(&env);
        let old_bps = old_config.as_ref().map(|c| c.protocol_bps).unwrap_or(0);

        env.storage()
            .persistent()
            .set(&constants::storage::FEE_CONFIG, &config);

        // Emit global fee config update event
        env.events().publish(
            (events::FEE_CONFIG_UPDATED_EVENT_NAME, admin),
            events::FeeConfigUpdatedEvent {
                old_bps,
                new_bps: protocol_bps,
                updated_at_ledger: env.ledger().sequence(),
            },
        );

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

    /// Sets the bonding curve slope parameter.
    ///
    /// The slope controls how much the key price increases per unit of supply.
    /// When slope is 0 (the default), the bonding curve is flat (fixed price).
    /// When slope > 0, `price(supply) = KEY_PRICE + slope * supply`.
    pub fn set_curve_slope(env: Env, admin: Address, slope: i128) -> Result<(), ContractError> {
        admin.require_auth();
        if slope < 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        env.storage()
            .persistent()
            .set(&constants::storage::CURVE_SLOPE, &slope);
        Ok(())
    }

    /// Read-only view: returns the current bonding curve slope.
    pub fn get_curve_slope(env: Env) -> i128 {
        read_curve_slope(&env)
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
        let config = read_required_effective_fee_config(&env)?;
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
        let Some(price) = resolve_quote_inputs(&env, &creator)? else {
            return Ok(zero_quote_response());
        };
        let (creator_fee, protocol_fee) = Self::compute_fees_for_payment(env.clone(), price)?;
        checked_format_quote_response(price, creator_fee, protocol_fee, true)
    }

    /// Read-only view: returns the total creator buyback cost for a given amount.
    ///
    /// The returned value is `base_price(amount) + protocol_fee(amount)` because the
    /// creator fee is explicitly waived on buybacks.
    pub fn get_buyback_quote(
        env: Env,
        creator: Address,
        amount: u32,
    ) -> Result<i128, ContractError> {
        if amount == 0 {
            return Ok(0);
        }

        let Some(price) = resolve_quote_inputs(&env, &creator)? else {
            return Ok(0);
        };
        let profile = read_registered_creator_profile(&env, &creator)?;
        if amount > profile.supply {
            return Err(ContractError::InsufficientSupply);
        }

        let base_price = compute_buyback_base_price(price, amount)?;
        let config = read_required_effective_fee_config(&env)?;
        fee::compute_buyback_cost(base_price, config.protocol_bps).ok_or(ContractError::Overflow)
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
        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;

        let Some(normalized) = normalize_quote_amount(base_price)? else {
            return Ok(zero_quote_response());
        };

        let balance = Self::get_key_balance(env.clone(), creator.clone(), holder);
        if balance == 0 {
            return Err(ContractError::InsufficientBalance);
        }

        let profile = read_registered_creator_profile(&env, &creator)?;
        let sell_supply = profile
            .supply
            .checked_sub(1)
            .ok_or(ContractError::SellUnderflow)?;
        let curve_price = compute_bonding_curve_price(&env, &creator, normalized, sell_supply)?;
        let Some(price) = normalize_quote_amount(curve_price)? else {
            return Ok(zero_quote_response());
        };

        let (creator_fee, protocol_fee) = Self::compute_fees_for_payment(env.clone(), price)?;
        checked_format_quote_response(price, creator_fee, protocol_fee, false)
    }

    /// Deposits `amount` as a dividend for all current key holders of `creator`.
    ///
    /// The protocol fee is deducted first; the remainder is distributed proportionally
    /// by dividing net / total_supply (integer floor). Dust from the division is
    /// lost in v1. The per-key accumulator grows by net / supply with each call.
    pub fn distribute_dividend(
        env: Env,
        creator: Address,
        distributor: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        distributor.require_auth();
        assert_not_paused(&env)?;

        if amount <= 0 {
            return Err(ContractError::ZeroDistributionAmount);
        }

        let profile = read_registered_creator_profile(&env, &creator)?;

        if profile.supply == 0 {
            return Err(ContractError::NoKeyHolders);
        }

        let config = read_required_effective_fee_config(&env)?;
        let (net_amount, protocol_amount) =
            fee::compute_fee_split(amount, config.creator_bps, config.protocol_bps);

        credit_protocol_fee_recipient_balance(&env, protocol_amount)?;

        let per_key_net = net_amount / profile.supply as i128;

        let acc_key = constants::storage::dividend_accumulator(&creator);
        let accumulator: i128 = env.storage().persistent().get(&acc_key).unwrap_or(0);
        let new_accumulator = fee::checked_accumulate(accumulator, per_key_net)?;
        env.storage().persistent().set(&acc_key, &new_accumulator);

        env.events().publish(
            events::dividend_distributed_topics(&creator),
            events::DividendDistributedEvent {
                creator: creator.clone(),
                total_amount: amount,
                snapshot_supply: profile.supply,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    /// Claims all accrued dividends for `holder` on `creator`'s keys.
    ///
    /// Reads the current claimable amount (pending + earned since last checkpoint),
    /// resets both pending and checkpoint, and returns the total claimed amount.
    /// Errors with `NoDividendClaimable` when nothing is due.
    pub fn claim_dividend(
        env: Env,
        creator: Address,
        holder: Address,
    ) -> Result<i128, ContractError> {
        holder.require_auth();
        assert_not_paused(&env)?;

        let claimable = compute_claimable_dividend(&env, &creator, &holder);
        if claimable == 0 {
            return Err(ContractError::NoDividendClaimable);
        }

        let accumulator = read_dividend_accumulator(&env, &creator);
        env.storage().persistent().set(
            &constants::storage::holder_dividend_pending(&creator, &holder),
            &0i128,
        );
        env.storage().persistent().set(
            &constants::storage::holder_dividend_checkpoint(&creator, &holder),
            &accumulator,
        );

        env.events().publish(
            events::dividend_claimed_topics(&creator, &holder),
            events::DividendClaimedEvent {
                creator: creator.clone(),
                claimant: holder.clone(),
                amount: claimable,
            },
        );

        Ok(claimable)
    }

    pub fn batch_claim_dividend(
        env: Env,
        creators: soroban_sdk::Vec<Address>,
        holder: Address,
    ) -> Result<soroban_sdk::Vec<ClaimResult>, ContractError> {
        holder.require_auth();
        assert_not_paused(&env)?;

        if creators.len() > 20 {
            return Err(ContractError::BatchClaimExceedsLimit);
        }

        let mut results = soroban_sdk::Vec::new(&env);

        for creator in creators.iter() {
            let claimable = compute_claimable_dividend(&env, &creator, &holder);

            if claimable > 0 {
                let accumulator = read_dividend_accumulator(&env, &creator);
                env.storage().persistent().set(
                    &constants::storage::holder_dividend_pending(&creator, &holder),
                    &0i128,
                );
                env.storage().persistent().set(
                    &constants::storage::holder_dividend_checkpoint(&creator, &holder),
                    &accumulator,
                );

                env.events().publish(
                    events::dividend_claimed_topics(&creator, &holder),
                    events::DividendClaimedEvent {
                        creator: creator.clone(),
                        claimant: holder.clone(),
                        amount: claimable,
                    },
                );
            }

            results.push_back(ClaimResult {
                creator: creator.clone(),
                amount_claimed: claimable,
            });
        }

        Ok(results)
    }

    /// Read-only view: returns the total unclaimed dividend amount for `wallet` on `creator`.
    ///
    /// Returns `0` when no dividends have accumulated or wallet holds no keys.
    /// Never mutates state.
    pub fn get_claimable_dividend(env: Env, creator: Address, wallet: Address) -> i128 {
        compute_claimable_dividend(&env, &creator, &wallet)
    }

    /// Claims time-locked key allocation for a creator.
    ///
    /// Only callable by the creator after the unlock_ledger has been reached.
    /// Transfers the locked keys to the creator's wallet and can only be called once.
    pub fn claim_locked_allocation(env: Env, creator: Address) -> Result<(), ContractError> {
        creator.require_auth();
        assert_not_paused(&env)?;

        let locked_key = constants::storage::locked_allocation(&creator);
        let mut locked: LockedAllocation = env
            .storage()
            .persistent()
            .get(&locked_key)
            .ok_or(ContractError::NotRegistered)?;

        if locked.claimed {
            return Err(ContractError::AlreadyClaimed);
        }

        let current_ledger = env.ledger().sequence();
        if current_ledger < locked.unlock_ledger {
            return Err(ContractError::AllocationLocked);
        }

        // Mark as claimed
        locked.claimed = true;
        env.storage().persistent().set(&locked_key, &locked);

        // Transfer keys to creator's balance
        let balance_key = constants::storage::holder_balance_key(&creator, &creator);
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        let new_balance = current_balance
            .checked_add(locked.amount)
            .ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&balance_key, &new_balance);

        // Update holder count if this is the creator's first key
        if current_balance == 0 {
            let mut profile = read_registered_creator_profile(&env, &creator)?;
            profile.holder_count = profile
                .holder_count
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
            let profile_key = constants::storage::creator(&creator);
            env.storage().persistent().set(&profile_key, &profile);
        }

        env.events().publish(
            (events::ALLOCATION_CLAIMED_EVENT_NAME, creator.clone()),
            events::AllocationClaimedEvent {
                creator_id: creator.clone(),
                amount: locked.amount,
                ledger: current_ledger,
            },
        );

        Ok(())
    }

    /// Read-only view: returns the locked allocation for a creator.
    ///
    /// Returns `None` if no locked allocation exists.
    pub fn get_locked_allocation(env: Env, creator: Address) -> Option<LockedAllocation> {
        env.storage()
            .persistent()
            .get(&constants::storage::locked_allocation(&creator))
    }

    /// Updates the protocol fee recipient address.
    ///
    /// Only callable by the current protocol admin. Emits an event with old and new addresses.
    pub fn update_protocol_fee_recipient(
        env: Env,
        admin: Address,
        new_recipient: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        validate_non_zero_address(&env, &new_recipient)?;

        let old_recipient: Address = env
            .storage()
            .persistent()
            .get(&constants::storage::PROTOCOL_FEE_RECIPIENT)
            .ok_or(ContractError::Unauthorized)?;

        if old_recipient == new_recipient {
            return Ok(());
        }

        env.storage()
            .persistent()
            .set(&constants::storage::PROTOCOL_FEE_RECIPIENT, &new_recipient);

        env.events().publish(
            (events::PROTOCOL_FEE_RECIPIENT_UPDATED_EVENT_NAME, admin),
            events::ProtocolFeeRecipientUpdatedEvent {
                old_recipient,
                new_recipient,
            },
        );

        Ok(())
    }

    /// Updates the creator fee recipient address.
    ///
    /// Only callable by the current fee recipient for that creator (self-rotation).
    pub fn update_creator_fee_recipient(
        env: Env,
        creator: Address,
        new_recipient: Address,
    ) -> Result<(), ContractError> {
        let profile = read_registered_creator_profile(&env, &creator)?;
        let current_recipient = profile.fee_recipient.clone();
        current_recipient.require_auth();
        validate_non_zero_address(&env, &new_recipient)?;

        if current_recipient == new_recipient {
            return Ok(());
        }

        let mut profile = profile;
        profile.fee_recipient = new_recipient.clone();
        let key = constants::storage::creator(&creator);
        env.storage().persistent().set(&key, &profile);

        env.events().publish(
            (
                events::CREATOR_FEE_RECIPIENT_UPDATED_EVENT_NAME,
                creator.clone(),
            ),
            events::CreatorFeeRecipientUpdatedEvent {
                creator_id: creator,
                old_recipient: current_recipient,
                new_recipient,
            },
        );

        Ok(())
    }

    /// Read-only view: returns the max supply cap for a creator.
    /// Read-only view: returns the max supply cap for a creator.
    ///
    /// Returns `None` if no max supply cap is set (uncapped).
    pub fn get_max_supply(env: Env, creator: Address) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&constants::storage::max_supply(&creator))
    }

    /// Read-only view: returns the curve preset for a creator.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    pub fn get_curve_preset(env: Env, creator: Address) -> Result<CurvePreset, ContractError> {
        if !env
            .storage()
            .persistent()
            .has(&constants::storage::creator(&creator))
        {
            return Err(ContractError::NotRegistered);
        }
        let preset = env
            .storage()
            .persistent()
            .get(&constants::storage::curve_preset(&creator))
            .unwrap_or(CurvePreset::Flat);
        Ok(preset)
    }

    /// Transfers key ownership between wallets without touching the bonding curve.
    ///
    /// The sender's balance is decremented and the recipient's balance is
    /// incremented by `amount`. Total supply is unchanged so the bonding curve
    /// price is not affected.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    /// - [`ContractError::ZeroTransferAmount`] if `amount` is zero.
    /// - [`ContractError::SelfTransfer`] if the sender is the same as the recipient.
    /// - [`ContractError::InsufficientBalance`] if the sender holds fewer keys than `amount`.
    pub fn transfer_keys(
        env: Env,
        creator: Address,
        from: Address,
        to: Address,
        amount: u32,
    ) -> Result<(), ContractError> {
        from.require_auth();
        assert_not_paused(&env)?;

        if amount == 0 {
            return Err(ContractError::ZeroTransferAmount);
        }
        if from == to {
            return Err(ContractError::SelfTransfer);
        }

        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

        let from_balance_key = constants::storage::holder_balance_key(&creator, &from);
        let from_balance: u32 = env
            .storage()
            .persistent()
            .get(&from_balance_key)
            .unwrap_or(0);

        // Settle dividends for sender before balance changes.
        settle_holder_dividends(&env, &creator, &from)?;

        if from_balance < amount {
            return Err(ContractError::InsufficientBalance);
        }

        // Settle dividends for recipient before balance changes.
        let to_balance_key = constants::storage::holder_balance_key(&creator, &to);
        let to_balance: u32 = env.storage().persistent().get(&to_balance_key).unwrap_or(0);
        settle_holder_dividends(&env, &creator, &to)?;

        // Update sender balance.
        let new_from_balance = from_balance
            .checked_sub(amount)
            .ok_or(ContractError::InsufficientBalance)?;
        env.storage()
            .persistent()
            .set(&from_balance_key, &new_from_balance);

        // Decrement holder count if sender balance reaches zero.
        if new_from_balance == 0 {
            profile.holder_count = profile
                .holder_count
                .checked_sub(1)
                .ok_or(ContractError::Overflow)?;
        }

        // Update recipient balance.
        let new_to_balance = to_balance
            .checked_add(amount)
            .ok_or(ContractError::Overflow)?;
        env.storage()
            .persistent()
            .set(&to_balance_key, &new_to_balance);

        // Increment holder count if recipient had zero balance before.
        if to_balance == 0 {
            profile.holder_count = profile
                .holder_count
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
        }

        // Write updated profile (holder_count changes).
        let profile_key = constants::storage::creator(&creator);
        env.storage().persistent().set(&profile_key, &profile);

        env.events().publish(
            (
                events::KEYS_TRANSFERRED_EVENT_NAME,
                creator.clone(),
                from.clone(),
            ),
            events::KeysTransferredEvent {
                creator_id: creator,
                from,
                to,
                amount,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    /// Returns the current withdrawable treasury balance.
    ///
    /// The treasury balance accumulates from the protocol fee portion of every
    /// `buy_key` and `sell_key` operation. Returns `0` before any fees have accrued.
    /// This method does not mutate contract state.
    pub fn get_treasury_balance(env: Env) -> i128 {
        read_treasury_balance(&env)
    }

    /// Withdraws `amount` from the protocol treasury to `recipient`.
    ///
    /// Only callable by the protocol admin (set via [`set_protocol_admin`]).
    /// Reverts with:
    /// - [`ContractError::Unauthorized`] if the caller is not the protocol admin.
    /// - [`ContractError::NotPositiveAmount`] if `amount` is zero or negative.
    /// - [`ContractError::InsufficientTreasuryBalance`] if `amount` exceeds the
    ///   current treasury balance.
    ///
    /// On success, decrements the treasury balance and emits a
    /// [`events::TreasuryWithdrawalEvent`].
    /// Partial withdrawals are supported; full withdrawal leaves the balance at zero.
    pub fn withdraw_treasury(
        env: Env,
        admin: Address,
        amount: i128,
        recipient: Address,
    ) -> Result<i128, ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        if amount <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let current = read_treasury_balance(&env);
        if amount > current {
            return Err(ContractError::InsufficientTreasuryBalance);
        }

        let remaining = current.checked_sub(amount).ok_or(ContractError::Overflow)?;
        env.storage()
            .persistent()
            .set(&constants::storage::TREASURY_BALANCE, &remaining);

        env.events().publish(
            events::treasury_withdrawal_event_topics(&recipient),
            events::TreasuryWithdrawalEvent {
                amount,
                recipient,
                remaining_balance: remaining,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(remaining)
    }

    pub fn query_supply(env: Env, creator: Address) -> Result<u32, ContractError> {
        Self::get_creator_supply(env, creator)
    }

    /// Read-only view: returns the configured referral fee basis points.
    ///
    /// Returns the default value when no custom value has been set.
    pub fn get_referral_fee_bps(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get::<DataKey, u32>(&constants::storage::referral_fee_bps())
            .unwrap_or(DEFAULT_REFERRAL_FEE_BPS)
    }

    /// Updates the referral fee basis points.
    ///
    /// Only callable by the protocol admin.
    pub fn set_referral_fee_bps(env: Env, admin: Address, bps: u32) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        if bps > fee::BPS_MAX {
            return Err(ContractError::InvalidFeeConfig);
        }
        env.storage()
            .persistent()
            .set(&constants::storage::referral_fee_bps(), &bps);
        Ok(())
    }

    /// Read-only view: returns the per-wallet cap for a creator.
    ///
    /// Returns `None` if no cap is set.
    pub fn get_wallet_cap(env: Env, creator: Address) -> Option<u32> {
        env.storage()
            .persistent()
            .get::<DataKey, u32>(&constants::storage::max_keys_per_wallet(&creator))
    }

    /// Read-only view: returns cumulative creator volume.
    ///
    /// Returns `0` when no volume has been recorded.
    pub fn get_creator_volume(env: Env, creator: Address) -> i128 {
        env.storage()
            .persistent()
            .get::<DataKey, i128>(&constants::storage::creator_volume(&creator))
            .unwrap_or(0)
    }

    /// Updates discount tiers (admin-only).
    ///
    /// Replaces the full tier list. Maximum 5 tiers allowed.
    pub fn update_discount_tiers(
        env: Env,
        admin: Address,
        tiers: Vec<DiscountTier>,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        if tiers.len() > MAX_DISCOUNT_TIERS {
            return Err(ContractError::DiscountTierLimitExceeded);
        }
        env.storage()
            .persistent()
            .set(&constants::storage::discount_tiers(), &tiers);
        Ok(())
    }

    /// Read-only view: returns the current discount tiers.
    pub fn get_discount_tiers(env: Env) -> Vec<DiscountTier> {
        env.storage()
            .persistent()
            .get::<DataKey, Vec<DiscountTier>>(&constants::storage::discount_tiers())
            .unwrap_or(Vec::new(&env))
    }
}

// ============================================================================
// STAKING & STAKE RECEIPT NFT (Feature 1)
// ============================================================================

pub fn stake_key(
    env: Env,
    creator: Address,
    staker: Address,
    amount: u32,
    lock_ledgers: u32,
) -> Result<u32, ContractError> {
    staker.require_auth();
    assert_not_paused(&env)?;
    read_registered_creator_profile(&env, &creator)?;
    if amount == 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    if lock_ledgers == 0 {
        return Err(ContractError::InvalidLockPeriod);
    }
    let bal_key = constants::storage::holder_balance_key(&creator, &staker);
    let liquid: u32 = env.storage().persistent().get(&bal_key).unwrap_or(0);
    if liquid < amount {
        return Err(ContractError::InsufficientBalance);
    }
    settle_holder_dividends(&env, &creator, &staker)?;
    let mut profile = read_registered_creator_profile(&env, &creator)?;
    let new_l = liquid
        .checked_sub(amount)
        .ok_or(ContractError::InsufficientBalance)?;
    if new_l == 0 {
        profile.holder_count = profile
            .holder_count
            .checked_sub(1)
            .ok_or(ContractError::SellUnderflow)?;
    }
    env.storage().persistent().set(&bal_key, &new_l);
    env.storage()
        .persistent()
        .set(&constants::storage::creator(&creator), &profile);
    let staked = read_staked_keys(&env, &creator, &staker);
    write_staked_keys(
        &env,
        &creator,
        &staker,
        staked.checked_add(amount).ok_or(ContractError::Overflow)?,
    );
    let cur = env.ledger().sequence();
    let unlock = cur
        .checked_add(lock_ledgers)
        .ok_or(ContractError::Overflow)?;
    let sid = assign_stake_id(&env, &creator, &staker)?;
    let pos = StakePosition {
        stake_id: sid,
        amount,
        unlock_ledger: unlock,
    };
    write_position(&env, &creator, &staker, &pos);
    let tid = assign_token_id(&env)?;
    let rec = StakeNftRecord {
        token_id: tid,
        creator: creator.clone(),
        stake_id: sid,
        owner: staker.clone(),
        amount,
        unlock_ledger: unlock,
    };
    env.storage()
        .persistent()
        .set(&constants::storage::stake_nft(tid), &rec);
    env.storage().persistent().set(
        &constants::storage::stake_nft_id(&creator, sid, &staker),
        &tid,
    );
    change_nft_holder_count(&env, &staker, 1)?;
    change_nft_total_supply(&env, 1)?;
    write_creator_supply(&env, &creator, profile.supply);
    extend_stake_ttl(&env, &creator, &staker, sid, tid);
    extend_creator_ttl(&env, &creator);
    env.events().publish(
        events::stake_nft_minted_topics(&creator, &staker),
        events::StakeNftMintedEvent {
            token_id: tid,
            creator,
            stake_id: sid,
            owner: staker,
            amount,
            unlock_ledger: unlock,
            ledger: cur,
        },
    );
    Ok(sid)
}

pub fn unstake_key(
    env: Env,
    creator: Address,
    holder: Address,
    stake_id: u32,
) -> Result<u32, ContractError> {
    holder.require_auth();
    assert_not_paused(&env)?;
    let pos = read_position(&env, &creator, &holder, stake_id)
        .ok_or(ContractError::StakePositionNotFound)?;
    if env.ledger().sequence() < pos.unlock_ledger {
        return Err(ContractError::StakeStillLocked);
    }
    settle_holder_dividends(&env, &creator, &holder)?;
    let sup = read_registered_creator_profile(&env, &creator)?.supply;
    burn_receipt(&env, &creator, &holder, &pos);
    Ok(sup)
}

pub fn get_stake_position(
    env: Env,
    creator: Address,
    owner: Address,
    stake_id: u32,
) -> Option<StakePosition> {
    read_position(&env, &creator, &owner, stake_id)
}
pub fn get_stake_nft(env: Env, token_id: u64) -> Option<StakeNftRecord> {
    read_nft(&env, token_id)
}
pub fn get_staked_keys(env: Env, creator: Address, owner: Address) -> u32 {
    read_staked_keys(&env, &creator, &owner)
}
pub fn get_stake_nft_id(env: Env, creator: Address, stake_id: u32, owner: Address) -> Option<u64> {
    read_nft_id(&env, &creator, stake_id, &owner)
}

pub fn name(_env: Env) -> String {
    String::from_str(&_env, STAKE_NFT_NAME)
}
pub fn symbol(_env: Env) -> String {
    String::from_str(&_env, STAKE_NFT_SYMBOL)
}
pub fn decimals(_env: Env) -> u32 {
    STAKE_NFT_DECIMALS
}
pub fn total_supply(env: Env) -> i128 {
    read_nft_total_supply(&env)
}
pub fn balance(env: Env, owner: Address) -> i128 {
    read_nft_balance(&env, &owner)
}
pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
    read_allowance(&env, &owner, &spender)
}

pub fn approve(
    env: Env,
    owner: Address,
    spender: Address,
    amount: i128,
    exp: u32,
) -> Result<i128, ContractError> {
    owner.require_auth();
    if amount < 0 {
        return Err(ContractError::InvalidTokenAmount);
    }
    let eff = if exp <= env.ledger().sequence() {
        0
    } else {
        amount
    };
    write_allowance(
        &env,
        &owner,
        &spender,
        &TokenAllowance {
            amount: eff,
            expiration_ledger: exp,
        },
    );
    Ok(eff)
}

pub fn transfer(
    env: Env,
    from: Address,
    to: Address,
    token_id: u64,
    amount: i128,
    spender: Address,
) -> Result<Vec<Address>, ContractError> {
    assert_not_paused(&env)?;
    if amount != 1 {
        return Err(ContractError::InvalidTokenAmount);
    }
    let rec = read_nft(&env, token_id).ok_or(ContractError::StakeNftNotFound)?;
    if rec.owner != from {
        return Err(ContractError::StakeNftNotOwned);
    }
    if from == to {
        return Err(ContractError::SelfStakeNftTransfer);
    }
    let used = consume_allowance(&env, &from, &spender, amount)?;
    if !used && spender != from {
        return Err(ContractError::SpenderNotAuthorized);
    }
    if used {
        spender.require_auth();
    }
    let pos = read_position(&env, &rec.creator, &from, rec.stake_id)
        .ok_or(ContractError::StakePositionNotFound)?;
    settle_holder_dividends(&env, &rec.creator, &from)?;
    settle_holder_dividends(&env, &rec.creator, &to)?;
    let fs = read_staked_keys(&env, &rec.creator, &from);
    write_staked_keys(
        &env,
        &rec.creator,
        &from,
        fs.checked_sub(pos.amount)
            .ok_or(ContractError::InsufficientBalance)?,
    );
    let ts = read_staked_keys(&env, &rec.creator, &to);
    write_staked_keys(
        &env,
        &rec.creator,
        &to,
        ts.checked_add(pos.amount).ok_or(ContractError::Overflow)?,
    );
    env.storage()
        .persistent()
        .remove(&constants::storage::stake_position(
            &rec.creator,
            &from,
            pos.stake_id,
        ));
    write_position(&env, &rec.creator, &to, &pos);
    env.storage()
        .persistent()
        .remove(&constants::storage::stake_nft_id(
            &rec.creator,
            pos.stake_id,
            &from,
        ));
    env.storage().persistent().set(
        &constants::storage::stake_nft_id(&rec.creator, pos.stake_id, &to),
        &token_id,
    );
    let creator_clone = rec.creator.clone();
    let upd = StakeNftRecord {
        token_id,
        creator: creator_clone.clone(),
        stake_id: rec.stake_id,
        owner: to.clone(),
        amount: rec.amount,
        unlock_ledger: rec.unlock_ledger,
    };
    env.storage()
        .persistent()
        .set(&constants::storage::stake_nft(token_id), &upd);
    change_nft_holder_count(&env, &from, -1)?;
    change_nft_holder_count(&env, &to, 1)?;
    extend_stake_ttl(&env, &creator_clone, &from, rec.stake_id, token_id);
    extend_stake_ttl(&env, &creator_clone, &to, rec.stake_id, token_id);
    extend_creator_ttl(&env, &rec.creator);
    env.events().publish(
        events::stake_nft_transferred_topics(&from, &to),
        events::StakeNftTransferredEvent {
            token_id,
            creator: rec.creator,
            stake_id: rec.stake_id,
            from,
            to,
            amount,
            ledger: env.ledger().sequence(),
        },
    );
    Ok(Vec::new(&env))
}

pub fn burn(
    env: Env,
    from: Address,
    token_id: u64,
    amount: i128,
    auth: Address,
) -> Result<i128, ContractError> {
    assert_not_paused(&env)?;
    if amount != 1 {
        return Err(ContractError::InvalidTokenAmount);
    }
    let rec = read_nft(&env, token_id).ok_or(ContractError::StakeNftNotFound)?;
    if rec.owner != from {
        return Err(ContractError::StakeNftNotOwned);
    }
    let used = consume_allowance(&env, &from, &auth, amount)?;
    if !used && auth != from {
        return Err(ContractError::SpenderNotAuthorized);
    }
    if used {
        auth.require_auth();
    }
    if env.ledger().sequence() < rec.unlock_ledger {
        return Err(ContractError::StakeStillLocked);
    }
    settle_holder_dividends(&env, &rec.creator, &from)?;
    burn_receipt(
        &env,
        &rec.creator,
        &from,
        &StakePosition {
            stake_id: rec.stake_id,
            amount: rec.amount,
            unlock_ledger: rec.unlock_ledger,
        },
    );
    extend_creator_ttl(&env, &rec.creator);
    Ok(read_nft_total_supply(&env))
}

pub fn burn_from(
    env: Env,
    from: Address,
    token_id: u64,
    amount: i128,
    spender: Address,
) -> Result<i128, ContractError> {
    assert_not_paused(&env)?;
    spender.require_auth();
    if amount != 1 {
        return Err(ContractError::InvalidTokenAmount);
    }
    let rec = read_nft(&env, token_id).ok_or(ContractError::StakeNftNotFound)?;
    if rec.owner != from {
        return Err(ContractError::StakeNftNotOwned);
    }
    let bk = constants::storage::stake_nft_burned(&spender, token_id);
    if env.storage().persistent().has(&bk) {
        return Err(ContractError::StakeNftNotFound);
    }
    let cur = read_allowance(&env, &from, &spender);
    if cur < amount {
        return Err(ContractError::InsufficientAllowance);
    }
    let ex = env
        .storage()
        .persistent()
        .get::<DataKey, TokenAllowance>(&constants::storage::stake_nft_allowance(&from, &spender))
        .map(|s| s.expiration_ledger)
        .unwrap_or(0);
    write_allowance(
        &env,
        &from,
        &spender,
        &TokenAllowance {
            amount: cur - amount,
            expiration_ledger: ex,
        },
    );
    env.storage().persistent().set(
        &constants::storage::stake_nft_burned(&spender, token_id),
        &true,
    );
    if env.ledger().sequence() < rec.unlock_ledger {
        return Err(ContractError::StakeStillLocked);
    }
    settle_holder_dividends(&env, &rec.creator, &from)?;
    burn_receipt(
        &env,
        &rec.creator,
        &from,
        &StakePosition {
            stake_id: rec.stake_id,
            amount: rec.amount,
            unlock_ledger: rec.unlock_ledger,
        },
    );
    extend_creator_ttl(&env, &rec.creator);
    Ok(read_nft_total_supply(&env))
}

// ============================================================================
// VAULT REBALANCING (Feature 2)
// ============================================================================

pub fn set_target_weights(
    env: Env,
    admin: Address,
    creator: Address,
    weights: Vec<TargetWeight>,
) -> Result<(), ContractError> {
    admin.require_auth();
    assert_is_admin_or_governance(&env, &admin)?;
    validate_target_weights(&weights)?;
    let prev = read_target_weights(&env, &creator);
    env.storage().persistent().set(
        &constants::storage::vault_target_weights(&creator),
        &weights,
    );
    let mut a = Vec::new(&env);
    let mut m = false;
    for w in weights.iter() {
        let mut c = 0;
        for o in prev.iter() {
            if o.key == w.key {
                c = 0;
                m = true;
            }
        }
        a.push_back(VaultAllocation {
            key: w.key,
            units: c,
            value: 0,
        });
    }
    if !m {
        a = zero_allocations(&env, &weights);
    }
    write_allocations(&env, &creator, &a);
    Ok(())
}

pub fn set_vault_key_price(
    env: Env,
    admin: Address,
    creator: Address,
    key: Address,
    price: i128,
) -> Result<(), ContractError> {
    admin.require_auth();
    assert_is_admin_or_governance(&env, &admin)?;
    if price <= 0 {
        return Err(ContractError::InvalidTargetWeights);
    }
    env.storage()
        .persistent()
        .set(&constants::storage::vault_key_price(&creator, &key), &price);
    Ok(())
}

pub fn set_vault_tolerance_bps(
    env: Env,
    admin: Address,
    creator: Address,
    tol: u32,
) -> Result<(), ContractError> {
    admin.require_auth();
    assert_is_admin_or_governance(&env, &admin)?;
    if tol > fee::BPS_MAX {
        return Err(ContractError::InvalidTargetWeights);
    }
    env.storage()
        .persistent()
        .set(&constants::storage::vault_tolerance_bps(&creator), &tol);
    Ok(())
}

pub fn seed_vault_allocations(
    env: Env,
    admin: Address,
    creator: Address,
    allocs: Vec<VaultAllocation>,
) -> Result<(), ContractError> {
    admin.require_auth();
    assert_is_admin_or_governance(&env, &admin)?;
    let w = read_target_weights(&env, &creator);
    if w.is_empty() || allocs.len() != w.len() {
        return Err(ContractError::VaultWeightsNotSet);
    }
    let mut n = Vec::new(&env);
    for (i, a) in allocs.iter().enumerate() {
        let wt = w.get(i as u32).ok_or(ContractError::InvalidTargetWeights)?;
        if a.key != wt.key {
            return Err(ContractError::InvalidTargetWeights);
        }
        if a.units < 0 {
            return Err(ContractError::InvalidTargetWeights);
        }
        let p =
            read_key_price(&env, &creator, &wt.key).ok_or(ContractError::InvalidTargetWeights)?;
        n.push_back(VaultAllocation {
            key: a.key,
            units: a.units,
            value: a.units.checked_mul(p).ok_or(ContractError::Overflow)?,
        });
    }
    write_allocations(&env, &creator, &n);
    Ok(())
}

pub fn rebalance(
    env: Env,
    admin: Address,
    creator: Address,
    max_slip: u32,
) -> Result<VaultRebalanceSummary, ContractError> {
    admin.require_auth();
    assert_is_admin_or_governance(&env, &admin)?;
    let w = read_target_weights(&env, &creator);
    if w.is_empty() {
        return Err(ContractError::VaultWeightsNotSet);
    }
    let p = resolve_prices(&env, &creator, &w)?;
    let u = resolve_units(&env, &w, &read_allocations(&env, &creator))?;
    let priced = value_allocations(&env, &w, &u, &p)?;
    let tot = total_value_of(&priced)?;
    let tgt = compute_target_values(&env, &w, tot)?;
    let mut tu = Vec::new(&env);
    for i in 0..w.len() {
        tu.push_back(
            tgt.get(i)
                .unwrap_or(0)
                .checked_div(p.get(i).unwrap_or(1))
                .ok_or(ContractError::Overflow)?,
        );
    }
    let (tr, su) = match_sellers_to_buyers(&env, &w, &u, &p, &tu, max_slip)?;
    let al = value_allocations(&env, &w, &su, &p)?;
    let tv = total_value_of(&al)?;
    write_allocations(&env, &creator, &al);
    env.events().publish(
        events::rebalance_executed_topics(&creator),
        events::RebalanceExecutedEvent {
            creator: creator.clone(),
            trades: tr.clone(),
            allocations: al.clone(),
            total_value: tv,
            max_slippage_bps: max_slip,
            ledger: env.ledger().sequence(),
        },
    );
    Ok(VaultRebalanceSummary {
        creator,
        trades: tr,
        allocations: al,
        total_value: tv,
        max_slippage_bps: max_slip,
    })
}

pub fn get_target_weights(env: Env, creator: Address) -> Vec<TargetWeight> {
    read_target_weights(&env, &creator)
}
pub fn get_vault_allocations(env: Env, creator: Address) -> Vec<VaultAllocation> {
    read_allocations(&env, &creator)
}
pub fn get_vault_total_value(env: Env, creator: Address) -> i128 {
    let w = read_target_weights(&env, &creator);
    if w.is_empty() {
        return 0;
    }
    let Ok(p) = resolve_prices(&env, &creator, &w) else {
        return 0;
    };
    let a = read_allocations(&env, &creator);
    let mut t: i128 = 0;
    for (i, al) in a.iter().enumerate() {
        t = t
            .checked_add(
                al.units
                    .checked_mul(p.get(i as u32).unwrap_or(0))
                    .unwrap_or(0),
            )
            .unwrap_or(0);
    }
    t
}
pub fn get_vault_key_price(env: Env, creator: Address, key: Address) -> Option<i128> {
    read_key_price(&env, &creator, &key)
}
pub fn get_vault_tolerance_bps(env: Env, creator: Address) -> u32 {
    read_tolerance_bps(&env, &creator)
}
pub fn get_vault_drift(env: Env, creator: Address) -> Vec<VaultDrift> {
    let w = read_target_weights(&env, &creator);
    if w.is_empty() {
        return Vec::new(&env);
    }
    let Ok(p) = resolve_prices(&env, &creator, &w) else {
        return Vec::new(&env);
    };
    let Ok(u) = resolve_units(&env, &w, &read_allocations(&env, &creator)) else {
        return Vec::new(&env);
    };
    let Ok(a) = value_allocations(&env, &w, &u, &p) else {
        return Vec::new(&env);
    };
    let tv = total_value_of(&a).unwrap_or(0);
    let Ok(tgt) = compute_target_values(&env, &w, tv) else {
        return Vec::new(&env);
    };
    let mut d = Vec::new(&env);
    for (i, w) in w.iter().enumerate() {
        let cv = a.get(i as u32).map(|x| x.value).unwrap_or(0);
        let tv = tgt.get(i as u32).unwrap_or(0);
        let diff = cv - tv;
        let mag = if diff < 0 { -diff } else { diff };
        let drift = if tv > 0 {
            mag.checked_mul(i128::from(fee::BPS_MAX)).unwrap_or(0) / tv
        } else {
            0
        };
        d.push_back(VaultDrift {
            key: w.key,
            weight_bps: w.weight_bps,
            target_value: tv,
            current_value: cv,
            drift_bps: drift,
        });
    }
    d
}
pub fn is_vault_within_tolerance(env: Env, creator: Address) -> bool {
    let t = i128::from(read_tolerance_bps(&env, &creator));
    let d = get_vault_drift(env.clone(), creator.clone());
    if d.is_empty() {
        return false;
    }
    for x in d.iter() {
        if x.drift_bps > t {
            return false;
        }
    }
    true
}

pub fn get_governance_address(env: Env) -> Option<Address> {
    read_governance_address(&env)
}
pub fn set_governance_address(env: Env, admin: Address, gov: Address) -> Result<(), ContractError> {
    admin.require_auth();
    assert_is_admin_or_governance(&env, &admin)?;
    validate_non_zero_address(&env, &gov)?;
    env.storage()
        .persistent()
        .set(&constants::storage::GOVERNANCE_ADDRESS, &gov);
    Ok(())
}

// ============================================================================
// DYNAMIC FEE TIERS (Feature 3)
// ============================================================================

pub fn set_fee_tiers(env: Env, admin: Address, tiers: Vec<FeeTier>) -> Result<(), ContractError> {
    admin.require_auth();
    assert_is_admin_or_governance(&env, &admin)?;
    if tiers.is_empty() {
        env.storage()
            .persistent()
            .remove(&constants::storage::FEE_TIERS);
        return Ok(());
    }
    validate_fee_tiers(&tiers)?;
    env.storage()
        .persistent()
        .set(&constants::storage::FEE_TIERS, &tiers);
    Ok(())
}

pub fn get_current_fee(env: Env) -> u32 {
    current_protocol_bps(&env)
}
pub fn get_dynamic_fee_view(env: Env) -> DynamicFeeView {
    let s = read_protocol_fee_config(&env);
    let p = current_protocol_bps(&env);
    let t = read_fee_tiers(&env);
    DynamicFeeView {
        protocol_bps: p,
        creator_bps: if s.is_some() { fee::BPS_MAX - p } else { 0 },
        rolling_volume: rolling_volume(&env),
        tier_index: active_tier_index(&t, rolling_volume(&env)),
        is_configured: !t.is_empty(),
    }
}
pub fn get_fee_tiers(env: Env) -> Vec<FeeTier> {
    read_fee_tiers(&env)
}
pub fn get_rolling_volume(env: Env) -> i128 {
    rolling_volume(&env)
}
pub fn get_volume_buckets(env: Env) -> Vec<VolumeBucket> {
    read_volume_buckets(&env)
}

// ============================================================================
// BONDING CURVE RESET (Feature 4)
// ============================================================================

pub fn reset_curve(
    env: Env,
    admin: Address,
    creator: Address,
    reset: u32,
    curve: CurveConfig,
) -> Result<u32, ContractError> {
    admin.require_auth();
    creator.require_auth();
    assert_is_admin(&env, &admin)?;
    assert_not_paused(&env)?;
    if curve.slope < 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    let p = read_registered_creator_profile(&env, &creator)?;
    if p.supply != 0 {
        return Err(ContractError::InsufficientSupply);
    }
    if reset > p.supply {
        return Err(ContractError::NotPositiveAmount);
    }
    let old = p.supply;
    env.storage()
        .persistent()
        .set(&constants::storage::curve_preset(&creator), &curve.preset);
    env.storage().persistent().set(
        &constants::storage::creator_curve_slope(&creator),
        &curve.slope,
    );
    write_creator_supply(&env, &creator, reset);
    let c = read_reset_count(&env, &creator)
        .checked_add(1)
        .ok_or(ContractError::Overflow)?;
    env.storage()
        .persistent()
        .set(&constants::storage::curve_reset_count(&creator), &c);
    extend_creator_ttl(&env, &creator);
    env.events().publish(
        events::curve_reset_topics(&creator),
        events::CurveResetEvent {
            creator,
            old_supply: old,
            new_supply: reset,
            preset: curve.preset,
            slope: curve.slope,
            reset_count: c,
            ledger: env.ledger().sequence(),
        },
    );
    Ok(reset)
}
pub fn get_curve_reset_count(env: Env, creator: Address) -> u32 {
    read_reset_count(&env, &creator)
}
pub fn get_creator_curve_slope(env: Env, creator: Address) -> Option<i128> {
    read_creator_slope(&env, &creator)
}

// ============================================================================
// STAKING & STAKE RECEIPT NFT (Feature 1)
// ============================================================================

/// A minted stake receipt NFT record.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakeNftRecord {
    pub token_id: u64,
    pub creator: Address,
    pub stake_id: u32,
    pub owner: Address,
    pub amount: u32,
    pub unlock_ledger: u32,
}

/// A token-level allowance created by the SEP-41 `approve` entrypoint.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TokenAllowance {
    pub amount: i128,
    pub expiration_ledger: u32,
}

// ============================================================================
// VAULT REBALANCING (Feature 2)
// ============================================================================

/// One key's target share of vault value.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TargetWeight {
    pub key: Address,
    pub weight_bps: u32,
}

/// One key's current holding in the vault.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VaultAllocation {
    pub key: Address,
    pub units: i128,
    pub value: i128,
}

/// Per-key divergence between current and target value.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VaultDrift {
    pub key: Address,
    pub weight_bps: u32,
    pub target_value: i128,
    pub current_value: i128,
    pub drift_bps: i128,
}

/// Result of a successful rebalance call.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VaultRebalanceSummary {
    pub creator: Address,
    pub trades: Vec<events::RebalanceTrade>,
    pub allocations: Vec<VaultAllocation>,
    pub total_value: i128,
    pub max_slippage_bps: u32,
}

// ============================================================================
// DYNAMIC FEE TIERS (Feature 3)
// ============================================================================

/// One dynamic fee tier.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct FeeTier {
    pub volume_threshold: i128,
    pub protocol_bps: u32,
}

/// One slice of the rolling volume window.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VolumeBucket {
    pub bucket_start: u32,
    pub volume: i128,
}

/// Non-optional view of the active dynamic fee.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DynamicFeeView {
    pub protocol_bps: u32,
    pub creator_bps: u32,
    pub rolling_volume: i128,
    pub tier_index: u32,
    pub is_configured: bool,
}

// ============================================================================
// BONDING CURVE RESET (Feature 4)
// ============================================================================

/// Curve parameters applied from a reset point onwards.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CurveConfig {
    pub preset: CurvePreset,
    pub slope: i128,
}

// ============================================================================
// HELPER FUNCTIONS FOR NEW FEATURES
// ============================================================================

// --- Staking helpers ---

fn read_position(
    env: &Env,
    creator: &Address,
    owner: &Address,
    stake_id: u32,
) -> Option<StakePosition> {
    env.storage()
        .persistent()
        .get(&constants::storage::stake_position(
            creator, owner, stake_id,
        ))
}

fn write_position(env: &Env, creator: &Address, owner: &Address, position: &StakePosition) {
    env.storage().persistent().set(
        &constants::storage::stake_position(creator, owner, position.stake_id),
        position,
    );
}

fn read_nft(env: &Env, token_id: u64) -> Option<StakeNftRecord> {
    env.storage()
        .persistent()
        .get(&constants::storage::stake_nft(token_id))
}

fn read_nft_id(env: &Env, creator: &Address, stake_id: u32, owner: &Address) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&constants::storage::stake_nft_id(creator, stake_id, owner))
}

pub fn read_nft_balance(env: &Env, owner: &Address) -> i128 {
    let count: u32 = env
        .storage()
        .persistent()
        .get(&constants::storage::stake_nft_holder_count(owner))
        .unwrap_or(0);
    i128::from(count)
}

pub fn read_nft_total_supply(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&constants::storage::STAKE_NFT_TOTAL_SUPPLY)
        .unwrap_or(0)
}

fn read_allowance(env: &Env, owner: &Address, spender: &Address) -> i128 {
    let stored: Option<TokenAllowance> = env
        .storage()
        .persistent()
        .get(&constants::storage::stake_nft_allowance(owner, spender));
    match stored {
        Some(allowance) if allowance.expiration_ledger > env.ledger().sequence() => {
            allowance.amount
        }
        _ => 0,
    }
}

fn write_allowance(env: &Env, owner: &Address, spender: &Address, allowance: &TokenAllowance) {
    let key = constants::storage::stake_nft_allowance(owner, spender);
    if allowance.amount == 0 {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, allowance);
    }
}

fn consume_allowance(
    env: &Env,
    owner: &Address,
    spender: &Address,
    amount: i128,
) -> Result<bool, ContractError> {
    if spender == owner {
        return Ok(false);
    }
    let current = read_allowance(env, owner, spender);
    if current == 0 {
        return Ok(false);
    }
    if current < amount {
        return Err(ContractError::InsufficientAllowance);
    }
    let expiry = env
        .storage()
        .persistent()
        .get::<DataKey, TokenAllowance>(&constants::storage::stake_nft_allowance(owner, spender))
        .map(|s| s.expiration_ledger)
        .unwrap_or(0);
    write_allowance(
        env,
        owner,
        spender,
        &TokenAllowance {
            amount: current - amount,
            expiration_ledger: expiry,
        },
    );
    Ok(true)
}

fn change_nft_holder_count(env: &Env, owner: &Address, delta: i128) -> Result<(), ContractError> {
    let key = constants::storage::stake_nft_holder_count(owner);
    let current: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    let updated = i128::from(current)
        .checked_add(delta)
        .ok_or(ContractError::Overflow)?;
    if updated < 0 {
        return Err(ContractError::InsufficientAllowance);
    }
    env.storage().persistent().set(
        &key,
        &u32::try_from(updated).map_err(|_| ContractError::Overflow)?,
    );
    Ok(())
}

fn change_nft_total_supply(env: &Env, delta: i128) -> Result<(), ContractError> {
    let current = read_nft_total_supply(env);
    let updated = current.checked_add(delta).ok_or(ContractError::Overflow)?;
    if updated < 0 {
        return Err(ContractError::InsufficientAllowance);
    }
    env.storage()
        .persistent()
        .set(&constants::storage::STAKE_NFT_TOTAL_SUPPLY, &updated);
    Ok(())
}

fn extend_stake_ttl(env: &Env, creator: &Address, owner: &Address, stake_id: u32, token_id: u64) {
    let current = env.ledger().sequence();
    let extend_to = current + STAKE_TTL_LEDGERS;
    let p = env.storage().persistent();
    for key in [
        constants::storage::staked_keys(creator, owner),
        constants::storage::next_stake_id(creator, owner),
        constants::storage::stake_position(creator, owner, stake_id),
        constants::storage::stake_nft_id(creator, stake_id, owner),
        constants::storage::stake_nft(token_id),
    ] {
        if p.has(&key) {
            p.extend_ttl(&key, current, extend_to);
        }
    }
}

fn assign_stake_id(env: &Env, creator: &Address, owner: &Address) -> Result<u32, ContractError> {
    let key = constants::storage::next_stake_id(creator, owner);
    let stake_id: u32 = env.storage().persistent().get(&key).unwrap_or(1);
    let next = stake_id.checked_add(1).ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&key, &next);
    Ok(stake_id)
}

fn assign_token_id(env: &Env) -> Result<u64, ContractError> {
    let key = constants::storage::NEXT_STAKE_NFT_ID;
    let token_id: u64 = env.storage().persistent().get(&key).unwrap_or(1);
    let next = token_id.checked_add(1).ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&key, &next);
    Ok(token_id)
}

fn release_position(
    env: &Env,
    creator: &Address,
    owner: &Address,
    position: &StakePosition,
) -> Result<(), ContractError> {
    let mut profile = read_registered_creator_profile(env, creator)?;
    let staked = read_staked_keys(env, creator, owner);
    let remaining = staked
        .checked_sub(position.amount)
        .ok_or(ContractError::InsufficientBalance)?;
    write_staked_keys(env, creator, owner, remaining);
    let bal_key = constants::storage::holder_balance_key(creator, owner);
    let liquid: u32 = env.storage().persistent().get(&bal_key).unwrap_or(0);
    let new_liquid = liquid
        .checked_add(position.amount)
        .ok_or(ContractError::Overflow)?;
    if liquid == 0 {
        profile.holder_count = profile
            .holder_count
            .checked_add(1)
            .ok_or(ContractError::SellUnderflow)?;
    }
    env.storage().persistent().set(&bal_key, &new_liquid);
    env.storage()
        .persistent()
        .set(&constants::storage::creator(creator), &profile);
    env.storage()
        .persistent()
        .remove(&constants::storage::stake_position(
            creator,
            owner,
            position.stake_id,
        ));
    Ok(())
}

fn burn_receipt(env: &Env, creator: &Address, owner: &Address, position: &StakePosition) {
    let Some(token_id) = read_nft_id(env, creator, position.stake_id, owner) else {
        return;
    };
    release_position(env, creator, owner, position).expect("release_position");
    env.storage()
        .persistent()
        .remove(&constants::storage::stake_nft_id(
            creator,
            position.stake_id,
            owner,
        ));
    env.storage()
        .persistent()
        .remove(&constants::storage::stake_nft(token_id));
    change_nft_holder_count(env, owner, -1).expect("holder count underflow");
    change_nft_total_supply(env, -1).expect("total supply underflow");
    extend_stake_ttl(env, creator, owner, position.stake_id, token_id);
}

// --- Vault helpers ---

pub fn read_target_weights(env: &Env, creator: &Address) -> Vec<TargetWeight> {
    env.storage()
        .persistent()
        .get(&constants::storage::vault_target_weights(creator))
        .unwrap_or(Vec::new(env))
}

pub fn read_allocations(env: &Env, creator: &Address) -> Vec<VaultAllocation> {
    env.storage()
        .persistent()
        .get(&constants::storage::vault_allocations(creator))
        .unwrap_or(Vec::new(env))
}

pub fn read_key_price(env: &Env, creator: &Address, key: &Address) -> Option<i128> {
    env.storage()
        .persistent()
        .get(&constants::storage::vault_key_price(creator, key))
}

pub fn read_tolerance_bps(env: &Env, creator: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&constants::storage::vault_tolerance_bps(creator))
        .unwrap_or(DEFAULT_VAULT_TOLERANCE_BPS)
}

fn write_allocations(env: &Env, creator: &Address, allocations: &Vec<VaultAllocation>) {
    env.storage()
        .persistent()
        .set(&constants::storage::vault_allocations(creator), allocations);
}

fn validate_target_weights(weights: &Vec<TargetWeight>) -> Result<(), ContractError> {
    if weights.is_empty() || weights.len() > MAX_VAULT_KEYS {
        return Err(ContractError::InvalidTargetWeights);
    }
    let mut total_bps: u32 = 0;
    for (i, w) in weights.iter().enumerate() {
        if w.weight_bps == 0 {
            return Err(ContractError::InvalidTargetWeights);
        }
        total_bps = total_bps
            .checked_add(w.weight_bps)
            .ok_or(ContractError::TargetWeightsNotNormalized)?;
        for o in weights.iter().take(i) {
            if o.key == w.key {
                return Err(ContractError::InvalidTargetWeights);
            }
        }
    }
    if total_bps != fee::BPS_MAX {
        return Err(ContractError::TargetWeightsNotNormalized);
    }
    Ok(())
}

fn resolve_prices(
    env: &Env,
    creator: &Address,
    weights: &Vec<TargetWeight>,
) -> Result<Vec<i128>, ContractError> {
    let mut prices = Vec::new(env);
    for w in weights.iter() {
        let p = read_key_price(env, creator, &w.key).ok_or(ContractError::InvalidTargetWeights)?;
        if p <= 0 {
            return Err(ContractError::InvalidTargetWeights);
        }
        prices.push_back(p);
    }
    Ok(prices)
}

fn zero_allocations(env: &Env, weights: &Vec<TargetWeight>) -> Vec<VaultAllocation> {
    let mut a = Vec::new(env);
    for w in weights.iter() {
        a.push_back(VaultAllocation {
            key: w.key,
            units: 0,
            value: 0,
        });
    }
    a
}

fn compute_target_values(
    env: &Env,
    weights: &Vec<TargetWeight>,
    total: i128,
) -> Result<Vec<i128>, ContractError> {
    let mut t = Vec::new(env);
    for w in weights.iter() {
        let val = total
            .checked_mul(i128::from(w.weight_bps))
            .ok_or(ContractError::Overflow)?;
        t.push_back(val / i128::from(fee::BPS_MAX));
    }
    Ok(t)
}

fn value_allocations(
    env: &Env,
    weights: &Vec<TargetWeight>,
    units: &Vec<i128>,
    prices: &Vec<i128>,
) -> Result<Vec<VaultAllocation>, ContractError> {
    let mut a = Vec::new(env);
    for i in 0..weights.len() {
        let k = weights.get(i).ok_or(ContractError::InvalidTargetWeights)?;
        let u = units.get(i).unwrap_or(0);
        let p = prices.get(i).unwrap_or(0);
        a.push_back(VaultAllocation {
            key: k.key,
            units: u,
            value: u.checked_mul(p).ok_or(ContractError::Overflow)?,
        });
    }
    Ok(a)
}

fn total_value_of(a: &Vec<VaultAllocation>) -> Result<i128, ContractError> {
    let mut tot: i128 = 0;
    for x in a.iter() {
        tot = tot.checked_add(x.value).ok_or(ContractError::Overflow)?;
    }
    Ok(tot)
}

fn compute_slippage_bps(ref_p: i128, exec_p: i128) -> Result<u32, ContractError> {
    let diff = ref_p.checked_sub(exec_p).ok_or(ContractError::Overflow)?;
    let mag = if diff < 0 { -diff } else { diff };
    let scaled = mag
        .checked_mul(i128::from(fee::BPS_MAX))
        .ok_or(ContractError::Overflow)?;
    let bps = scaled.checked_div(ref_p).ok_or(ContractError::Overflow)?;
    u32::try_from(bps).map_err(|_| ContractError::Overflow)
}

fn resolve_units(
    env: &Env,
    weights: &Vec<TargetWeight>,
    stored: &Vec<VaultAllocation>,
) -> Result<Vec<i128>, ContractError> {
    let mut u = Vec::new(env);
    for i in 0..weights.len() {
        let k = weights
            .get(i)
            .ok_or(ContractError::InvalidTargetWeights)?
            .key;
        let mut uk = 0;
        for a in stored.iter() {
            if a.key == k {
                uk = a.units;
            }
        }
        u.push_back(uk);
    }
    Ok(u)
}

const NO_INDEX: i128 = -1;

fn find_nonzero(b: &Vec<i128>) -> i128 {
    for i in 0..b.len() {
        if b.get(i).unwrap_or(0) > 0 {
            return i128::from(i);
        }
    }
    NO_INDEX
}

fn min_of(l: i128, r: i128) -> i128 {
    if l < r {
        l
    } else {
        r
    }
}

fn match_sellers_to_buyers(
    env: &Env,
    weights: &Vec<TargetWeight>,
    units: &Vec<i128>,
    prices: &Vec<i128>,
    target_units: &Vec<i128>,
    max_slippage_bps: u32,
) -> Result<(Vec<events::RebalanceTrade>, Vec<i128>), ContractError> {
    let count = weights.len();
    let mut updated = units.clone();
    let mut trades = Vec::new(env);
    let mut sell = Vec::new(env);
    let mut buy = Vec::new(env);
    for i in 0..count {
        let cur = units.get(i).unwrap_or(0);
        let tgt = target_units.get(i).unwrap_or(0);
        let p = prices.get(i).unwrap_or(0);
        let d = tgt - cur;
        if d < 0 {
            sell.push_back(d.checked_mul(p).ok_or(ContractError::Overflow)?);
            buy.push_back(0);
        } else {
            sell.push_back(0);
            buy.push_back(d.checked_mul(p).ok_or(ContractError::Overflow)?);
        }
    }
    let max_p = count.saturating_mul(2).saturating_add(2);
    let mut pass = 0;
    while pass < max_p {
        let s = find_nonzero(&sell);
        if s == NO_INDEX {
            break;
        }
        let b = find_nonzero(&buy);
        if b == NO_INDEX {
            break;
        }
        let si = u32::try_from(s).map_err(|_| ContractError::Overflow)?;
        let bi = u32::try_from(b).map_err(|_| ContractError::Overflow)?;
        let sp = prices.get(si).unwrap_or(0);
        let bp = prices.get(bi).unwrap_or(0);
        let avail = min_of(sell.get(si).unwrap_or(0), buy.get(bi).unwrap_or(0));
        if avail <= 0 {
            break;
        }
        let slp = compute_slippage_bps(sp, bp)?;
        if slp > max_slippage_bps {
            return Err(ContractError::SlippageExceeded);
        }
        let uo = (avail / sp).min(-sell.get(si).unwrap_or(0) / sp);
        let ui = (avail / bp).min(buy.get(bi).unwrap_or(0) / bp);
        if uo <= 0 || ui <= 0 {
            sell.set(si, 0);
            buy.set(bi, 0);
            pass += 1;
            continue;
        }
        let vo = uo.checked_mul(sp).ok_or(ContractError::Overflow)?;
        let vi = ui.checked_mul(bp).ok_or(ContractError::Overflow)?;
        let mv = min_of(vo, vi);
        let nu = updated
            .get(si)
            .unwrap_or(0)
            .checked_add(uo)
            .ok_or(ContractError::Overflow)?;
        updated.set(si, nu);
        let nu = updated
            .get(bi)
            .unwrap_or(0)
            .checked_add(ui)
            .ok_or(ContractError::Overflow)?;
        updated.set(bi, nu);
        sell.set(si, sell.get(si).unwrap_or(0) - mv);
        buy.set(bi, buy.get(bi).unwrap_or(0) - mv);
        trades.push_back(events::RebalanceTrade {
            from_key: weights
                .get(si)
                .ok_or(ContractError::InvalidTargetWeights)?
                .key,
            to_key: weights
                .get(bi)
                .ok_or(ContractError::InvalidTargetWeights)?
                .key,
            amount: mv,
            reference_price: sp,
            execution_price: bp,
            slippage_bps: slp,
        });
        pass += 1;
    }
    Ok((trades, updated))
}

// --- Dynamic fee helpers ---

pub fn read_fee_tiers(env: &Env) -> Vec<FeeTier> {
    env.storage()
        .persistent()
        .get(&constants::storage::FEE_TIERS)
        .unwrap_or(Vec::new(env))
}

fn read_volume_buckets(env: &Env) -> Vec<VolumeBucket> {
    env.storage()
        .persistent()
        .get(&constants::storage::FEE_VOLUME_BUCKETS)
        .unwrap_or(Vec::new(env))
}

fn bucket_start_for(seq: u32) -> u32 {
    seq - (seq % VOLUME_BUCKET_LEDGERS)
}

fn validate_fee_tiers(t: &Vec<FeeTier>) -> Result<(), ContractError> {
    if t.is_empty() {
        return Err(ContractError::InvalidTargetWeights);
    }
    if t.len() > MAX_FEE_TIERS {
        return Err(ContractError::DiscountTierLimitExceeded);
    }
    let mut prev: Option<i128> = None;
    for tier in t.iter() {
        if tier.protocol_bps > fee::BPS_MAX {
            return Err(ContractError::InvalidTargetWeights);
        }
        if let Some(p) = prev {
            if tier.volume_threshold <= p {
                return Err(ContractError::InvalidTargetWeights);
            }
        }
        prev = Some(tier.volume_threshold);
    }
    if t.get(0).map(|x| x.volume_threshold) != Some(0) {
        return Err(ContractError::InvalidTargetWeights);
    }
    Ok(())
}

pub fn rolling_volume(env: &Env) -> i128 {
    let cutoff = env
        .ledger()
        .sequence()
        .saturating_sub(ROLLING_WINDOW_LEDGERS);
    let mut tot: i128 = 0;
    for b in read_volume_buckets(env).iter() {
        if b.bucket_start >= cutoff {
            tot = tot.saturating_add(b.volume);
        }
    }
    tot
}

fn active_tier_index(tiers: &Vec<FeeTier>, vol: i128) -> u32 {
    let mut idx: u32 = 0;
    for (pos, tier) in tiers.iter().enumerate() {
        if vol >= tier.volume_threshold {
            idx = pos as u32;
        } else {
            break;
        }
    }
    idx
}

pub fn current_protocol_bps(env: &Env) -> u32 {
    let tiers = read_fee_tiers(env);
    let flat = read_protocol_fee_config(env).map(|c| c.protocol_bps);
    if tiers.is_empty() {
        return flat.unwrap_or(0);
    }
    let idx = active_tier_index(&tiers, rolling_volume(env));
    tiers
        .get(idx)
        .map(|t| t.protocol_bps)
        .unwrap_or_else(|| flat.unwrap_or(0))
}

fn effective_fee_config(env: &Env) -> Option<fee::FeeConfig> {
    let stored = read_protocol_fee_config(env)?;
    if read_fee_tiers(env).is_empty() {
        return Some(stored);
    }
    Some(fee::FeeConfig {
        creator_bps: fee::BPS_MAX - current_protocol_bps(env),
        protocol_bps: current_protocol_bps(env),
    })
}

fn record_volume(env: &Env, vol: i128) -> Result<(), ContractError> {
    if vol <= 0 {
        return Ok(());
    }
    let seq = env.ledger().sequence();
    let bs = bucket_start_for(seq);
    let cutoff = seq.saturating_sub(ROLLING_WINDOW_LEDGERS);
    let buckets = read_volume_buckets(env);
    let mut upd = Vec::new(env);
    let mut found = false;
    for b in buckets.iter() {
        if b.bucket_start == bs {
            upd.push_back(VolumeBucket {
                bucket_start: bs,
                volume: b.volume.checked_add(vol).ok_or(ContractError::Overflow)?,
            });
            found = true;
        } else if b.bucket_start > cutoff {
            upd.push_back(b);
        }
    }
    if !found {
        upd.push_back(VolumeBucket {
            bucket_start: bs,
            volume: vol,
        });
    }
    env.storage()
        .persistent()
        .set(&constants::storage::FEE_VOLUME_BUCKETS, &upd);
    Ok(())
}

pub fn apply_trade_volume(env: &Env, vol: i128) -> Result<u32, ContractError> {
    let tiers = read_fee_tiers(env);
    if tiers.is_empty() {
        return Ok(read_protocol_fee_config(env)
            .map(|c| c.protocol_bps)
            .unwrap_or(0));
    }
    let prev_idx: u32 = env
        .storage()
        .persistent()
        .get(&constants::storage::ACTIVE_FEE_TIER_INDEX)
        .unwrap_or(events::NO_FEE_TIER_INDEX);
    let prev_bps = tiers.get(prev_idx).map(|t| t.protocol_bps).unwrap_or(0);
    record_volume(env, vol)?;
    let new_idx = active_tier_index(&tiers, rolling_volume(env));
    let new_bps = tiers.get(new_idx).map(|t| t.protocol_bps).unwrap_or(0);
    if new_idx != prev_idx {
        env.storage()
            .persistent()
            .set(&constants::storage::ACTIVE_FEE_TIER_INDEX, &new_idx);
        env.events().publish(
            (events::FEE_TIER_CHANGED_EVENT_NAME,),
            events::FeeTierChangedEvent {
                old_tier_index: prev_idx,
                new_tier_index: new_idx,
                old_protocol_bps: prev_bps,
                new_protocol_bps: new_bps,
                ledger: env.ledger().sequence(),
            },
        );
    }
    Ok(new_bps)
}

// --- Curve reset helpers ---

pub fn read_creator_slope(env: &Env, creator: &Address) -> Option<i128> {
    env.storage()
        .persistent()
        .get(&constants::storage::creator_curve_slope(creator))
}

pub fn read_reset_count(env: &Env, creator: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&constants::storage::curve_reset_count(creator))
        .unwrap_or(0)
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

    // --- read_creator_supply helper tests (#587) ---

    #[test]
    fn test_read_creator_supply_returns_correct_supply_for_initialized_creator() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let creator = Address::generate(&env);
        let contract_id = env.register(super::CreatorKeysContract, ());

        let profile = super::CreatorProfile {
            creator: creator.clone(),
            handle: soroban_sdk::String::from_str(&env, "alice"),
            supply: 42,
            holder_count: 5,
            fee_recipient: creator.clone(),
            registered_at: 0,
        };

        let supply = env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&super::constants::storage::creator(&creator), &profile);

            super::read_creator_supply(&env, &creator)
        });
        assert_eq!(supply, 42);
    }

    #[test]
    fn test_read_creator_supply_returns_zero_for_uninitialized_creator() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let missing_creator = Address::generate(&env);
        let contract_id = env.register(super::CreatorKeysContract, ());

        let supply = env.as_contract(&contract_id, || {
            super::read_creator_supply(&env, &missing_creator)
        });
        assert_eq!(supply, 0);
    }

    #[test]
    fn test_read_creator_supply_matches_read_key_balance() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let creator = Address::generate(&env);
        let contract_id = env.register(super::CreatorKeysContract, ());

        let profile = super::CreatorProfile {
            creator: creator.clone(),
            handle: soroban_sdk::String::from_str(&env, "alice"),
            supply: 15,
            holder_count: 3,
            fee_recipient: creator.clone(),
            registered_at: 0,
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&super::constants::storage::creator(&creator), &profile);

            let supply_from_new = super::read_creator_supply(&env, &creator);
            let supply_from_old = super::read_key_balance(&env, &creator);
            assert_eq!(supply_from_new, supply_from_old);
            assert_eq!(supply_from_new, 15);
        });
    }

    // --- write_creator_supply helper tests (#596) ---

    #[test]
    fn test_write_creator_supply_read_after_write_returns_written_value() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let creator = Address::generate(&env);
        let contract_id = env.register(super::CreatorKeysContract, ());

        let profile = super::CreatorProfile {
            creator: creator.clone(),
            handle: soroban_sdk::String::from_str(&env, "alice"),
            supply: 0,
            holder_count: 0,
            fee_recipient: creator.clone(),
            registered_at: 0,
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&super::constants::storage::creator(&creator), &profile);

            super::write_creator_supply(&env, &creator, 42);
            let result = super::read_creator_supply(&env, &creator);
            assert_eq!(result, 42);
        });
    }

    #[test]
    fn test_write_creator_supply_overwrite_replaces_previous_value() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let creator = Address::generate(&env);
        let contract_id = env.register(super::CreatorKeysContract, ());

        let profile = super::CreatorProfile {
            creator: creator.clone(),
            handle: soroban_sdk::String::from_str(&env, "alice"),
            supply: 10,
            holder_count: 0,
            fee_recipient: creator.clone(),
            registered_at: 0,
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&super::constants::storage::creator(&creator), &profile);

            super::write_creator_supply(&env, &creator, 25);
            assert_eq!(super::read_creator_supply(&env, &creator), 25);

            super::write_creator_supply(&env, &creator, 50);
            assert_eq!(super::read_creator_supply(&env, &creator), 50);
        });
    }

    #[test]
    fn test_write_creator_supply_preserves_other_profile_fields() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let creator = Address::generate(&env);
        let contract_id = env.register(super::CreatorKeysContract, ());

        let profile = super::CreatorProfile {
            creator: creator.clone(),
            handle: soroban_sdk::String::from_str(&env, "alice"),
            supply: 10,
            holder_count: 5,
            fee_recipient: creator.clone(),
            registered_at: 42,
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&super::constants::storage::creator(&creator), &profile);

            super::write_creator_supply(&env, &creator, 20);

            let updated: super::CreatorProfile = env
                .storage()
                .persistent()
                .get(&super::constants::storage::creator(&creator))
                .unwrap();

            assert_eq!(updated.supply, 20, "supply should be updated");
            assert_eq!(updated.holder_count, 5, "holder_count should be preserved");
            assert_eq!(
                updated.handle,
                soroban_sdk::String::from_str(&env, "alice"),
                "handle should be preserved"
            );
            assert_eq!(
                updated.registered_at, 42,
                "registered_at should be preserved"
            );
        });
    }
}

// #[cfg(test)]
// mod test_issues;
