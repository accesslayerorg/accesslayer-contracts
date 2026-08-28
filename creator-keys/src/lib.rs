#![no_std]
pub mod quote_view_errors;

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String, Vec};

pub mod events;
pub mod test_new_features;

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
    WalletBlacklisted = 37,
    SchemaVersionTooOld = 38,
    SchemaVersionUnsupported = 39,
    DisplayNameEmpty = 40,
    DeadlinePassed = 41,
    CapAlreadySet = 42,
    MultisigAdminLimitExceeded = 43,
    AlreadyApproved = 44,
    ProposalNotFound = 45,
    VestingNotFound = 46,
    VestingNotStarted = 47,
    NothingToClaim = 48,
    NotWhitelisted = 49,
    CircuitBreakerTriggered = 50,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
/// Error variants for the co-creator removal, pre-launch auction, and
/// staking reward claim entrypoints.
///
/// These live in a separate error type from [`ContractError`] rather than as
/// additional variants there: the Soroban contract spec format caps a single
/// `#[contracterror]` enum at 50 cases (`SCSpecUDTErrorEnumV0.cases<50>`),
/// and `ContractError` is already at that limit.
pub enum FeatureError {
    Unauthorized = 1,
    NotRegistered = 2,
    Overflow = 3,
    ProtocolPaused = 4,
    NotPositiveAmount = 5,
    NoCoCreatorSet = 6,
    AuctionAlreadyStarted = 7,
    NoAuctionConfigured = 8,
    InvalidAuctionConfig = 9,
    StakeLockActive = 10,
    NoStakeFound = 11,
    /// The batch buy call exceeds the maximum allowed order count.
    BatchSizeExceeded = 13,
    /// Creator royalty fee exceeds the allowed maximum.
    RoyaltyExceedsLimit = 14,
    /// The bonding curve exponent is outside the allowed range.
    InvalidExponent = 15,
    /// Holder exceeds the per-creator percentage holding cap.
    MaxHoldingExceeded = 16,
    /// Sell rejected because the holder's lockup period is still active.
    LockupPeriodActive = 17,
    /// Holder cap value is outside the allowed range.
    InvalidHolderCap = 18,
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
    pub const PROTOCOL_BPS_MAX: u32 = 10_000;

    #[derive(Clone, Eq, PartialEq)]
    #[contracttype]
    pub struct FeeConfig {
        pub creator_bps: u32,
        pub protocol_bps: u32,
    }

    /// Validates creator and protocol basis points for storage and fee-setting entrypoints.
    pub fn validate_fee_bps(creator_bps: u32, protocol_bps: u32) -> bool {
        if protocol_bps > PROTOCOL_BPS_MAX {
            return false;
        }
        let Some(sum) = creator_bps.checked_add(protocol_bps) else {
            return false;
        };
        if sum == 0 || sum > BPS_MAX {
            return false;
        }
        true
    }

    /// Shared guard for fee config updates that need structured contract errors.
    pub fn assert_valid_fee_bps(creator_bps: u32, protocol_bps: u32) -> Result<(), ContractError> {
        if protocol_bps > PROTOCOL_BPS_MAX {
            return Err(ContractError::ProtocolFeeExceedsCap);
        }
        let Some(sum) = creator_bps.checked_add(protocol_bps) else {
            return Err(ContractError::InvalidFeeConfig);
        };
        if sum == 0 || sum > BPS_MAX {
            return Err(ContractError::InvalidFeeConfig);
        }
        Ok(())
    }

    /// Computes the fee split for a given total amount.
    ///
    /// Returns `(creator_amount, protocol_amount)`. When creator_bps + protocol_bps == BPS_MAX,
    /// remainder from integer division is assigned to the creator so creator_amount + protocol_amount == total.
    /// Otherwise, each fee is computed independently via basis points.
    pub fn compute_fee_split(total: i128, creator_bps: u32, protocol_bps: u32) -> (i128, i128) {
        if total <= 0 {
            return (0, 0);
        }
        let protocol_amount = (total * protocol_bps as i128) / BPS_MAX as i128;
        let creator_amount = if creator_bps.saturating_add(protocol_bps) == BPS_MAX {
            total - protocol_amount
        } else {
            (total * creator_bps as i128) / BPS_MAX as i128
        };
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
        creator_bps: u32,
        protocol_bps: u32,
    ) -> Option<(i128, i128)> {
        if total <= 0 {
            return Some((0, 0));
        }
        let protocol_amount = apply_percentage_fee(total, protocol_bps)?;
        let creator_amount = if creator_bps.checked_add(protocol_bps) == Some(BPS_MAX) {
            checked_sub_i128(total, protocol_amount)?
        } else {
            apply_percentage_fee(total, creator_bps)?
        };
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
        pub const RETENTION_POLICY: DataKey = DataKey::RetentionPolicy;
        pub const GLOBAL_DEADLINE_LEDGER: DataKey = DataKey::GlobalDeadlineLedger;
        pub const PROTOCOL_FEE_BPS: DataKey = DataKey::ProtocolFeeBps;
        pub const LOCKUP_DURATION_SECS: DataKey = DataKey::LockupDurationSecs;

        /// Protocol-wide emergency trading halt flag (#784).
        pub const GLOBAL_TRADING_PAUSED: DataKey = DataKey::GlobalTradingPaused;
        /// The 2-of-3 admin set authorised to trigger the global emergency pause.
        pub const GLOBAL_PAUSE_ADMINS: DataKey = DataKey::GlobalPauseAdmins;

        /// Storage key for a pending `global_pause` vote by `admin`.
        pub fn global_pause_vote(admin: &Address) -> DataKey {
            DataKey::GlobalPauseVote(admin.clone())
        }

        /// Storage key for a pending `global_resume` vote by `admin`.
        pub fn global_resume_vote(admin: &Address) -> DataKey {
            DataKey::GlobalResumeVote(admin.clone())
        }

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

        pub fn blacklisted(wallet: &Address) -> DataKey {
            DataKey::Blacklisted(wallet.clone())
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

        pub fn staked_balance(creator: &Address, holder: &Address) -> DataKey {
            DataKey::StakedBalance(creator.clone(), holder.clone())
        }

        pub fn key_balance(creator: &Address, holder: &Address) -> DataKey {
            key_balance_key(creator, holder)
        }

        pub fn max_keys_per_wallet(creator: &Address) -> DataKey {
            DataKey::MaxKeysPerWallet(creator.clone())
        }

        pub fn referral_fee_bps() -> DataKey {
            DataKey::ReferralFeeBps
        }

        pub fn royalty_config(creator: &Address) -> DataKey {
            DataKey::CreatorFeatureData(0, creator.clone(), creator.clone())
        }

        pub fn curve_exponent(creator: &Address) -> DataKey {
            DataKey::CreatorFeatureData(1, creator.clone(), creator.clone())
        }

        /// Absolute live-until ledger the contract last set for `creator`'s
        /// profile key, used to decide whether to emit the TTL-extension event.
        pub fn creator_ttl_live_until(creator: &Address) -> DataKey {
            DataKey::CreatorTtlLiveUntil(creator.clone())
        }

        pub fn multisig_admins(creator: &Address) -> DataKey {
            DataKey::MultisigAdmins(creator.clone())
        }

        pub fn pause_proposal(creator: &Address, admin: &Address) -> DataKey {
            DataKey::PauseProposal(creator.clone(), admin.clone())
        }

        pub fn vesting_schedule(creator: &Address, beneficiary: &Address) -> DataKey {
            DataKey::VestingSchedule(creator.clone(), beneficiary.clone())
        }

        pub const CIRCUIT_BREAKER_THRESHOLD: DataKey = DataKey::CircuitBreakerThreshold;

        pub fn referral_earnings(referrer: &Address) -> DataKey {
            DataKey::ReferralEarnings(referrer.clone())
        }

        pub fn whitelist_entry(key_id: &Address, wallet: &Address) -> DataKey {
            DataKey::WhitelistMap(key_id.clone(), wallet.clone())
        }

        pub fn whitelist_mode(key_id: &Address) -> DataKey {
            DataKey::WhitelistMode(key_id.clone())
        }

        pub fn vesting_claimed(creator: &Address, beneficiary: &Address) -> DataKey {
            DataKey::VestingClaimed(creator.clone(), beneficiary.clone())
        }

        pub fn auction_config(creator: &Address) -> DataKey {
            DataKey::CreatorFeatureData(2, creator.clone(), creator.clone())
        }

        pub fn stake_unlock_ledger(creator: &Address, holder: &Address) -> DataKey {
            DataKey::CreatorFeatureData(6, creator.clone(), holder.clone())
        }

        pub fn total_staked(creator: &Address) -> DataKey {
            DataKey::CreatorFeatureData(3, creator.clone(), creator.clone())
        }

        pub fn staking_rewards_pool(creator: &Address) -> DataKey {
            DataKey::CreatorFeatureData(4, creator.clone(), creator.clone())
        }

        pub fn holder_cap_bps(creator: &Address) -> DataKey {
            DataKey::CreatorFeatureData(5, creator.clone(), creator.clone())
        }

        pub fn last_buy_timestamp(creator: &Address, holder: &Address) -> DataKey {
            DataKey::LastBuyTimestamp(creator.clone(), holder.clone())
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

/// Minimum remaining TTL (in ledgers) that triggers a TTL extension event.
///
/// When the creator key's remaining TTL drops strictly below this threshold,
/// the next trade will emit a [`events::TTL_EXTENDED_EVENT_NAME`] event.
/// When the remaining TTL is at or above this value, the extension is still
/// performed (via Soroban's `extend_ttl` SDK call, which is a no-op when the
/// entry already has a healthy expiration), but no event is emitted.
pub const TTL_EXTENSION_THRESHOLD: u32 = 100;

/// TTL (time-to-live) extension decision logic.
///
/// Storage TTL extension should only fire when the remaining TTL drops below
/// a configured minimum threshold. This pure helper isolates that decision so
/// it can be unit tested independently of Soroban's storage TTL model.
pub mod ttl {
    /// Returns `true` when `current_ttl` (ledgers remaining) is strictly below
    /// `threshold`, meaning a TTL extension should be triggered.
    ///
    /// The check is exclusive at the boundary: a TTL exactly at `threshold`
    /// does not trigger an extension.
    pub fn should_extend(current_ttl: u32, threshold: u32) -> bool {
        current_ttl < threshold
    }
}

/// Minimum TTL extension (in ledgers) applied to persistent storage entries.
///
/// Roughly 30 days at 5 seconds per ledger. [`bump_persistent_ttl`] guarantees
/// that every entry it touches keeps at least this much remaining lifetime, so
/// actively read or written state never expires unexpectedly.
pub const TTL_MIN_EXTENSION_LEDGERS: u32 = 518_400;

/// Default protocol trade fee in basis points (100 = 1%).
///
/// Applied to every buy and sell once the trade fee is configured via
/// `set_protocol_fee`; the admin can override it with an explicit value.
pub const DEFAULT_PROTOCOL_FEE_BPS: u32 = 100;

/// Default per-holder holding cap in basis points (1000 = 10% of supply).
///
/// Applied when a creator enables the holding cap via `set_holder_cap` without
/// requesting a custom percentage; explicit values must fall between
/// [`HOLDER_CAP_MIN_BPS`] and [`HOLDER_CAP_MAX_BPS`].
pub const DEFAULT_HOLDER_CAP_BPS: u32 = 1000;

/// Minimum configurable holding cap in basis points (1%).
pub const HOLDER_CAP_MIN_BPS: u32 = 100;

/// Maximum configurable holding cap in basis points (25%).
pub const HOLDER_CAP_MAX_BPS: u32 = 2500;

/// Default sell lockup duration in seconds (24 hours).
///
/// Enforced once configured via `set_lockup_duration`: a holder cannot sell
/// keys until at least this much time has elapsed since their most recent buy.
pub const DEFAULT_LOCKUP_DURATION_SECS: u64 = 86_400;

/// Current client-facing schema version of this contract.
///
/// Increment this constant whenever the contract's ABI or on-chain data layout
/// changes in a way that is incompatible with older clients.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Minimum schema version that clients must present.
///
/// Calls that supply a version strictly below this value are rejected with
/// [`ContractError::SchemaVersionTooOld`] so stale clients are forced to
/// upgrade before interacting with the contract.
pub const MIN_SCHEMA_VERSION: u32 = 1;

/// Schema version compatibility guard.
///
/// Pure function — no Soroban `Env` required — so it is easily unit-testable
/// and reusable across any entrypoint that wants version gating.
///
/// # Rules
/// - `client_version == 0`            → `SchemaVersionTooOld`
/// - `client_version < MIN_SCHEMA_VERSION` → `SchemaVersionTooOld`
/// - `client_version > CURRENT_SCHEMA_VERSION` → `SchemaVersionUnsupported`
/// - otherwise                        → `Ok(())`
pub fn assert_schema_version(client_version: u32) -> Result<(), ContractError> {
    if client_version == 0 || client_version < MIN_SCHEMA_VERSION {
        return Err(ContractError::SchemaVersionTooOld);
    }
    if client_version > CURRENT_SCHEMA_VERSION {
        return Err(ContractError::SchemaVersionUnsupported);
    }
    Ok(())
}

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

/// Maximum number of entries in a single batch buy call.
pub const MAX_BATCH_BUY_SIZE: usize = 5;

/// Maximum royalty fee basis points (5%).
pub const MAX_ROYALTY_BPS: u32 = 500;

/// Maximum number of keys a pre-launch auction can allocate at the fixed
/// auction price before the bonding curve takes over.
pub const MAX_AUCTION_SUPPLY: u32 = 10_000;

/// Lock duration for staked keys before a reward claim is permitted (30 days
/// at 5s per ledger).
pub const STAKE_LOCK_LEDGERS: u32 = 518_400;

/// Share of each protocol fee collection routed into a creator's staking
/// rewards pool (10%), on top of the existing treasury/recipient split.
pub const STAKING_REWARD_SHARE_BPS: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum CurvePreset {
    Linear = 0,
    Quadratic = 1,
    Flat = 2,
}

/// Archive partition strategy for retention management.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum PartitionStrategy {
    Daily = 0,
    Weekly = 1,
    Monthly = 2,
    Ledger = 3,
}

/// Archive retention policy configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RetentionPolicy {
    pub retention_days: u32,
    pub partition_strategy: PartitionStrategy,
    pub compression_enabled: bool,
    pub batch_size: u32,
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
    StakedBalance(Address, Address), // (creator, holder) -> staked amount
    MaxKeysPerWallet(Address),
    ReferralFeeBps,
    DiscountTiers,
    CreatorVolume(Address),
    /// Absolute live-until ledger the contract last set for the creator key
    /// via `extend_ttl`. Tracks the TTL extension state so the contract can
    /// decide whether to emit the TTL-extension event without a TTL read
    /// (the Soroban SDK does not expose TTL reads to contract code).
    CreatorTtlLiveUntil(Address),
    /// Wallet addresses the protocol admin has barred from buying, selling,
    /// or registering as a creator.
    Blacklisted(Address),
    /// Archive retention policy configuration.
    RetentionPolicy,
    /// Protocol-wide ledger sequence at (and after) which buys are rejected.
    /// Absent means no deadline is configured and buys are never time-gated.
    GlobalDeadlineLedger,
    MultisigAdmins(Address),
    PauseProposal(Address, Address),
    VestingSchedule(Address, Address),
    VestingClaimed(Address, Address),
    TimelockProposal(u32),
    TimelockNextId,
    VoteSnapshot(Address, u32, Address),
    CircuitBreakerThreshold,
    ReferralEarnings(Address),
    WhitelistMap(Address, Address),
    WhitelistMode(Address),
    /// Protocol-wide emergency trading halt flag (#784). When `true`, every
    /// buy and sell is rejected regardless of per-key pause state.
    GlobalTradingPaused,
    /// The 2-of-3 admin set authorised to trigger the global emergency pause.
    GlobalPauseAdmins,
    /// A pending `global_pause` vote cast by the given admin.
    GlobalPauseVote(Address),
    /// A pending `global_resume` vote cast by the given admin.
    GlobalResumeVote(Address),
    /// Generic compound key for per-creator feature storage.
    /// Encodes (feature_type, creator, secondary_addr) where feature_type
    /// distinguishes royalty_config(0), curve_exponent(1), auction_config(2),
    /// total_staked(3), staking_rewards_pool(4), holder_cap_bps(5),
    /// stake_unlock_ledger(6).
    CreatorFeatureData(u32, Address, Address),
    /// Ledger timestamp of a holder's most recent buy for a creator.
    LastBuyTimestamp(Address, Address),
    /// Protocol trade fee in basis points.
    ProtocolFeeBps,
    /// Sell lockup duration in seconds.
    LockupDurationSecs,
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

/// Vesting schedule for linear key release over a fixed period.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct VestingSchedule {
    pub beneficiary: Address,
    pub total_keys: u32,
    pub start_ledger: u32,
    pub vesting_period_ledgers: u32,
    pub claimed_keys: u32,
}

/// Supported timelock change types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum TimelockChangeType {
    Fee = 0,
    CurveExponent = 1,
    Treasury = 2,
}

/// A timelocked config change proposal.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct TimelockProposal {
    pub change_type: TimelockChangeType,
    pub payload: soroban_sdk::Bytes,
    pub proposer: Address,
    pub proposed_at: u32,
    pub execution_not_before: u32,
    pub executed: bool,
    pub cancelled: bool,
}

/// Multisig admin configuration for pause proposals.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct MultisigAdmins {
    pub admins: Vec<Address>,
}

/// A pause proposal initiated by one admin, awaiting a second approval.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PauseProposal {
    pub proposer: Address,
    pub approved: bool,
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

/// Creator royalty configuration for buy and sell fees.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RoyaltyConfig {
    pub buy_fee_bps: u32,
    pub sell_fee_bps: u32,
}

/// Result of a single order in a batch buy.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BatchBuyOrderResult {
    pub creator: Address,
    pub quantity: u32,
    pub price_paid: i128,
}

/// Pre-launch fixed-price auction configuration for a creator.
///
/// When configured, the first `auction_supply` keys are sold at `auction_price`
/// instead of the bonding curve price. The contract transitions back to the
/// curve automatically once the auction supply is exhausted.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AuctionConfig {
    pub auction_price: i128,
    pub auction_supply: u32,
    pub auction_sold: u32,
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
    let mode_key = constants::storage::whitelist_mode(&profile.creator);
    let is_mode_on: bool = env.storage().persistent().get(&mode_key).unwrap_or(false);
    if is_mode_on {
        let entry_key = constants::storage::whitelist_entry(&profile.creator, buyer);
        let is_approved: bool = env.storage().persistent().get(&entry_key).unwrap_or(false);
        if !is_approved {
            return Err(ContractError::NotWhitelisted);
        }
        return Ok(());
    }

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

/// Reads the fee recipient address for a creator from their profile.
///
/// Returns `None` when the creator has not been registered (no profile exists).
/// Returns `Some(recipient)` when the creator is registered, defaulting to the
/// creator's own address if the profile was created without an explicit recipient.
pub fn read_creator_fee_recipient(env: &Env, creator: &Address) -> Option<Address> {
    read_creator_profile(env, creator).map(|p| p.fee_recipient)
}

/// Updates the fee recipient address stored in a creator's profile.
///
/// # Panics
///
/// Panics if the creator profile does not exist in storage. Callers must
/// verify creator registration before invoking this helper.
pub fn write_creator_fee_recipient(env: &Env, creator: &Address, recipient: &Address) {
    let key = constants::storage::creator(creator);
    let mut profile: CreatorProfile = env
        .storage()
        .persistent()
        .get(&key)
        .expect("write_creator_fee_recipient: creator profile not found");
    profile.fee_recipient = recipient.clone();
    env.storage().persistent().set(&key, &profile);
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
    extend_key_ttl_to_full_window(env, &key);
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
    extend_key_ttl_to_full_window(env, &key);
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

/// Validates a creator's display handle.
///
/// A blank handle — empty, or nothing but ASCII whitespace — is reported as
/// [`ContractError::DisplayNameEmpty`] ahead of the length and character rules,
/// so a caller that simply omitted the field gets that back rather than the
/// generic "too short". The over-length check runs first because the handle
/// bytes are read into a fixed `HANDLE_LEN_MAX` buffer.
fn validate_creator_handle(handle: &String) -> Result<(), ContractError> {
    let len = handle.len();
    if len > HANDLE_LEN_MAX {
        return Err(ContractError::HandleTooLong);
    }

    let mut bytes = [0u8; HANDLE_LEN_MAX as usize];
    handle.copy_into_slice(&mut bytes[..len as usize]);
    let handle_bytes = &bytes[..len as usize];

    // An empty slice satisfies `all`, so this covers the empty-string case too.
    if handle_bytes.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(ContractError::DisplayNameEmpty);
    }
    if len < HANDLE_LEN_MIN {
        return Err(ContractError::HandleTooShort);
    }
    if handle_bytes.iter().any(|byte| !is_valid_handle_byte(*byte)) {
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

/// Number of distinct admin approvals required to toggle the global pause (#784).
const GLOBAL_PAUSE_THRESHOLD: u32 = 2;

/// Which side of the global-pause multisig a vote belongs to.
#[derive(Clone, Copy, PartialEq)]
enum GlobalVoteKind {
    Pause,
    Resume,
}

/// Read-only: whether the protocol-wide emergency trading halt is active (#784).
fn is_global_trading_paused(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get::<DataKey, bool>(&constants::storage::GLOBAL_TRADING_PAUSED)
        .unwrap_or(false)
}

/// Rejects buy/sell while the global emergency pause is active. Checked before
/// the per-key pause guard so a global halt always takes precedence.
fn assert_global_trading_not_halted(env: &Env) -> Result<(), ContractError> {
    if is_global_trading_paused(env) {
        return Err(ContractError::ProtocolPaused);
    }
    Ok(())
}

/// Loads the configured global-pause admin set, or `Unauthorized` if unset.
fn read_global_pause_admins(env: &Env) -> Result<MultisigAdmins, ContractError> {
    env.storage()
        .persistent()
        .get(&constants::storage::GLOBAL_PAUSE_ADMINS)
        .ok_or(ContractError::Unauthorized)
}

/// Asserts `caller` is a member of the global-pause admin set.
fn assert_global_pause_admin(
    config: &MultisigAdmins,
    caller: &Address,
) -> Result<(), ContractError> {
    for admin in config.admins.iter() {
        if admin == *caller {
            return Ok(());
        }
    }
    Err(ContractError::Unauthorized)
}

/// Counts how many distinct configured admins currently have a `kind` vote recorded.
fn count_global_votes(env: &Env, config: &MultisigAdmins, kind: GlobalVoteKind) -> u32 {
    let mut count = 0u32;
    for admin in config.admins.iter() {
        let key = match kind {
            GlobalVoteKind::Pause => constants::storage::global_pause_vote(&admin),
            GlobalVoteKind::Resume => constants::storage::global_resume_vote(&admin),
        };
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&key)
            .unwrap_or(false)
        {
            count += 1;
        }
    }
    count
}

/// Clears every pending pause and resume vote for the configured admin set.
///
/// Called after each successful toggle so a subsequent action starts from a
/// clean slate and stale votes can never carry over.
fn clear_global_votes(env: &Env, config: &MultisigAdmins) {
    for admin in config.admins.iter() {
        env.storage()
            .persistent()
            .remove(&constants::storage::global_pause_vote(&admin));
        env.storage()
            .persistent()
            .remove(&constants::storage::global_resume_vote(&admin));
    }
}

fn is_blacklisted(env: &Env, wallet: &Address) -> bool {
    env.storage()
        .persistent()
        .get::<DataKey, bool>(&constants::storage::blacklisted(wallet))
        .unwrap_or(false)
}

fn assert_not_blacklisted(env: &Env, wallet: &Address) -> Result<(), ContractError> {
    if is_blacklisted(env, wallet) {
        return Err(ContractError::WalletBlacklisted);
    }
    Ok(())
}

/// Reads the protocol-wide buy deadline ledger, if one has been configured.
fn read_global_deadline(env: &Env) -> Option<u32> {
    env.storage()
        .persistent()
        .get::<DataKey, u32>(&constants::storage::GLOBAL_DEADLINE_LEDGER)
}

/// Rejects the call once the configured global deadline ledger has been reached.
///
/// The deadline is exclusive: the last ledger on which a buy is accepted is
/// `deadline - 1`, so a buy submitted *at* the deadline is already too late.
/// With no deadline configured the check is a no-op.
fn assert_before_global_deadline(env: &Env) -> Result<(), ContractError> {
    if let Some(deadline) = read_global_deadline(env) {
        if env.ledger().sequence() >= deadline {
            return Err(ContractError::DeadlinePassed);
        }
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

/// Reads the protocol fee basis points from storage, panicking if uninitialized.
///
/// # Panics
///
/// Panics with a descriptive message if called before contract initialization
/// (when no fee configuration has been stored).
pub fn read_protocol_fee_bps(env: &Env) -> u32 {
    read_protocol_fee_config(env)
        .expect("read_protocol_fee_bps: contract is uninitialized (protocol_fee_bps not set)")
        .protocol_bps
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
    extend_key_ttl_to_full_window(env, &constants::storage::PROTOCOL_FEE_RECIPIENT_BALANCE);
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
    extend_key_ttl_to_full_window(env, &constants::storage::TREASURY_BALANCE);
    Ok(())
}

/// Reads the protocol trade fee configuration set via `set_protocol_fee`.
///
/// Returns `None` until both the fee rate and the treasury address have been
/// configured, so the trade fee stays dormant on deployments that never opt in.
fn read_trade_fee_config(env: &Env) -> Option<(u32, Address)> {
    let fee_bps: u32 = env
        .storage()
        .persistent()
        .get(&constants::storage::PROTOCOL_FEE_BPS)?;
    let treasury: Address = env
        .storage()
        .persistent()
        .get(&constants::storage::TREASURY_ADDRESS)?;
    Some((fee_bps, treasury))
}

/// Pure computation of the protocol trade fee owed on `amount`.
///
/// Returns `0` when the trade fee is not configured, the rate is zero, or the
/// amount is non-positive. Mirrors the deduction performed by
/// [`collect_protocol_trade_fee`] so slippage and quote math stay consistent.
fn compute_trade_fee(env: &Env, amount: i128) -> Result<i128, ContractError> {
    let Some((fee_bps, _)) = read_trade_fee_config(env) else {
        return Ok(0);
    };
    if fee_bps == 0 {
        return Ok(0);
    }
    fee::apply_percentage_fee(amount, fee_bps).ok_or(ContractError::Overflow)
}

/// Deducts the protocol trade fee from `amount`, credits it to the protocol
/// treasury balance and emits a [`events::FEE_COLLECTED_EVENT_NAME`] event.
///
/// Returns the net remainder that flows into the creator/seller payout math.
/// With a rate of 0 bps no treasury credit or event is produced and the full
/// amount is returned unchanged.
fn collect_protocol_trade_fee(env: &Env, amount: i128) -> Result<i128, ContractError> {
    let trade_fee = compute_trade_fee(env, amount)?;
    if trade_fee == 0 {
        return Ok(amount);
    }
    let (_, treasury) = read_trade_fee_config(env).ok_or(ContractError::FeeConfigNotSet)?;
    credit_treasury_balance(env, trade_fee)?;
    env.events().publish(
        events::fee_collected_topics(&treasury),
        events::FeeCollectedEvent {
            treasury: treasury.clone(),
            amount: trade_fee,
            ledger: env.ledger().sequence(),
        },
    );
    fee::checked_sub_i128(amount, trade_fee).ok_or(ContractError::Overflow)
}

/// Reads the configured sell lockup duration in seconds.
///
/// Returns `None` until `set_lockup_duration` has been called, so sells are
/// never time-gated on deployments that do not opt in to the lockup.
fn read_lockup_duration_secs(env: &Env) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&constants::storage::LOCKUP_DURATION_SECS)
}

/// Reads the accumulated staking rewards pool balance for a creator, returning `0` when none is stored.
pub fn read_staking_rewards_pool(env: &Env, creator: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&constants::storage::staking_rewards_pool(creator))
        .unwrap_or(0)
}

/// Reads the total keys currently staked across all holders for a creator.
pub fn read_total_staked(env: &Env, creator: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&constants::storage::total_staked(creator))
        .unwrap_or(0)
}

/// Routes a share of a protocol fee collection into the creator's staking rewards pool.
///
/// This is additive bookkeeping on top of the existing treasury/protocol-fee-recipient
/// split — it does not reduce what those balances receive, so existing fee-accounting
/// invariants are unaffected. [`CreatorKeysContract::claim_stake_reward`] pays stakers
/// out of this dedicated pool.
fn credit_staking_rewards_pool(
    env: &Env,
    creator: &Address,
    protocol_fee: i128,
) -> Result<(), ContractError> {
    if protocol_fee <= 0 {
        return Ok(());
    }
    let share = fee::apply_percentage_fee(protocol_fee, STAKING_REWARD_SHARE_BPS)
        .ok_or(ContractError::Overflow)?;
    if share <= 0 {
        return Ok(());
    }
    let key = constants::storage::staking_rewards_pool(creator);
    let updated = read_staking_rewards_pool(env, creator)
        .checked_add(share)
        .ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&key, &updated);
    Ok(())
}

/// Archive retention configuration module with canonical defaults.
pub mod retention {
    use super::PartitionStrategy;

    /// Default retention window in days (30 days).
    pub const DEFAULT_RETENTION_DAYS: u32 = 30;
    /// Default partition strategy (Daily).
    pub const DEFAULT_PARTITION_STRATEGY: PartitionStrategy = PartitionStrategy::Daily;
    /// Default compression enabled flag (true).
    pub const DEFAULT_COMPRESSION_ENABLED: bool = true;
    /// Default batch size for archive processing (100).
    pub const DEFAULT_BATCH_SIZE: u32 = 100;
}

/// Returns the canonical default [`RetentionPolicy`].
pub fn default_retention_policy() -> RetentionPolicy {
    RetentionPolicy {
        retention_days: retention::DEFAULT_RETENTION_DAYS,
        partition_strategy: retention::DEFAULT_PARTITION_STRATEGY,
        compression_enabled: retention::DEFAULT_COMPRESSION_ENABLED,
        batch_size: retention::DEFAULT_BATCH_SIZE,
    }
}

/// Reads the current archive retention policy from storage, falling back to defaults.
pub fn read_retention_policy(env: &Env) -> RetentionPolicy {
    env.storage()
        .persistent()
        .get(&constants::storage::RETENTION_POLICY)
        .unwrap_or_else(default_retention_policy)
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
    // The protocol trade fee is deducted before the creator/seller split, so
    // proceeds are computed on the net amount to mirror the sell execution path.
    let trade_fee = compute_trade_fee(env, price)?;
    let net_price = fee::checked_sub_i128(price, trade_fee).ok_or(ContractError::Overflow)?;
    let (creator_fee, protocol_fee) =
        CreatorKeysContract::compute_fees_for_payment(env.clone(), net_price)?;
    let fees = fee::checked_fee_sum(creator_fee, protocol_fee).ok_or(ContractError::Overflow)?;
    fee::checked_sub_i128(net_price, fees).ok_or(ContractError::SellUnderflow)
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
    // Deduct the protocol trade fee first so the treasury is paid before the
    // creator/seller split is computed on the remainder.
    let net_price = collect_protocol_trade_fee(env, price)?;

    if read_protocol_fee_config(env).is_none() {
        return Ok(());
    }

    bump_persistent_ttl(env, &constants::storage::FEE_CONFIG);

    let (creator_fee, protocol_fee) =
        CreatorKeysContract::compute_fees_for_payment(env.clone(), net_price)?;
    credit_creator_fee(env, creator, creator_fee)?;
    credit_treasury_balance(env, protocol_fee)?;
    credit_staking_rewards_pool(env, creator, protocol_fee)?;

    if env
        .storage()
        .persistent()
        .get::<DataKey, Address>(&constants::storage::PROTOCOL_FEE_RECIPIENT)
        .is_some()
    {
        credit_protocol_fee_recipient_balance(env, protocol_fee)?;
    }

    if let Some(royalty) = read_royalty_config(env, creator) {
        let royalty_amount = fee::apply_percentage_fee(price, royalty.sell_fee_bps)
            .ok_or(ContractError::Overflow)?;
        if royalty_amount > 0 {
            credit_creator_fee_recipient_balance(env, creator, royalty_amount)?;
        }
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

/// Resolves the price [`CreatorKeysContract::get_buy_quote`] should report for the next
/// buy, mirroring [`CreatorKeysContract::buy_key`]'s own price resolution: while the
/// creator's supply is below a configured auction's `auction_supply`, the fixed auction
/// price applies instead of the bonding curve.
///
/// Preserves [`resolve_quote_inputs`]'s error-priority ordering (missing base price is
/// reported before an unregistered creator) for every case that isn't auction-specific.
fn resolve_buy_quote_price(env: &Env, creator: &Address) -> Result<Option<i128>, ContractError> {
    let base_price: i128 = env
        .storage()
        .persistent()
        .get(&constants::storage::KEY_PRICE)
        .ok_or(ContractError::KeyPriceNotSet)?;

    let Some(normalized) = normalize_quote_amount(base_price)? else {
        return Ok(None);
    };

    let profile = read_registered_creator_profile(env, creator)?;

    let auction_config: Option<AuctionConfig> = env
        .storage()
        .persistent()
        .get(&constants::storage::auction_config(creator));
    if let Some(config) = auction_config {
        if profile.supply < config.auction_supply {
            return normalize_quote_amount(config.auction_price);
        }
    }

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

fn read_royalty_config(env: &Env, creator: &Address) -> Option<RoyaltyConfig> {
    env.storage()
        .persistent()
        .get(&constants::storage::royalty_config(creator))
}

fn read_curve_exponent(env: &Env, creator: &Address) -> Option<u32> {
    env.storage()
        .persistent()
        .get(&constants::storage::curve_exponent(creator))
}

fn compute_bonding_curve_price(
    env: &Env,
    creator: &Address,
    base_price: i128,
    supply: u32,
) -> Result<i128, ContractError> {
    if let Some(exponent) = read_curve_exponent(env, creator) {
        let slope = read_curve_slope(env);
        let supply_exp = checked_pow_i128(supply as i128, exponent)?;
        let supply_component = slope
            .checked_mul(supply_exp)
            .ok_or(ContractError::Overflow)?;
        return base_price
            .checked_add(supply_component)
            .ok_or(ContractError::Overflow);
    }

    let preset = env
        .storage()
        .persistent()
        .get(&constants::storage::curve_preset(creator))
        .unwrap_or(CurvePreset::Linear);

    match preset {
        CurvePreset::Flat => Ok(base_price),
        CurvePreset::Linear => {
            let slope = read_curve_slope(env);
            let supply_component = slope
                .checked_mul(i128::from(supply))
                .ok_or(ContractError::Overflow)?;
            base_price
                .checked_add(supply_component)
                .ok_or(ContractError::Overflow)
        }
        CurvePreset::Quadratic => {
            let slope = read_curve_slope(env);
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

fn checked_pow_i128(base: i128, exp: u32) -> Result<i128, ContractError> {
    let mut result: i128 = 1;
    let mut _exp = exp;
    while _exp > 0 {
        result = result.checked_mul(base).ok_or(ContractError::Overflow)?;
        _exp -= 1;
    }
    Ok(result)
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
fn settle_holder_dividends(
    env: &Env,
    creator: &Address,
    holder: &Address,
    current_balance: u32,
) -> Result<(), ContractError> {
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
    let earned = (current_balance as i128)
        .checked_mul(diff)
        .ok_or(ContractError::Overflow)?;
    let new_pending = pending.checked_add(earned).ok_or(ContractError::Overflow)?;

    env.storage().persistent().set(&pending_key, &new_pending);
    env.storage()
        .persistent()
        .set(&checkpoint_key, &accumulator);
    // Keep dividend settlement state live for the same horizon as the
    // creator profile between trades.
    extend_key_ttl_to_full_window(env, &pending_key);
    extend_key_ttl_to_full_window(env, &checkpoint_key);
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

    let balance_key = constants::storage::holder_balance_key(creator, holder);
    let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);

    let diff = accumulator.saturating_sub(checkpoint);
    let earned = (balance as i128).saturating_mul(diff);
    pending.saturating_add(earned)
}

/// Extends the TTL of a freshly written storage entry to the full
/// [`CREATOR_TTL_LEDGERS`] window.
///
/// Uses `CREATOR_TTL_LEDGERS` as both the threshold and the extension window.
/// New entries start with the network-default TTL, which is shorter than
/// `CREATOR_TTL_LEDGERS` on fresh networks; forcing the full window at write
/// time keeps the entry's real TTL aligned with the live-until the contract
/// tracks for the TTL-extension event.
fn extend_key_ttl_to_full_window(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, CREATOR_TTL_LEDGERS, CREATOR_TTL_LEDGERS);
}

/// Extends the TTL of a persistent storage entry to at least
/// [`TTL_MIN_EXTENSION_LEDGERS`] (~30 days) from the current ledger.
///
/// Called after reads and writes on persistent entries that are not already
/// covered by the full-window extension, so actively used state never sits
/// closer to expiry than the 30-day floor. Extending an entry that does not
/// exist is a runtime no-op.
fn bump_persistent_ttl(env: &Env, key: &DataKey) {
    env.storage().persistent().extend_ttl(
        key,
        TTL_MIN_EXTENSION_LEDGERS,
        TTL_MIN_EXTENSION_LEDGERS,
    );
}

/// Extends TTL for all creator-related storage keys.
///
/// This function extends the TTL of the creator's primary storage entries
/// to prevent active creator state from expiring. Called after successful
/// buy, sell, and buyback operations. Emits a [`events::TTL_EXTENDED_EVENT_NAME`]
/// event only when the creator key's remaining TTL was below
/// [`TTL_EXTENSION_THRESHOLD`] before this call — a healthy TTL silently
/// skips the event.
fn extend_creator_ttl(env: &Env, creator: &Address) {
    let current_ledger = env.ledger().sequence();
    let extend_to = current_ledger + CREATOR_TTL_LEDGERS;
    let threshold = current_ledger;

    let creator_key = constants::storage::creator(creator);
    let live_until_key = constants::storage::creator_ttl_live_until(creator);

    // The Soroban SDK does not expose TTL reads to contract code, so the
    // contract tracks the live-until ledger it last set for the creator key
    // in persistent storage ([`DataKey::CreatorTtlLiveUntil`]). The remaining
    // TTL is derived from that value and used only to decide whether to emit
    // the TTL-extension event. The tracked value is always <= the entry's
    // real live-until (the network default can exceed `CREATOR_TTL_LEDGERS`),
    // so the event may fire slightly early on such networks — never too late.
    // The `extend_ttl` SDK calls below still run unconditionally — the
    // runtime no-ops when the entry already has a healthy expiration.
    let live_until: u32 = env
        .storage()
        .persistent()
        .get(&live_until_key)
        .unwrap_or(current_ledger);
    let remaining = live_until.saturating_sub(current_ledger);
    let needs_event = ttl::should_extend(remaining, TTL_EXTENSION_THRESHOLD);

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

    // Record the new live-until ledger so future trades can re-evaluate
    // whether the TTL-extension event should be emitted.
    env.storage().persistent().set(&live_until_key, &extend_to);
    extend_key_ttl_to_full_window(env, &live_until_key);

    // Only emit the TTL extension event when the remaining TTL was below the
    // extension threshold before this call. A healthy TTL silently skips the event.
    if needs_event {
        env.events()
            .publish(events::ttl_extended_topics(creator), extend_to);
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
    /// - `handle`: validated by [`validate_creator_handle`] — a blank handle
    ///   (empty or whitespace-only) returns [`ContractError::DisplayNameEmpty`],
    ///   below the minimum length returns [`ContractError::HandleTooShort`],
    ///   above the maximum returns [`ContractError::HandleTooLong`], and any
    ///   disallowed byte returns [`ContractError::InvalidHandleCharacter`].
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
        assert_not_blacklisted(&env, &creator)?;

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
        // Set initial TTL for creator storage. The full window is forced at
        // write time so the entry's real TTL matches the live-until the
        // contract tracks for the TTL-extension event.
        let extend_to = current_ledger + CREATOR_TTL_LEDGERS;
        extend_key_ttl_to_full_window(&env, &key);
        extend_key_ttl_to_full_window(&env, &preset_key);
        let co_creator_key = constants::storage::co_creator(&creator);
        if env.storage().persistent().has(&co_creator_key) {
            extend_key_ttl_to_full_window(&env, &co_creator_key);
        }
        let whitelist_key = constants::storage::whitelist(&creator);
        if env.storage().persistent().has(&whitelist_key) {
            extend_key_ttl_to_full_window(&env, &whitelist_key);
        }

        // Record the live-until the contract set for the creator key so
        // `extend_creator_ttl` can later decide whether to emit the
        // TTL-extension event.
        let live_until_key = constants::storage::creator_ttl_live_until(&creator);
        env.storage().persistent().set(&live_until_key, &extend_to);
        extend_key_ttl_to_full_window(&env, &live_until_key);

        env.events().publish(
            events::register_event_topics(&profile.creator),
            events::CreatorRegisteredEvent {
                creator: profile.creator.clone(),
                handle: profile.handle.clone(),
                supply: profile.supply,
                holder_count: profile.holder_count,
                creator_bps: fee_config.creator_bps,
                protocol_bps: fee_config.protocol_bps,
                fee_recipient: profile.fee_recipient.clone(),
                registered_at_ledger: current_ledger,
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
        Self::buy_key_impl(env, creator, buyer, payment, max_price, referrer)
    }

    /// Internal buy logic shared by `buy_key_with_referrer` and
    /// `batch_buy`.  Called without `require_auth` so that batch
    /// iterations do not double-authorise the same frame.
    fn buy_key_impl(
        env: Env,
        creator: Address,
        buyer: Address,
        payment: i128,
        max_price: Option<i128>,
        referrer: Option<Address>,
    ) -> Result<u32, ContractError> {
        assert_global_trading_not_halted(&env)?;
        assert_not_paused(&env)?;
        assert_not_blacklisted(&env, &buyer)?;
        assert_before_global_deadline(&env)?;

        if payment <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        if let Some(ref referrer_addr) = referrer {
            if *referrer_addr == buyer || *referrer_addr == creator {
                return Err(ContractError::InvalidReferrer);
            }
        }

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        // The price entry is read on every trade; keep it well clear of expiry.
        bump_persistent_ttl(&env, &constants::storage::KEY_PRICE);

        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;
        assert_whitelist_allows_buy(&env, &profile, &buyer)?;

        let auction_config_key = constants::storage::auction_config(&creator);
        let mut auction_config: Option<AuctionConfig> =
            env.storage().persistent().get(&auction_config_key);
        let in_auction = auction_config
            .as_ref()
            .map(|config| profile.supply < config.auction_supply)
            .unwrap_or(false);

        let price = if in_auction {
            // Auction-phase buys settle at the fixed auction price. The
            // circuit breaker only guards bonding-curve price movement, so
            // it does not apply while the fixed-price auction is active.
            auction_config
                .as_ref()
                .expect("in_auction implies auction_config is Some")
                .auction_price
        } else {
            let pre_price =
                compute_bonding_curve_price(&env, &creator, base_price, profile.supply)?;

            // Circuit breaker pre/post price check
            let post_supply = profile
                .supply
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
            let post_price = compute_bonding_curve_price(&env, &creator, base_price, post_supply)?;

            let threshold_pct: u32 = env
                .storage()
                .persistent()
                .get(&constants::storage::CIRCUIT_BREAKER_THRESHOLD)
                .unwrap_or(30);

            if pre_price > 0 {
                let price_change = post_price.saturating_sub(pre_price);
                let max_change = (pre_price as u128)
                    .checked_mul(threshold_pct as u128)
                    .ok_or(ContractError::Overflow)?
                    / 100;
                // When max_change rounds to zero the threshold is too
                // small to enforce meaningfully; skip the check.
                if max_change > 0 && (price_change as u128) >= max_change {
                    env.events().publish(
                        (events::circuit_breaker_triggered_topics(),),
                        events::CircuitBreakerTriggeredEvent {
                            pre_price,
                            post_price,
                        },
                    );
                    return Err(ContractError::CircuitBreakerTriggered);
                }
            }

            pre_price
        };

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

        // Check the per-creator percentage holding cap. Once a creator enables
        // a cap via `set_holder_cap`, no single non-creator wallet may hold more
        // than that share of the supply. The creator's own wallet is exempt.
        if buyer != creator {
            if let Some(cap_bps) = env
                .storage()
                .persistent()
                .get::<DataKey, u32>(&constants::storage::holder_cap_bps(&creator))
            {
                let post_buy_supply = profile
                    .supply
                    .checked_add(1)
                    .ok_or(ContractError::Overflow)?;
                let post_buy_balance = current_balance
                    .checked_add(1)
                    .ok_or(ContractError::Overflow)?;
                // cap_bps <= HOLDER_CAP_MAX_BPS (< BPS_MAX), so the product
                // cannot overflow i128 and the result never exceeds supply.
                let max_allowed = ((i128::from(post_buy_supply) * i128::from(cap_bps))
                    / i128::from(fee::BPS_MAX)) as u32;
                if post_buy_balance > max_allowed {
                    return Err(ContractError::SupplyCapExceeded);
                }
            }
        }

        // Settle dividends before balance changes so earnings are captured at old balance.
        settle_holder_dividends(&env, &creator, &buyer, current_balance)?;

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
        // Grant the balance entry the full TTL window so long-held positions
        // survive the same horizon as creator state between trades.
        extend_key_ttl_to_full_window(&env, &balance_key);

        // Record the purchase time on the holder's entry so sells can enforce
        // the anti-flash-trade lockup window.
        let last_buy_key = constants::storage::last_buy_timestamp(&creator, &buyer);
        env.storage()
            .persistent()
            .set(&last_buy_key, &env.ledger().timestamp());
        extend_key_ttl_to_full_window(&env, &last_buy_key);

        // Deduct the protocol trade fee before computing the creator payout so
        // the fee collector is paid ahead of every other participant.
        let net_amount = collect_protocol_trade_fee(&env, price)?;

        if let Some(config) = read_protocol_fee_config(&env) {
            let (creator_fee, protocol_fee) =
                fee::checked_compute_fee_split(net_amount, config.creator_bps, config.protocol_bps)
                    .ok_or(ContractError::Overflow)?;

            credit_creator_fee(&env, &creator, creator_fee)?;
            credit_staking_rewards_pool(&env, &creator, protocol_fee)?;

            // Split protocol fee between treasury and referrer only when a referrer is provided
            if let Some(referrer_addr) = referrer {
                let referral_amount = protocol_fee / 2;
                let treasury_amount = protocol_fee - referral_amount;

                credit_treasury_balance(&env, treasury_amount)?;
                credit_protocol_fee_recipient_balance(&env, treasury_amount)?;

                if referral_amount > 0 {
                    let ref_key = constants::storage::referral_earnings(&referrer_addr);
                    let current_earnings: i128 =
                        env.storage().persistent().get(&ref_key).unwrap_or(0);
                    let new_earnings = current_earnings
                        .checked_add(referral_amount)
                        .ok_or(ContractError::Overflow)?;
                    env.storage().persistent().set(&ref_key, &new_earnings);
                    extend_key_ttl_to_full_window(&env, &ref_key);

                    env.events().publish(
                        (events::referral_fee_paid_topics(),),
                        events::ReferralFeePaidEvent {
                            referrer: referrer_addr,
                            amount: referral_amount,
                        },
                    );
                }
            } else {
                // No referrer: full protocol fee goes to treasury and recipient
                credit_treasury_balance(&env, protocol_fee)?;
                credit_protocol_fee_recipient_balance(&env, protocol_fee)?;
            }
        }

        if let Some(royalty) = read_royalty_config(&env, &creator) {
            let royalty_amount = fee::apply_percentage_fee(price, royalty.buy_fee_bps)
                .ok_or(ContractError::Overflow)?;
            if royalty_amount > 0 {
                credit_creator_fee_recipient_balance(&env, &creator, royalty_amount)?;
            }
        }

        if in_auction {
            let mut config = auction_config
                .take()
                .expect("in_auction implies auction_config is Some");
            config.auction_sold = config
                .auction_sold
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
            env.storage().persistent().set(&auction_config_key, &config);

            env.events().publish(
                events::auction_purchase_topics(&creator, &buyer),
                events::AuctionPurchaseEvent {
                    buyer: buyer.clone(),
                    creator_id: creator.clone(),
                    quantity: 1,
                    price_paid: price,
                    new_supply: profile.supply,
                    auction_sold: config.auction_sold,
                    ledger: env.ledger().sequence(),
                },
            );
        } else {
            let buy_event_data = events::KeysBoughtEvent {
                buyer: buyer.clone(),
                creator_id: creator.clone(),
                quantity: 1,
                price_paid: price,
                new_supply: profile.supply,
                ledger: env.ledger().sequence(),
            };

            env.events()
                .publish(events::buy_event_topics(&creator, &buyer), buy_event_data);
        }

        // Extend TTL for creator storage after successful buy
        extend_creator_ttl(&env, &creator);

        Ok(profile.supply)
    }

    /// Validates that `client_schema_version` is compatible with this deployment.
    ///
    /// Returns `Ok(())` when the version matches the contract's current schema.
    /// Returns an error when the client is too old or too new:
    /// - [`ContractError::SchemaVersionTooOld`] — version is `0` or below the
    ///   minimum supported version; the client must upgrade.
    /// - [`ContractError::SchemaVersionUnsupported`] — version exceeds the
    ///   current schema; this contract deployment does not understand it yet.
    ///
    /// No state is read or written; the check is pure and can be called without
    /// authorization.
    pub fn check_schema_version(
        _env: Env,
        client_schema_version: u32,
    ) -> Result<(), ContractError> {
        assert_schema_version(client_schema_version)
    }

    pub fn sell_key(
        env: Env,
        creator: Address,
        seller: Address,
        min_proceeds: Option<i128>,
    ) -> Result<u32, ContractError> {
        seller.require_auth();
        assert_global_trading_not_halted(&env)?;
        assert_not_paused(&env)?;
        assert_not_blacklisted(&env, &seller)?;

        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

        let balance_key = constants::storage::holder_balance_key(&creator, &seller);
        // Missing balance entries are interpreted as zero and rejected consistently.
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if current_balance == 0 {
            return Err(ContractError::InsufficientBalance);
        }

        // Check liquid balance (total balance - staked balance)
        let staked_balance_key = constants::storage::staked_balance(&creator, &seller);
        let staked_balance: u32 = env
            .storage()
            .persistent()
            .get(&staked_balance_key)
            .unwrap_or(0);
        let liquid_balance = current_balance.saturating_sub(staked_balance);

        if liquid_balance == 0 {
            return Err(ContractError::InsufficientBalance);
        }

        // Enforce the anti-flash-trade lockup: once a lockup duration is
        // configured, a holder cannot sell until the configured time has
        // elapsed since their most recent buy for this creator.
        if let Some(lockup_secs) = read_lockup_duration_secs(&env) {
            let last_buy_key = constants::storage::last_buy_timestamp(&creator, &seller);
            if let Some(last_buy_ts) = env
                .storage()
                .persistent()
                .get::<DataKey, u64>(&last_buy_key)
            {
                let now = env.ledger().timestamp();
                let unlock_at = last_buy_ts
                    .checked_add(lockup_secs)
                    .ok_or(ContractError::Overflow)?;
                if now < unlock_at {
                    env.events().publish(
                        events::lockup_blocked_topics(&creator, &seller),
                        events::LockupBlockedEvent {
                            creator_id: creator.clone(),
                            seller: seller.clone(),
                            last_buy_timestamp: last_buy_ts,
                            unlock_at,
                            current_timestamp: now,
                        },
                    );
                    return Err(ContractError::SellUnderflow);
                }
            }
        }

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        // The price entry is read on every trade; keep it well clear of expiry.
        bump_persistent_ttl(&env, &constants::storage::KEY_PRICE);
        let sell_supply = profile
            .supply
            .checked_sub(1)
            .ok_or(ContractError::SellUnderflow)?;
        let price = compute_bonding_curve_price(&env, &creator, base_price, sell_supply)?;

        // Settle dividends before balance changes so earnings are captured at old balance.
        settle_holder_dividends(&env, &creator, &seller, current_balance)?;

        assert_sell_proceeds_slippage(&env, price, min_proceeds)?;

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
        if new_balance == 0 {
            env.storage().persistent().remove(&balance_key);
            env.storage()
                .persistent()
                .remove(&constants::storage::last_buy_timestamp(&creator, &seller));
        } else {
            env.storage().persistent().set(&balance_key, &new_balance);
            extend_key_ttl_to_full_window(&env, &balance_key);
        }
        accrue_sell_trade_fees(&env, &creator, price)?;

        let proceeds = compute_sell_proceeds(&env, price).unwrap_or(0);
        let sell_event_data = events::KeysSoldEvent {
            seller: seller.clone(),
            creator_id: creator.clone(),
            quantity: 1,
            proceeds,
            ledger: env.ledger().sequence(),
        };

        env.events().publish(
            (events::SELL_EVENT_NAME, creator.clone(), seller),
            sell_event_data,
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
        let config = read_required_protocol_fee_config(&env)?;
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
        if let Some(config) = read_protocol_fee_config(&env) {
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
            settle_holder_dividends(&env, &creator, &entry.address, current_balance)?;

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

    /// Sets the protocol-wide deadline ledger after which buys are rejected.
    ///
    /// Only the protocol admin may call this. The deadline is exclusive: buys
    /// are accepted while `ledger < deadline_ledger` and rejected with
    /// [`ContractError::DeadlinePassed`] from `deadline_ledger` onwards. Passing
    /// `None` clears the deadline and reopens buying indefinitely.
    ///
    /// Emits a `dl_set` event carrying the admin and the new deadline.
    pub fn set_global_deadline(
        env: Env,
        admin: Address,
        deadline_ledger: Option<u32>,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        match deadline_ledger {
            Some(deadline) => {
                env.storage()
                    .persistent()
                    .set(&constants::storage::GLOBAL_DEADLINE_LEDGER, &deadline);
                extend_key_ttl_to_full_window(&env, &constants::storage::GLOBAL_DEADLINE_LEDGER);
            }
            None => env
                .storage()
                .persistent()
                .remove(&constants::storage::GLOBAL_DEADLINE_LEDGER),
        }

        env.events().publish(
            (events::GLOBAL_DEADLINE_SET_EVENT_NAME, admin),
            deadline_ledger,
        );
        Ok(())
    }

    /// Read-only view: returns the configured global deadline ledger, if any.
    pub fn get_global_deadline(env: Env) -> Option<u32> {
        read_global_deadline(&env)
    }

    /// Blocks a wallet from buying, selling, or registering as a creator.
    ///
    /// Only the protocol admin may call this. Emits a `blacklist` event.
    pub fn blacklist_wallet(
        env: Env,
        admin: Address,
        wallet: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&constants::storage::blacklisted(&wallet), &true);
        env.events()
            .publish((events::BLACKLIST_ADDED_EVENT_NAME, wallet), ());
        Ok(())
    }

    /// Restores a previously blacklisted wallet's access to buy, sell, and
    /// register as a creator.
    ///
    /// Only the protocol admin may call this. Emits an `unblacklist` event.
    pub fn remove_from_blacklist(
        env: Env,
        admin: Address,
        wallet: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .remove(&constants::storage::blacklisted(&wallet));
        env.events()
            .publish((events::BLACKLIST_REMOVED_EVENT_NAME, wallet), ());
        Ok(())
    }

    /// Read-only view: returns whether `wallet` is currently blacklisted.
    pub fn is_wallet_blacklisted(env: Env, wallet: Address) -> bool {
        is_blacklisted(&env, &wallet)
    }

    pub fn get_key_balance(env: Env, creator: Address, wallet: Address) -> u32 {
        let key = constants::storage::holder_balance_key(&creator, &wallet);
        // Read-only callers get `0` for unseen balances to avoid sparse-map lookups failing.
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Read-only view: returns the key balance for a wallet and creator.
    ///
    /// Alias for [`get_key_balance`](Self::get_key_balance). Returns `0` for any wallet address
    /// that has never bought or been transferred keys, or that has sold all keys, without panicking or returning an error.
    pub fn get_balance(env: Env, creator: Address, wallet: Address) -> u32 {
        Self::get_key_balance(env, creator, wallet)
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
        read_creator_fee_recipient(&env, &creator).ok_or(ContractError::NotRegistered)
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
        assert_is_admin(&env, &admin)?;
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
        let is_first_init = old_config.is_none();
        let old_bps = old_config.as_ref().map(|c| c.creator_bps).unwrap_or(0);

        env.storage()
            .persistent()
            .set(&constants::storage::FEE_CONFIG, &config);
        extend_key_ttl_to_full_window(&env, &constants::storage::FEE_CONFIG);

        if is_first_init {
            let protocol_fee_recipient: Address = env
                .storage()
                .persistent()
                .get(&constants::storage::PROTOCOL_FEE_RECIPIENT)
                .unwrap_or_else(|| admin.clone());
            env.events().publish(
                (events::CONTRACT_INITIALIZED_EVENT_NAME, admin.clone()),
                events::ContractInitializedEvent {
                    admin: admin.clone(),
                    protocol_fee_bps: protocol_bps,
                    protocol_fee_recipient,
                    initialized_at_ledger: env.ledger().sequence(),
                },
            );
        }

        // Emit global fee config update event
        env.events().publish(
            (events::FEE_CONFIG_UPDATED_EVENT_NAME, admin),
            events::FeeConfigUpdatedEvent {
                old_bps,
                new_bps: creator_bps,
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
        // Grant the price entry the full TTL window so buy/sell reads stay
        // live for the same horizon as creator state.
        extend_key_ttl_to_full_window(&env, &constants::storage::KEY_PRICE);
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
        extend_key_ttl_to_full_window(&env, &constants::storage::CURVE_SLOPE);
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
        extend_key_ttl_to_full_window(&env, &constants::storage::TREASURY_ADDRESS);
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

    /// Configures the protocol trade fee charged on every buy and sell.
    ///
    /// The fee is deducted from the trade amount before the creator payout is
    /// computed and credited to the protocol treasury balance. Both the fee
    /// rate and the treasury address are stored in persistent storage; the fee
    /// stays dormant until this entrypoint is called.
    ///
    /// Parameter validation:
    /// - `admin`: must authorize the call (`require_auth`) and match the stored
    ///   admin, otherwise [`ContractError::Unauthorized`].
    /// - `fee_bps`: `None` selects [`DEFAULT_PROTOCOL_FEE_BPS`] (100 = 1%); an
    ///   explicit value above [`fee::BPS_MAX`] returns
    ///   [`ContractError::InvalidFeeConfig`]. A rate of 0 bps leaves trades
    ///   fee-free and skips the treasury credit entirely.
    /// - `treasury`: must not be the Stellar zero address, otherwise
    ///   [`ContractError::ZeroAddress`].
    pub fn set_protocol_fee(
        env: Env,
        admin: Address,
        fee_bps: Option<u32>,
        treasury: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        validate_non_zero_address(&env, &treasury)?;

        let resolved_bps = fee_bps.unwrap_or(DEFAULT_PROTOCOL_FEE_BPS);
        if resolved_bps > fee::BPS_MAX {
            return Err(ContractError::InvalidFeeConfig);
        }

        env.storage()
            .persistent()
            .set(&constants::storage::PROTOCOL_FEE_BPS, &resolved_bps);
        extend_key_ttl_to_full_window(&env, &constants::storage::PROTOCOL_FEE_BPS);
        env.storage()
            .persistent()
            .set(&constants::storage::TREASURY_ADDRESS, &treasury);
        extend_key_ttl_to_full_window(&env, &constants::storage::TREASURY_ADDRESS);

        Ok(())
    }

    /// Read-only view: returns the configured protocol trade fee.
    ///
    /// Returns `(fee_bps, Some(treasury))` once `set_protocol_fee` has been
    /// called and `(0, None)` while the trade fee is dormant.
    pub fn get_protocol_trade_fee(env: Env) -> (u32, Option<Address>) {
        match read_trade_fee_config(&env) {
            Some((fee_bps, treasury)) => (fee_bps, Some(treasury)),
            None => (0, None),
        }
    }

    /// Sets the protocol admin address.
    ///
    /// Only callable by an authorized admin. Stores the admin address used
    /// for protocol administration.
    pub fn set_protocol_admin(
        env: Env,
        admin: Address,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        validate_non_zero_address(&env, &new_admin)?;

        let current_admin: Option<Address> = env
            .storage()
            .persistent()
            .get(&constants::storage::ADMIN_ADDRESS);

        if let Some(ref current) = current_admin {
            if admin != *current {
                return Err(ContractError::Unauthorized);
            }
            if *current == new_admin {
                return Ok(());
            }
        }

        env.storage()
            .persistent()
            .set(&constants::storage::ADMIN_ADDRESS, &new_admin);
        extend_key_ttl_to_full_window(&env, &constants::storage::ADMIN_ADDRESS);
        Ok(())
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
        assert_is_admin(&env, &admin)?;
        validate_non_zero_address(&env, &recipient)?;

        let old_recipient: Option<Address> = env
            .storage()
            .persistent()
            .get(&constants::storage::PROTOCOL_FEE_RECIPIENT);

        if old_recipient.as_ref() == Some(&recipient) {
            return Ok(());
        }

        env.storage()
            .persistent()
            .set(&constants::storage::PROTOCOL_FEE_RECIPIENT, &recipient);
        extend_key_ttl_to_full_window(&env, &constants::storage::PROTOCOL_FEE_RECIPIENT);

        if let Some(old) = old_recipient {
            env.events().publish(
                (events::PROTOCOL_FEE_RECIPIENT_UPDATED_EVENT_NAME, admin),
                events::ProtocolFeeRecipientUpdatedEvent {
                    old_recipient: old,
                    new_recipient: recipient,
                },
            );
        }

        Ok(())
    }

    /// Sets the archive retention policy configuration.
    ///
    /// Only callable by an authorized admin.
    ///
    /// Parameter validation:
    /// - `admin`: must authorize the call (`require_auth`).
    /// - `batch_size`: must be strictly positive; returns [`ContractError::NotPositiveAmount`].
    pub fn set_retention_policy(
        env: Env,
        admin: Address,
        retention_days: u32,
        partition_strategy: PartitionStrategy,
        compression_enabled: bool,
        batch_size: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        if batch_size == 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let policy = RetentionPolicy {
            retention_days,
            partition_strategy,
            compression_enabled,
            batch_size,
        };

        env.storage()
            .persistent()
            .set(&constants::storage::RETENTION_POLICY, &policy);
        extend_key_ttl_to_full_window(&env, &constants::storage::RETENTION_POLICY);

        Ok(())
    }

    /// Read-only view: returns the current archive retention configuration.
    ///
    /// Returns the configured [`RetentionPolicy`] or canonical defaults if unset.
    /// Does not mutate contract state or panic when uninitialized.
    pub fn get_retention_policy(env: Env) -> RetentionPolicy {
        read_retention_policy(&env)
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
    /// Fees are calculated based on the fixed key price, or the fixed auction price
    /// while a pre-launch auction configured via [`Self::configure_auction`] is still
    /// active for `creator` — mirroring exactly what [`Self::buy_key`] would charge.
    pub fn get_buy_quote(env: Env, creator: Address) -> Result<QuoteResponse, ContractError> {
        let Some(price) = resolve_buy_quote_price(&env, &creator)? else {
            return Ok(zero_quote_response());
        };
        let (creator_fee, protocol_fee) = Self::compute_fees_for_payment(env.clone(), price)?;
        checked_format_quote_response(price, creator_fee, protocol_fee, true)
    }

    /// Read-only price query helper for a given creator and supply step.
    ///
    /// Computes the bonding curve price for `supply` without requiring authorization
    /// or mutating contract state.
    ///
    /// Returns `Err(ContractError::KeyPriceNotSet)` if base key price is not set,
    /// or `Err(ContractError::Overflow)` if arithmetic overflows or supply exceeds `u32::MAX`.
    pub fn query_price(env: Env, creator: Address, supply: u64) -> Result<i128, ContractError> {
        let supply_u32 = u32::try_from(supply).map_err(|_| ContractError::Overflow)?;
        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;

        compute_bonding_curve_price(&env, &creator, base_price, supply_u32)
    }

    /// Read-only price query helper for a given creator and supply step.
    ///
    /// Computes the bonding curve price for `supply` without requiring authorization
    /// or mutating contract state. At `supply == 0`, returns the configured base key price.
    ///
    /// Returns `Err(ContractError::KeyPriceNotSet)` if base key price is not set,
    /// or `Err(ContractError::Overflow)` if arithmetic overflows or supply exceeds `u32::MAX`.
    pub fn get_price(env: Env, creator: Address, supply: u64) -> Result<i128, ContractError> {
        Self::query_price(env, creator, supply)
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
        let config = read_required_protocol_fee_config(&env)?;
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

        let config = read_required_protocol_fee_config(&env)?;
        let (net_amount, protocol_amount) =
            fee::compute_fee_split(amount, config.creator_bps, config.protocol_bps);

        credit_protocol_fee_recipient_balance(&env, protocol_amount)?;

        let per_key_net = net_amount / profile.supply as i128;

        let acc_key = constants::storage::dividend_accumulator(&creator);
        let accumulator: i128 = env.storage().persistent().get(&acc_key).unwrap_or(0);
        let new_accumulator = fee::checked_accumulate(accumulator, per_key_net)?;
        env.storage().persistent().set(&acc_key, &new_accumulator);
        extend_key_ttl_to_full_window(&env, &acc_key);

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
        let pending_key = constants::storage::holder_dividend_pending(&creator, &holder);
        let checkpoint_key = constants::storage::holder_dividend_checkpoint(&creator, &holder);
        env.storage().persistent().set(&pending_key, &0i128);
        env.storage()
            .persistent()
            .set(&checkpoint_key, &accumulator);
        extend_key_ttl_to_full_window(&env, &pending_key);
        extend_key_ttl_to_full_window(&env, &checkpoint_key);

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
                let pending_key = constants::storage::holder_dividend_pending(&creator, &holder);
                let checkpoint_key =
                    constants::storage::holder_dividend_checkpoint(&creator, &holder);
                env.storage().persistent().set(&pending_key, &0i128);
                env.storage()
                    .persistent()
                    .set(&checkpoint_key, &accumulator);
                extend_key_ttl_to_full_window(&env, &pending_key);
                extend_key_ttl_to_full_window(&env, &checkpoint_key);

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
        write_creator_fee_recipient(&env, &creator, &new_recipient);

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

    /// Sets the maximum share of the supply a single wallet may hold for this
    /// creator's keys.
    ///
    /// Only callable by the creator. `cap_bps` may be omitted to select
    /// [`DEFAULT_HOLDER_CAP_BPS`] (10%); an explicit value must lie between
    /// [`HOLDER_CAP_MIN_BPS`] (1%) and [`HOLDER_CAP_MAX_BPS`] (25%), otherwise
    /// [`ContractError::InvalidFeeConfig`] is returned. Once configured,
    /// `buy_key` rejects purchases that would push a non-creator wallet above
    /// `cap_bps` of the total supply with
    /// [`ContractError::SupplyCapExceeded`]. The creator's own wallet is
    /// exempt from the cap.
    pub fn set_holder_cap(
        env: Env,
        creator: Address,
        cap_bps: Option<u32>,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        let resolved_bps = cap_bps.unwrap_or(DEFAULT_HOLDER_CAP_BPS);
        if !(HOLDER_CAP_MIN_BPS..=HOLDER_CAP_MAX_BPS).contains(&resolved_bps) {
            return Err(ContractError::InvalidFeeConfig);
        }
        let key = constants::storage::holder_cap_bps(&creator);
        env.storage().persistent().set(&key, &resolved_bps);
        extend_key_ttl_to_full_window(&env, &key);
        Ok(())
    }

    /// Read-only view: returns the holder cap basis points for a creator.
    ///
    /// Returns `None` while no cap is configured, meaning buys are not limited
    /// by a percentage holding cap.
    pub fn get_holder_cap(env: Env, creator: Address) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&constants::storage::holder_cap_bps(&creator))
    }

    /// Configures the sell lockup duration enforced on every sell.
    ///
    /// Only the protocol admin may call this. A duration of 0 returns
    /// [`ContractError::NotPositiveAmount`]; use [`DEFAULT_LOCKUP_DURATION_SECS`]
    /// (24 hours) as the canonical starting value. Once configured, `sell_key`
    /// rejects sales made less than `duration_secs` after the seller's most
    /// recent buy of that creator's keys with
    /// [`ContractError::SellUnderflow`] and emits a
    /// [`events::LOCKUP_BLOCKED_EVENT_NAME`] event.
    pub fn set_lockup_duration(
        env: Env,
        admin: Address,
        duration_secs: u64,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        if duration_secs == 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        env.storage()
            .persistent()
            .set(&constants::storage::LOCKUP_DURATION_SECS, &duration_secs);
        extend_key_ttl_to_full_window(&env, &constants::storage::LOCKUP_DURATION_SECS);
        Ok(())
    }

    /// Read-only view: returns the effective sell lockup duration in seconds.
    ///
    /// Returns [`DEFAULT_LOCKUP_DURATION_SECS`] when no duration has been
    /// configured; note the lockup is only enforced after `set_lockup_duration`
    /// has been called.
    pub fn get_lockup_duration(env: Env) -> u64 {
        read_lockup_duration_secs(&env).unwrap_or(DEFAULT_LOCKUP_DURATION_SECS)
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
        settle_holder_dividends(&env, &creator, &from, from_balance)?;

        if from_balance < amount {
            return Err(ContractError::InsufficientBalance);
        }

        // Settle dividends for recipient before balance changes.
        let to_balance_key = constants::storage::holder_balance_key(&creator, &to);
        let to_balance: u32 = env.storage().persistent().get(&to_balance_key).unwrap_or(0);
        settle_holder_dividends(&env, &creator, &to, to_balance)?;

        // Update sender balance.
        let new_from_balance = from_balance
            .checked_sub(amount)
            .ok_or(ContractError::InsufficientBalance)?;
        env.storage()
            .persistent()
            .set(&from_balance_key, &new_from_balance);
        extend_key_ttl_to_full_window(&env, &from_balance_key);

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
        extend_key_ttl_to_full_window(&env, &to_balance_key);

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
        extend_key_ttl_to_full_window(&env, &constants::storage::TREASURY_BALANCE);

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

    /// Stakes a specified amount of keys for a holder.
    ///
    /// Staked keys are locked and cannot be sold until unstaked. The holder must authorize
    /// the call. The staked amount is tracked separately from the total balance.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotPositiveAmount`] if `amount` is zero
    /// - [`ContractError::InsufficientBalance`] if the holder's liquid balance is less than `amount`
    /// - [`ContractError::ProtocolPaused`] if the contract is paused
    pub fn stake_keys(
        env: Env,
        creator: Address,
        holder: Address,
        amount: u32,
    ) -> Result<(), ContractError> {
        holder.require_auth();
        assert_not_paused(&env)?;

        if amount == 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        // Verify creator is registered
        let _profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

        let balance_key = constants::storage::key_balance(&creator, &holder);
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        let staked_balance_key = constants::storage::staked_balance(&creator, &holder);
        let current_staked: u32 = env
            .storage()
            .persistent()
            .get(&staked_balance_key)
            .unwrap_or(0);

        // Check if holder has enough liquid balance to stake
        let liquid_balance = current_balance.saturating_sub(current_staked);
        if liquid_balance < amount {
            return Err(ContractError::InsufficientBalance);
        }

        // Update staked balance
        let new_staked = current_staked
            .checked_add(amount)
            .ok_or(ContractError::Overflow)?;
        env.storage()
            .persistent()
            .set(&staked_balance_key, &new_staked);
        extend_key_ttl_to_full_window(&env, &staked_balance_key);

        let total_staked_key = constants::storage::total_staked(&creator);
        let new_total_staked = read_total_staked(&env, &creator)
            .checked_add(amount)
            .ok_or(ContractError::Overflow)?;
        env.storage()
            .persistent()
            .set(&total_staked_key, &new_total_staked);

        // Refresh the reward-claim lock window on every additional stake so
        // `claim_stake_reward` always measures eligibility from the most
        // recent stake.
        let unlock_key = constants::storage::stake_unlock_ledger(&creator, &holder);
        let unlock_ledger = env
            .ledger()
            .sequence()
            .checked_add(STAKE_LOCK_LEDGERS)
            .ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&unlock_key, &unlock_ledger);

        Ok(())
    }

    /// Unstakes a specified amount of keys for a holder.
    ///
    /// Unstaked keys become liquid and can be sold. The holder must authorize the call.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotPositiveAmount`] if `amount` is zero
    /// - [`ContractError::InsufficientBalance`] if the holder's staked balance is less than `amount`
    /// - [`ContractError::ProtocolPaused`] if the contract is paused
    pub fn unstake_keys(
        env: Env,
        creator: Address,
        holder: Address,
        amount: u32,
    ) -> Result<(), ContractError> {
        holder.require_auth();
        assert_not_paused(&env)?;

        if amount == 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        // Verify creator is registered
        let _profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

        let staked_balance_key = constants::storage::staked_balance(&creator, &holder);
        let current_staked: u32 = env
            .storage()
            .persistent()
            .get(&staked_balance_key)
            .unwrap_or(0);

        if current_staked < amount {
            return Err(ContractError::InsufficientBalance);
        }

        // Update staked balance
        let new_staked = current_staked
            .checked_sub(amount)
            .ok_or(ContractError::Overflow)?;

        if new_staked == 0 {
            env.storage().persistent().remove(&staked_balance_key);
            env.storage()
                .persistent()
                .remove(&constants::storage::stake_unlock_ledger(&creator, &holder));
        } else {
            env.storage()
                .persistent()
                .set(&staked_balance_key, &new_staked);
            extend_key_ttl_to_full_window(&env, &staked_balance_key);
        }

        let total_staked_key = constants::storage::total_staked(&creator);
        let new_total_staked = read_total_staked(&env, &creator).saturating_sub(amount);
        if new_total_staked == 0 {
            env.storage().persistent().remove(&total_staked_key);
        } else {
            env.storage()
                .persistent()
                .set(&total_staked_key, &new_total_staked);
        }

        Ok(())
    }

    /// Returns the staked balance for a holder.
    ///
    /// Staked keys are locked and cannot be sold until unstaked.
    pub fn get_staked_balance(env: Env, creator: Address, holder: Address) -> u32 {
        let staked_balance_key = constants::storage::staked_balance(&creator, &holder);
        env.storage()
            .persistent()
            .get(&staked_balance_key)
            .unwrap_or(0)
    }

    /// Returns the liquid balance for a holder.
    ///
    /// Liquid balance is the total balance minus staked balance. Only liquid keys can be sold.
    pub fn get_liquid_balance(env: Env, creator: Address, holder: Address) -> u32 {
        let balance_key = constants::storage::key_balance(&creator, &holder);
        let total_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        let staked_balance_key = constants::storage::staked_balance(&creator, &holder);
        let staked_balance: u32 = env
            .storage()
            .persistent()
            .get(&staked_balance_key)
            .unwrap_or(0);

        total_balance.saturating_sub(staked_balance)
    }

    // =========================================================================
    // #766 — Supply cap configuration
    // =========================================================================

    /// Sets or updates the supply cap for a creator's keys.
    ///
    /// Only callable by the creator. Panics with `CapAlreadySet` if a cap is
    /// already set and the new cap is lower than the current supply.
    pub fn set_supply_cap(env: Env, creator: Address, cap: u32) -> Result<(), ContractError> {
        creator.require_auth();

        let profile = read_registered_creator_profile(&env, &creator)?;
        if profile.creator != creator {
            return Err(ContractError::Unauthorized);
        }

        let cap_key = constants::storage::max_supply(&creator);
        let existing: Option<u32> = env.storage().persistent().get(&cap_key);

        if existing.is_some() {
            return Err(ContractError::CapAlreadySet);
        }

        if cap == 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        if profile.supply > cap {
            return Err(ContractError::CapAlreadySet);
        }

        env.storage().persistent().set(&cap_key, &cap);

        env.events().publish(
            events::supply_cap_set_topics(&creator),
            events::SupplyCapSetEvent {
                creator_id: creator,
                cap,
            },
        );

        Ok(())
    }

    // =========================================================================
    // #761 — Multi-sig pause/unpause
    // =========================================================================

    /// Sets the multisig admin list for a creator (up to 3 addresses).
    ///
    /// Only callable by the creator. Replaces any existing admin list.
    pub fn set_multisig_admins(
        env: Env,
        creator: Address,
        admins: Vec<Address>,
    ) -> Result<(), ContractError> {
        creator.require_auth();

        read_registered_creator_profile(&env, &creator)?;

        if admins.len() > 3 || admins.is_empty() {
            return Err(ContractError::MultisigAdminLimitExceeded);
        }

        let config = MultisigAdmins { admins };
        env.storage()
            .persistent()
            .set(&constants::storage::multisig_admins(&creator), &config);

        Ok(())
    }

    /// Read-only view: returns the multisig admin list for a creator.
    pub fn get_multisig_admins(env: Env, creator: Address) -> Option<MultisigAdmins> {
        env.storage()
            .persistent()
            .get(&constants::storage::multisig_admins(&creator))
    }

    /// Proposes a pause for a creator's trading.
    ///
    /// Callable by any admin in the multisig list. If this is the first
    /// proposal, it records the proposer and awaits a second approval.
    pub fn propose_pause(env: Env, creator: Address, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();

        let config: MultisigAdmins = env
            .storage()
            .persistent()
            .get(&constants::storage::multisig_admins(&creator))
            .ok_or(ContractError::Unauthorized)?;

        let mut is_admin = false;
        for admin in config.admins.iter() {
            if admin == caller {
                is_admin = true;
                break;
            }
        }
        if !is_admin {
            return Err(ContractError::Unauthorized);
        }

        let proposal_key = constants::storage::pause_proposal(&creator, &caller);
        if env.storage().persistent().has(&proposal_key) {
            return Err(ContractError::AlreadyApproved);
        }

        let proposal = PauseProposal {
            proposer: caller.clone(),
            approved: true,
        };
        env.storage().persistent().set(&proposal_key, &proposal);

        env.events().publish(
            events::pause_proposed_topics(&creator),
            events::PauseProposedEvent {
                creator_id: creator,
                proposer: caller,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    /// Approves a pause proposal for a creator's trading.
    ///
    /// Callable by a second admin. When the approval threshold (2 of 3) is
    /// reached, the pause executes automatically and all proposals are reset.
    pub fn approve_pause(env: Env, creator: Address, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();

        let config: MultisigAdmins = env
            .storage()
            .persistent()
            .get(&constants::storage::multisig_admins(&creator))
            .ok_or(ContractError::Unauthorized)?;

        let mut is_admin = false;
        for admin in config.admins.iter() {
            if admin == caller {
                is_admin = true;
                break;
            }
        }
        if !is_admin {
            return Err(ContractError::Unauthorized);
        }

        let caller_proposal_key = constants::storage::pause_proposal(&creator, &caller);
        if env.storage().persistent().has(&caller_proposal_key) {
            return Err(ContractError::AlreadyApproved);
        }

        // Check if another admin has already proposed
        let mut has_other_proposal = false;
        for admin in config.admins.iter() {
            if admin != caller {
                let proposal_key = constants::storage::pause_proposal(&creator, &admin);
                if env.storage().persistent().has(&proposal_key) {
                    has_other_proposal = true;
                    break;
                }
            }
        }

        if !has_other_proposal {
            return Err(ContractError::ProposalNotFound);
        }

        // Threshold reached — execute pause
        env.storage()
            .persistent()
            .set(&constants::storage::PAUSED, &true);

        // Reset all proposals
        for admin in config.admins.iter() {
            let proposal_key = constants::storage::pause_proposal(&creator, &admin);
            env.storage().persistent().remove(&proposal_key);
        }

        env.events().publish(
            events::trading_paused_topics(&creator),
            events::TradingPausedEvent {
                creator_id: creator,
                approver: caller,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    // =========================================================================
    // #784 — Global emergency pause
    // =========================================================================

    /// Configures the admin set authorised to trigger the global emergency pause.
    ///
    /// Only the protocol admin may call this. The set must hold 2 or 3 distinct
    /// addresses; any two of them together can toggle the global halt. Replaces
    /// any existing set. Existing pending votes are cleared so a membership
    /// change never leaves a stale approval behind.
    pub fn set_global_pause_admins(
        env: Env,
        admin: Address,
        admins: Vec<Address>,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        if admins.len() < 2 || admins.len() > 3 {
            return Err(ContractError::MultisigAdminLimitExceeded);
        }

        if let Ok(existing) = read_global_pause_admins(&env) {
            clear_global_votes(&env, &existing);
        }

        let config = MultisigAdmins { admins };
        env.storage()
            .persistent()
            .set(&constants::storage::GLOBAL_PAUSE_ADMINS, &config);

        Ok(())
    }

    /// Read-only view: the configured global emergency-pause admin set, if any.
    pub fn get_global_pause_admins(env: Env) -> Option<MultisigAdmins> {
        env.storage()
            .persistent()
            .get(&constants::storage::GLOBAL_PAUSE_ADMINS)
    }

    /// Read-only view: whether the protocol-wide emergency trading halt is active.
    pub fn get_global_trading_paused(env: Env) -> bool {
        is_global_trading_paused(&env)
    }

    /// Casts a vote to activate the global emergency pause (#784).
    ///
    /// Callable by any member of the global-pause admin set. The first admin's
    /// call only records the vote and trading continues; once a second distinct
    /// admin calls, the protocol-wide halt activates, a `global_pause_activated`
    /// event is emitted and all pending votes are cleared. A single admin can
    /// never activate the pause alone.
    ///
    /// The global halt takes precedence over per-key pause state: while it is
    /// active every `buy_key` / `sell_key` panics with `GlobalTradingHalted`.
    pub fn global_pause(env: Env, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();

        let config = read_global_pause_admins(&env)?;
        assert_global_pause_admin(&config, &caller)?;

        if is_global_trading_paused(&env) {
            return Err(ContractError::AlreadyApproved);
        }

        env.storage()
            .persistent()
            .set(&constants::storage::global_pause_vote(&caller), &true);

        if count_global_votes(&env, &config, GlobalVoteKind::Pause) < GLOBAL_PAUSE_THRESHOLD {
            return Ok(());
        }

        env.storage()
            .persistent()
            .set(&constants::storage::GLOBAL_TRADING_PAUSED, &true);
        clear_global_votes(&env, &config);

        env.events().publish(
            events::global_pause_activated_topics(&caller),
            env.ledger().sequence(),
        );

        Ok(())
    }

    /// Casts a vote to lift the global emergency pause (#784).
    ///
    /// Mirror of [`global_pause`]: callable by any member of the global-pause
    /// admin set, and the halt is lifted only once a second distinct admin
    /// approves. On the second approval a `global_pause_lifted` event is emitted
    /// and all pending votes are cleared.
    pub fn global_resume(env: Env, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();

        let config = read_global_pause_admins(&env)?;
        assert_global_pause_admin(&config, &caller)?;

        if !is_global_trading_paused(&env) {
            return Err(ContractError::ProposalNotFound);
        }

        env.storage()
            .persistent()
            .set(&constants::storage::global_resume_vote(&caller), &true);

        if count_global_votes(&env, &config, GlobalVoteKind::Resume) < GLOBAL_PAUSE_THRESHOLD {
            return Ok(());
        }

        env.storage()
            .persistent()
            .set(&constants::storage::GLOBAL_TRADING_PAUSED, &false);
        clear_global_votes(&env, &config);

        env.events().publish(
            events::global_pause_lifted_topics(&caller),
            env.ledger().sequence(),
        );

        Ok(())
    }

    // =========================================================================
    // #763 — Vesting schedule
    // =========================================================================

    /// Creates a vesting schedule for a beneficiary.
    ///
    /// Only callable by the creator. Keys vest linearly over
    /// `vesting_period_ledgers` starting from the current ledger.
    pub fn create_vesting(
        env: Env,
        creator: Address,
        beneficiary: Address,
        total_keys: u32,
        vesting_period_ledgers: u32,
    ) -> Result<(), ContractError> {
        creator.require_auth();

        let profile = read_registered_creator_profile(&env, &creator)?;
        if profile.creator != creator {
            return Err(ContractError::Unauthorized);
        }

        if total_keys == 0 || vesting_period_ledgers == 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let vesting_key = constants::storage::vesting_schedule(&creator, &beneficiary);
        if env.storage().persistent().has(&vesting_key) {
            return Err(ContractError::AlreadyRegistered);
        }

        let start_ledger = env.ledger().sequence();
        let schedule = VestingSchedule {
            beneficiary: beneficiary.clone(),
            total_keys,
            start_ledger,
            vesting_period_ledgers,
            claimed_keys: 0,
        };

        env.storage().persistent().set(&vesting_key, &schedule);

        env.events().publish(
            events::vesting_created_topics(&creator),
            events::VestingCreatedEvent {
                creator_id: creator,
                beneficiary,
                total_keys,
                start_ledger,
                vesting_period_ledgers,
            },
        );

        Ok(())
    }

    /// Claims currently vested keys for the beneficiary.
    ///
    /// Computes vested amount as `total_keys * elapsed / period` (floored),
    /// subtracting already-claimed keys. Panics with `NothingToClaim` if no
    /// new keys have vested.
    pub fn claim_vested(
        env: Env,
        creator: Address,
        beneficiary: Address,
    ) -> Result<u32, ContractError> {
        beneficiary.require_auth();

        let vesting_key = constants::storage::vesting_schedule(&creator, &beneficiary);
        let mut schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&vesting_key)
            .ok_or(ContractError::VestingNotFound)?;

        if schedule.beneficiary != beneficiary {
            return Err(ContractError::Unauthorized);
        }

        let current_ledger = env.ledger().sequence();
        if current_ledger < schedule.start_ledger {
            return Err(ContractError::VestingNotStarted);
        }

        let elapsed = current_ledger.saturating_sub(schedule.start_ledger);

        let vested_keys = if elapsed >= schedule.vesting_period_ledgers {
            schedule.total_keys
        } else {
            (schedule.total_keys as u64)
                .checked_mul(elapsed as u64)
                .ok_or(ContractError::Overflow)?
                .checked_div(schedule.vesting_period_ledgers as u64)
                .ok_or(ContractError::Overflow)? as u32
        };

        let claimable = vested_keys
            .checked_sub(schedule.claimed_keys)
            .ok_or(ContractError::NothingToClaim)?;

        if claimable == 0 {
            return Err(ContractError::NothingToClaim);
        }

        schedule.claimed_keys = schedule
            .claimed_keys
            .checked_add(claimable)
            .ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&vesting_key, &schedule);

        // Credit keys to beneficiary balance
        let balance_key = constants::storage::holder_balance_key(&creator, &beneficiary);
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        let new_balance = current_balance
            .checked_add(claimable)
            .ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&balance_key, &new_balance);

        // Update holder count if first keys
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
            events::keys_claimed_topics(&creator, &beneficiary),
            events::KeysClaimedEvent {
                creator_id: creator,
                beneficiary,
                amount: claimable,
                ledger: current_ledger,
            },
        );

        Ok(claimable)
    }

    pub fn set_circuit_breaker_threshold(
        env: Env,
        admin: Address,
        threshold: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        let key = constants::storage::CIRCUIT_BREAKER_THRESHOLD;
        env.storage().persistent().set(&key, &threshold);
        extend_key_ttl_to_full_window(&env, &key);
        Ok(())
    }

    pub fn get_referral_earnings(env: Env, address: Address) -> i128 {
        let ref_key = constants::storage::referral_earnings(&address);
        env.storage().persistent().get(&ref_key).unwrap_or(0)
    }

    pub fn enable_whitelist(env: Env, creator: Address) -> Result<(), ContractError> {
        creator.require_auth();
        let profile = read_registered_creator_profile(&env, &creator)?;
        if profile.creator != creator {
            return Err(ContractError::Unauthorized);
        }

        let mode_key = constants::storage::whitelist_mode(&creator);
        env.storage().persistent().set(&mode_key, &true);
        extend_key_ttl_to_full_window(&env, &mode_key);

        env.events().publish(
            events::whitelist_enabled_topics(&creator),
            events::WhitelistEnabledEvent { creator },
        );

        Ok(())
    }

    pub fn disable_whitelist(env: Env, creator: Address) -> Result<(), ContractError> {
        creator.require_auth();
        let profile = read_registered_creator_profile(&env, &creator)?;
        if profile.creator != creator {
            return Err(ContractError::Unauthorized);
        }

        let mode_key = constants::storage::whitelist_mode(&creator);
        env.storage().persistent().set(&mode_key, &false);
        extend_key_ttl_to_full_window(&env, &mode_key);

        env.events().publish(
            events::whitelist_disabled_topics(&creator),
            events::WhitelistDisabledEvent { creator },
        );

        Ok(())
    }

    pub fn add_to_whitelist(
        env: Env,
        creator: Address,
        address: Address,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        let profile = read_registered_creator_profile(&env, &creator)?;
        if profile.creator != creator {
            return Err(ContractError::Unauthorized);
        }

        let entry_key = constants::storage::whitelist_entry(&creator, &address);
        env.storage().persistent().set(&entry_key, &true);
        extend_key_ttl_to_full_window(&env, &entry_key);

        env.events().publish(
            events::address_whitelisted_topics(&creator),
            events::AddressWhitelistedEvent { creator, address },
        );

        Ok(())
    }

    pub fn remove_from_whitelist(
        env: Env,
        creator: Address,
        address: Address,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        let profile = read_registered_creator_profile(&env, &creator)?;
        if profile.creator != creator {
            return Err(ContractError::Unauthorized);
        }

        let entry_key = constants::storage::whitelist_entry(&creator, &address);
        env.storage().persistent().set(&entry_key, &false);
        extend_key_ttl_to_full_window(&env, &entry_key);

        env.events().publish(
            events::address_removed_topics(&creator),
            events::AddressRemovedEvent { creator, address },
        );

        Ok(())
    }

    pub fn burn(
        env: Env,
        caller: Address,
        key_id: Address,
        quantity: u32,
    ) -> Result<u32, ContractError> {
        caller.require_auth();
        assert_not_paused(&env)?;

        if quantity == 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let mut profile = read_registered_creator_profile(&env, &key_id)?;
        let balance_key = constants::storage::holder_balance_key(&key_id, &caller);
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        if current_balance < quantity {
            return Err(ContractError::InsufficientBalance);
        }

        settle_holder_dividends(&env, &key_id, &caller, current_balance)?;

        let new_balance = current_balance
            .checked_sub(quantity)
            .ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&balance_key, &new_balance);

        let new_supply = profile
            .supply
            .checked_sub(quantity)
            .ok_or(ContractError::Overflow)?;

        if current_balance > 0 && new_balance == 0 {
            profile.holder_count = profile.holder_count.saturating_sub(1);
        }

        profile.supply = new_supply;
        let profile_key = constants::storage::creator(&key_id);
        env.storage().persistent().set(&profile_key, &profile);

        write_creator_supply(&env, &key_id, new_supply);

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        let _new_price = compute_bonding_curve_price(&env, &key_id, base_price, new_supply)?;

        env.events().publish(
            events::keys_burned_topics(&key_id),
            events::KeysBurnedEvent {
                wallet: caller,
                key_id: key_id.clone(),
                quantity,
                new_supply,
            },
        );

        extend_creator_ttl(&env, &key_id);

        Ok(new_supply)
    }

    /// Read-only view: returns the vesting schedule for a beneficiary.
    pub fn get_vesting_schedule(
        env: Env,
        creator: Address,
        beneficiary: Address,
    ) -> Option<VestingSchedule> {
        env.storage()
            .persistent()
            .get(&constants::storage::vesting_schedule(
                &creator,
                &beneficiary,
            ))
    }

    // =========================================================================
    // #768 — Time-locked admin config changes
    // =========================================================================

    /// Proposes a config change that cannot execute until 48 hours have elapsed.
    ///
    /// Only callable by the protocol admin. Records the proposal with an
    /// `execution_not_before` ledger computed from the current ledger plus
    /// the 48-hour equivalent in ledgers (~34,560 at 5s/ledger).
    pub fn propose_config_change(
        env: Env,
        admin: Address,
        change_type: TimelockChangeType,
        payload: soroban_sdk::Bytes,
    ) -> Result<u32, ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        // 48 hours = 172,800 seconds / 5 seconds per ledger = 34,560 ledgers
        const TIMELOCK_DELAY_LEDGERS: u32 = 34_560;

        let next_id_key = DataKey::TimelockNextId;
        let proposal_id: u32 = env.storage().persistent().get(&next_id_key).unwrap_or(1u32);

        let current_ledger = env.ledger().sequence();
        let execution_not_before = current_ledger
            .checked_add(TIMELOCK_DELAY_LEDGERS)
            .ok_or(ContractError::Overflow)?;

        let proposal = TimelockProposal {
            change_type,
            payload,
            proposer: admin.clone(),
            proposed_at: current_ledger,
            execution_not_before,
            executed: false,
            cancelled: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::TimelockProposal(proposal_id), &proposal);

        let next_id = proposal_id.checked_add(1).ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&next_id_key, &next_id);

        env.events().publish(
            events::config_change_proposed_topics(&admin),
            events::ConfigChangeProposedEvent {
                proposal_id,
                proposer: admin,
                change_type: change_type as u32,
                proposed_at: current_ledger,
                execution_not_before,
            },
        );

        Ok(proposal_id)
    }

    /// Executes a timelocked config change after the delay has elapsed.
    ///
    /// Panics with `AllocationLocked` if called before `execution_not_before`.
    pub fn execute_config_change(
        env: Env,
        admin: Address,
        proposal_id: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        let mut proposal: TimelockProposal = env
            .storage()
            .persistent()
            .get(&DataKey::TimelockProposal(proposal_id))
            .ok_or(ContractError::NotRegistered)?;

        if proposal.executed || proposal.cancelled {
            return Err(ContractError::NotRegistered);
        }

        let current_ledger = env.ledger().sequence();
        if current_ledger < proposal.execution_not_before {
            return Err(ContractError::AllocationLocked);
        }

        proposal.executed = true;
        env.storage()
            .persistent()
            .set(&DataKey::TimelockProposal(proposal_id), &proposal);

        env.events().publish(
            (events::config_change_executed_topics(),),
            events::ConfigChangeExecutedEvent {
                proposal_id,
                executed_at: current_ledger,
            },
        );

        Ok(())
    }

    /// Cancels a pending timelock proposal before execution.
    ///
    /// Only callable by the protocol admin.
    pub fn cancel_config_change(
        env: Env,
        admin: Address,
        proposal_id: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        let mut proposal: TimelockProposal = env
            .storage()
            .persistent()
            .get(&DataKey::TimelockProposal(proposal_id))
            .ok_or(ContractError::NotRegistered)?;

        if proposal.executed || proposal.cancelled {
            return Err(ContractError::NotRegistered);
        }

        proposal.cancelled = true;
        env.storage()
            .persistent()
            .set(&DataKey::TimelockProposal(proposal_id), &proposal);

        env.events().publish(
            (events::config_change_cancelled_topics(),),
            events::ConfigChangeCancelledEvent {
                proposal_id,
                cancelled_at: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    /// Read-only view: returns a timelock proposal by ID.
    pub fn get_timelock_proposal(env: Env, proposal_id: u32) -> Option<TimelockProposal> {
        env.storage()
            .persistent()
            .get(&DataKey::TimelockProposal(proposal_id))
    }

    // =========================================================================
    // #765 — Snapshot voting weight capture
    // =========================================================================

    /// Casts a vote using the holder's balance snapshot from the proposal
    /// creation ledger, preventing post-proposal key purchases from
    /// influencing the vote.
    ///
    /// The snapshot is captured lazily on first vote: the holder's balance
    /// at the proposal's `expires_at` (used as snapshot ledger) is read
    /// from the live balance at vote time and stored. Subsequent votes
    /// reuse the stored snapshot.
    pub fn cast_vote_with_snapshot(
        env: Env,
        creator_id: Address,
        voter: Address,
        poll_id: u32,
        option_index: u32,
    ) -> Result<(), crate::events::PollError> {
        use crate::events::{PollError, PollVote, POLL_VOTE_EVENT_NAME};

        voter.require_auth();
        let mut poll = events::read_poll(&env, &creator_id, poll_id)?;

        if events::is_poll_expired(&env, &poll) {
            return Err(PollError::PollExpired);
        }
        if option_index >= poll.options.len() {
            return Err(PollError::InvalidOption);
        }

        // Check for existing snapshot; if none, capture current balance as snapshot
        let snapshot_key = DataKey::VoteSnapshot(creator_id.clone(), poll_id, voter.clone());
        let weight: u32 = if let Some(snap) = env
            .storage()
            .persistent()
            .get::<DataKey, u32>(&snapshot_key)
        {
            snap
        } else {
            let balance_key = constants::storage::holder_balance_key(&creator_id, &voter);
            let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
            if balance == 0 {
                return Err(PollError::NotAHolder);
            }
            env.storage().persistent().set(&snapshot_key, &balance);
            balance
        };

        if weight == 0 {
            return Err(PollError::NotAHolder);
        }

        // Handle re-voting: remove previous weight
        let vote_key = events::vote_storage_key(&creator_id, poll_id, &voter);
        if let Some(previous_vote) = env
            .storage()
            .persistent()
            .get::<events::PollDataKey, PollVote>(&vote_key)
        {
            let previous_count = poll
                .vote_counts
                .get(previous_vote.option_index)
                .ok_or(PollError::InvalidOption)?;
            let updated_previous_count = previous_count
                .checked_sub(previous_vote.weight)
                .ok_or(PollError::Overflow)?;
            poll.vote_counts
                .set(previous_vote.option_index, updated_previous_count);
            poll.total_weight = poll
                .total_weight
                .checked_sub(previous_vote.weight)
                .ok_or(PollError::Overflow)?;
        }

        let selected_count = poll
            .vote_counts
            .get(option_index)
            .ok_or(PollError::InvalidOption)?;
        let updated_selected_count = selected_count
            .checked_add(weight)
            .ok_or(PollError::Overflow)?;
        poll.vote_counts.set(option_index, updated_selected_count);
        poll.total_weight = poll
            .total_weight
            .checked_add(weight)
            .ok_or(PollError::Overflow)?;

        env.storage()
            .persistent()
            .set(&events::poll_storage_key(&creator_id, poll_id), &poll);
        env.storage().persistent().set(
            &vote_key,
            &PollVote {
                option_index,
                weight,
            },
        );
        env.events().publish(
            (POLL_VOTE_EVENT_NAME, creator_id, poll_id, voter),
            (option_index, weight),
        );

        Ok(())
    }

    /// Read-only view: returns the snapshot weight for a voter on a poll.
    ///
    /// Returns `None` if no snapshot exists (voter hasn't voted yet).
    pub fn get_vote_snapshot(
        env: Env,
        creator_id: Address,
        poll_id: u32,
        voter: Address,
    ) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::VoteSnapshot(creator_id, poll_id, voter))
    }

    // =========================================================================
    // #791 — Co-creator removal
    // =========================================================================

    /// Removes a creator's configured co-creator split, restoring 100% of all
    /// future royalties to the creator.
    ///
    /// Only callable by the creator (`caller` must equal `creator`).
    ///
    /// # Errors
    ///
    /// - [`FeatureError::Unauthorized`] if `caller` is not `creator`
    /// - [`FeatureError::NoCoCreatorSet`] if no co-creator is configured for `creator`
    pub fn remove_co_creator(
        env: Env,
        creator: Address,
        caller: Address,
    ) -> Result<(), FeatureError> {
        caller.require_auth();
        if caller != creator {
            return Err(FeatureError::Unauthorized);
        }

        let key = constants::storage::co_creator(&creator);
        let config: CoCreatorConfig = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(FeatureError::NoCoCreatorSet)?;

        env.storage().persistent().remove(&key);

        env.events().publish(
            events::co_creator_removed_topics(&creator),
            events::CoCreatorRemovedEvent {
                creator_id: creator,
                co_creator: config.address,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    // =========================================================================
    // #787 / #793 / #790 — Pre-launch auction phase
    // =========================================================================

    /// Configures a fixed-price pre-launch auction phase for a creator's keys.
    ///
    /// While `total_supply` is below `auction_supply`, [`Self::buy_key`] sells
    /// at `auction_price` instead of the bonding curve price; the contract
    /// transitions back to the curve automatically once the auction supply is
    /// exhausted. Only callable by the creator, and only before any keys have
    /// been sold.
    ///
    /// # Errors
    ///
    /// - [`FeatureError::Unauthorized`] if `caller` is not `creator`
    /// - [`FeatureError::NotRegistered`] if `creator` is not a registered creator
    /// - [`FeatureError::AuctionAlreadyStarted`] if the creator's supply is already nonzero
    /// - [`FeatureError::NotPositiveAmount`] if `auction_price` is not positive
    /// - [`FeatureError::InvalidAuctionConfig`] if `auction_supply` is zero or exceeds
    ///   [`MAX_AUCTION_SUPPLY`]
    pub fn configure_auction(
        env: Env,
        creator: Address,
        caller: Address,
        auction_price: i128,
        auction_supply: u32,
    ) -> Result<(), FeatureError> {
        caller.require_auth();
        if caller != creator {
            return Err(FeatureError::Unauthorized);
        }

        let profile = read_registered_creator_profile(&env, &creator)
            .map_err(|_| FeatureError::NotRegistered)?;
        if profile.supply > 0 {
            return Err(FeatureError::AuctionAlreadyStarted);
        }
        if auction_price <= 0 {
            return Err(FeatureError::NotPositiveAmount);
        }
        if auction_supply == 0 || auction_supply > MAX_AUCTION_SUPPLY {
            return Err(FeatureError::InvalidAuctionConfig);
        }

        let config = AuctionConfig {
            auction_price,
            auction_supply,
            auction_sold: 0,
        };
        let key = constants::storage::auction_config(&creator);
        env.storage().persistent().set(&key, &config);
        extend_key_ttl_to_full_window(&env, &key);

        env.events().publish(
            events::auction_configured_topics(&creator),
            events::AuctionConfiguredEvent {
                creator_id: creator,
                auction_price,
                auction_supply,
            },
        );

        Ok(())
    }

    /// Cancels a creator's configured auction before any auction keys have sold.
    ///
    /// Only callable by the creator.
    ///
    /// # Errors
    ///
    /// - [`FeatureError::Unauthorized`] if `caller` is not `creator`
    /// - [`FeatureError::NoAuctionConfigured`] if no auction config exists for `creator`
    /// - [`FeatureError::AuctionAlreadyStarted`] if `auction_sold` is greater than zero
    pub fn cancel_auction(env: Env, creator: Address, caller: Address) -> Result<(), FeatureError> {
        caller.require_auth();
        if caller != creator {
            return Err(FeatureError::Unauthorized);
        }

        let key = constants::storage::auction_config(&creator);
        let config: AuctionConfig = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(FeatureError::NoAuctionConfigured)?;

        if config.auction_sold > 0 {
            return Err(FeatureError::AuctionAlreadyStarted);
        }

        env.storage().persistent().remove(&key);

        env.events().publish(
            events::auction_cancelled_topics(&creator),
            events::AuctionCancelledEvent {
                creator_id: creator,
                auction_price: config.auction_price,
                auction_supply: config.auction_supply,
            },
        );

        Ok(())
    }

    /// Read-only view: returns the configured auction state for a creator, if any.
    pub fn get_auction_config(env: Env, creator: Address) -> Option<AuctionConfig> {
        env.storage()
            .persistent()
            .get(&constants::storage::auction_config(&creator))
    }

    // =========================================================================
    // #786 / #789 — Staking reward claim
    // =========================================================================

    /// Unlocks a holder's staked keys after the lock period and pays out
    /// their pro-rata share of the creator's staking rewards pool.
    ///
    /// Callable by any wallet with an active stake for `creator`. The reward
    /// is `staking_pool_balance * staked_quantity / total_staked_quantity`,
    /// capped at the pool's current balance. On success the full staked
    /// quantity is unlocked back into the holder's liquid balance.
    ///
    /// # Errors
    ///
    /// - [`FeatureError::NoStakeFound`] if the caller has no active stake for `creator`
    /// - [`FeatureError::StakeLockActive`] if the current ledger is before the unlock ledger
    /// - [`FeatureError::ProtocolPaused`] if the contract is paused
    pub fn claim_stake_reward(
        env: Env,
        creator: Address,
        holder: Address,
    ) -> Result<i128, FeatureError> {
        holder.require_auth();
        assert_not_paused(&env).map_err(|_| FeatureError::ProtocolPaused)?;

        let staked_balance_key = constants::storage::staked_balance(&creator, &holder);
        let staked_quantity: u32 = env
            .storage()
            .persistent()
            .get(&staked_balance_key)
            .unwrap_or(0);
        if staked_quantity == 0 {
            return Err(FeatureError::NoStakeFound);
        }

        let unlock_key = constants::storage::stake_unlock_ledger(&creator, &holder);
        let unlock_ledger: u32 = env
            .storage()
            .persistent()
            .get(&unlock_key)
            .ok_or(FeatureError::NoStakeFound)?;
        if env.ledger().sequence() < unlock_ledger {
            return Err(FeatureError::StakeLockActive);
        }

        let total_staked = read_total_staked(&env, &creator);
        let pool_balance = read_staking_rewards_pool(&env, &creator);
        let reward = if total_staked == 0 || pool_balance <= 0 {
            0
        } else {
            let raw_share = pool_balance
                .checked_mul(i128::from(staked_quantity))
                .ok_or(FeatureError::Overflow)?
                / i128::from(total_staked);
            raw_share.min(pool_balance)
        };

        // Unlock: clear the stake and its lock record, mirroring `unstake_keys`.
        env.storage().persistent().remove(&staked_balance_key);
        env.storage().persistent().remove(&unlock_key);

        let total_staked_key = constants::storage::total_staked(&creator);
        let new_total_staked = total_staked.saturating_sub(staked_quantity);
        if new_total_staked == 0 {
            env.storage().persistent().remove(&total_staked_key);
        } else {
            env.storage()
                .persistent()
                .set(&total_staked_key, &new_total_staked);
        }

        if reward > 0 {
            let pool_key = constants::storage::staking_rewards_pool(&creator);
            let remaining_pool = pool_balance.saturating_sub(reward);
            env.storage().persistent().set(&pool_key, &remaining_pool);
        }

        env.events().publish(
            events::stake_reward_claimed_topics(&creator, &holder),
            events::StakeRewardClaimedEvent {
                wallet: holder,
                key_id: creator,
                quantity_unlocked: staked_quantity,
                reward_amount: reward,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(reward)
    }

    /// Read-only view: returns the total keys currently staked across all
    /// holders for a creator.
    pub fn get_total_staked(env: Env, creator: Address) -> u32 {
        read_total_staked(&env, &creator)
    }

    /// Read-only view: returns the current staking rewards pool balance for a creator.
    pub fn get_staking_rewards_pool(env: Env, creator: Address) -> i128 {
        read_staking_rewards_pool(&env, &creator)
    }

    /// Read-only view: returns the ledger sequence at which a holder's stake
    /// for a creator unlocks, if the holder has an active stake.
    pub fn get_stake_unlock_ledger(env: Env, creator: Address, holder: Address) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&constants::storage::stake_unlock_ledger(&creator, &holder))
    }

    // =========================================================================
    // #758 — Batch buy
    // =========================================================================

    /// Executes multiple key purchases in a single transaction.
    ///
    /// Each entry in `orders` is a `(creator, quantity)` pair processed
    /// sequentially. The caller pays the sum of all individual buy costs.
    /// Returns a vector of [`BatchBuyOrderResult`] with the per-order outcome.
    ///
    /// # Errors
    ///
    /// - [`ContractError::SupplyCapExceeded`] if `orders` is empty or exceeds
    ///   [`MAX_BATCH_BUY_SIZE`].
    pub fn batch_buy(
        env: Env,
        buyer: Address,
        orders: Vec<(Address, u32)>,
    ) -> Result<Vec<BatchBuyOrderResult>, ContractError> {
        buyer.require_auth();
        assert_global_trading_not_halted(&env)?;
        assert_not_paused(&env)?;
        assert_not_blacklisted(&env, &buyer)?;

        if orders.is_empty() || orders.len() as usize > MAX_BATCH_BUY_SIZE {
            return Err(ContractError::SupplyCapExceeded);
        }

        let mut results = Vec::new(&env);
        for order in orders.iter() {
            let (creator, quantity) = order;
            if quantity == 0 {
                return Err(ContractError::NotPositiveAmount);
            }
            let mut paid: i128 = 0;
            for _ in 0..quantity {
                // Resolve the per-key quote price so we can forward the
                // correct payment to buy_key and track cumulative cost.
                let per_key_price = resolve_buy_quote_price(&env, &creator)?
                    .ok_or(ContractError::KeyPriceNotSet)?;
                let _ = Self::buy_key_impl(
                    env.clone(),
                    creator.clone(),
                    buyer.clone(),
                    per_key_price,
                    None,
                    None,
                )?;
                paid = paid
                    .checked_add(per_key_price)
                    .ok_or(ContractError::Overflow)?;
            }
            results.push_back(BatchBuyOrderResult {
                creator: creator.clone(),
                quantity,
                price_paid: paid,
            });
        }
        Ok(results)
    }

    // =========================================================================
    // #755 — Royalty configuration
    // =========================================================================

    /// Configures creator-specific royalty fees for buys and sells.
    ///
    /// Each fee is capped at [`MAX_ROYALTY_BPS`]. Only callable by a
    /// registered creator for their own key.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotRegistered`] if `creator` is not registered
    /// - [`ContractError::ProtocolFeeExceedsCap`] if either fee exceeds [`MAX_ROYALTY_BPS`]
    /// - [`ContractError::Unauthorized`] if `caller` is not the creator
    pub fn set_royalty(
        env: Env,
        creator: Address,
        buy_fee_bps: u32,
        sell_fee_bps: u32,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        assert_not_paused(&env)?;
        let profile = read_registered_creator_profile(&env, &creator)?;
        if profile.creator != creator {
            return Err(ContractError::Unauthorized);
        }
        if buy_fee_bps > MAX_ROYALTY_BPS || sell_fee_bps > MAX_ROYALTY_BPS {
            return Err(ContractError::ProtocolFeeExceedsCap);
        }
        let config = RoyaltyConfig {
            buy_fee_bps,
            sell_fee_bps,
        };
        let key = constants::storage::royalty_config(&creator);
        env.storage().persistent().set(&key, &config);
        extend_key_ttl_to_full_window(&env, &key);
        Ok(())
    }

    /// Read-only view: returns the royalty configuration for a creator, if set.
    pub fn get_royalty_config(env: Env, creator: Address) -> Option<RoyaltyConfig> {
        read_royalty_config(&env, &creator)
    }

    // =========================================================================
    // #756 — Curve migration
    // =========================================================================

    /// Migrates a creator's bonding curve to a new exponent.
    ///
    /// Only callable by the protocol admin. The exponent must be between 1
    /// and 5 (inclusive). Stores the exponent per-creator so different keys
    /// can run different curve shapes.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `caller` is not the protocol admin
    /// - [`ContractError::InvalidFeeConfig`] if `exponent` is 0 or > 5
    pub fn migrate_curve(
        env: Env,
        caller: Address,
        exponent: u32,
        key_ids: Vec<Address>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .persistent()
            .get(&constants::storage::ADMIN_ADDRESS)
            .ok_or(ContractError::Unauthorized)?;
        if caller != admin {
            return Err(ContractError::Unauthorized);
        }
        if exponent == 0 || exponent > 5 {
            return Err(ContractError::InvalidFeeConfig);
        }
        for creator_id in key_ids.iter() {
            let key = constants::storage::curve_exponent(&creator_id);
            env.storage().persistent().set(&key, &exponent);
            extend_key_ttl_to_full_window(&env, &key);
        }
        Ok(())
    }

    /// Read-only view: returns the curve exponent for a creator, if set.
    pub fn get_curve_exponent(env: Env, creator: Address) -> Option<u32> {
        read_curve_exponent(&env, &creator)
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

        // Protocol Cap Exceeded (PROTOCOL_BPS_MAX = 10000)
        assert_eq!(
            fee::assert_valid_fee_bps(0, 10001),
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
        assert!(!fee::validate_fee_bps(0, 10001));

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

    // --- TTL extension threshold unit tests (#605) ---

    #[test]
    fn test_ttl_extension_triggers_below_threshold() {
        // TTL at threshold minus 1 ledger: extension should be triggered.
        assert!(super::ttl::should_extend(99, 100));
    }

    #[test]
    fn test_ttl_extension_not_triggered_at_threshold_boundary() {
        // TTL exactly at threshold: extension should not be triggered (boundary is exclusive).
        assert!(!super::ttl::should_extend(100, 100));
    }

    #[test]
    fn test_ttl_extension_not_triggered_above_threshold() {
        // TTL at threshold plus 100 ledgers: extension should not be triggered.
        assert!(!super::ttl::should_extend(200, 100));
    }

    #[test]
    fn test_extended_ttl_equals_configured_extension_amount() {
        const THRESHOLD: u32 = 100;
        let current_ttl = THRESHOLD - 1;

        let new_ttl = if super::ttl::should_extend(current_ttl, THRESHOLD) {
            super::CREATOR_TTL_LEDGERS
        } else {
            current_ttl
        };

        assert_eq!(new_ttl, super::CREATOR_TTL_LEDGERS);
    }

    // --- read_creator_fee_recipient / write_creator_fee_recipient helpers (#603) ---

    #[test]
    fn test_read_creator_fee_recipient_returns_none_for_unset_creator() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let creator = Address::generate(&env);

        let recipient = env.as_contract(&contract_id, || {
            super::read_creator_fee_recipient(&env, &creator)
        });

        assert_eq!(
            recipient, None,
            "unregistered creator should have no fee recipient"
        );
    }

    #[test]
    fn test_read_creator_fee_recipient_returns_address_after_write() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let creator = Address::generate(&env);
        let recipient_a = Address::generate(&env);

        let profile = super::CreatorProfile {
            creator: creator.clone(),
            handle: soroban_sdk::String::from_str(&env, "alice"),
            supply: 0,
            holder_count: 0,
            fee_recipient: recipient_a.clone(),
            registered_at: 0,
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&super::constants::storage::creator(&creator), &profile);

            let read = super::read_creator_fee_recipient(&env, &creator);
            assert_eq!(
                read,
                Some(recipient_a),
                "should return address A after write"
            );
        });
    }

    #[test]
    fn test_overwrite_creator_fee_recipient_replaces_old_address() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let creator = Address::generate(&env);
        let recipient_a = Address::generate(&env);
        let recipient_b = Address::generate(&env);

        let profile = super::CreatorProfile {
            creator: creator.clone(),
            handle: soroban_sdk::String::from_str(&env, "alice"),
            supply: 0,
            holder_count: 0,
            fee_recipient: recipient_a.clone(),
            registered_at: 0,
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&super::constants::storage::creator(&creator), &profile);

            super::write_creator_fee_recipient(&env, &creator, &recipient_b);

            let read = super::read_creator_fee_recipient(&env, &creator);
            assert_eq!(
                read,
                Some(recipient_b),
                "should return address B after overwrite"
            );
            assert_ne!(read, Some(recipient_a), "should no longer return address A");
        });
    }

    #[test]
    fn test_two_creators_store_independent_recipient_addresses() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let creator_1 = Address::generate(&env);
        let creator_2 = Address::generate(&env);
        let recipient_1 = Address::generate(&env);
        let recipient_2 = Address::generate(&env);

        let profile_1 = super::CreatorProfile {
            creator: creator_1.clone(),
            handle: soroban_sdk::String::from_str(&env, "alice"),
            supply: 0,
            holder_count: 0,
            fee_recipient: recipient_1.clone(),
            registered_at: 0,
        };
        let profile_2 = super::CreatorProfile {
            creator: creator_2.clone(),
            handle: soroban_sdk::String::from_str(&env, "bob"),
            supply: 0,
            holder_count: 0,
            fee_recipient: recipient_2.clone(),
            registered_at: 0,
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&super::constants::storage::creator(&creator_1), &profile_1);
            env.storage()
                .persistent()
                .set(&super::constants::storage::creator(&creator_2), &profile_2);

            let read_1 = super::read_creator_fee_recipient(&env, &creator_1);
            let read_2 = super::read_creator_fee_recipient(&env, &creator_2);

            assert_eq!(
                read_1,
                Some(recipient_1),
                "creator 1 should have recipient 1"
            );
            assert_eq!(
                read_2,
                Some(recipient_2),
                "creator 2 should have recipient 2"
            );
            assert_ne!(
                read_1, read_2,
                "two creators must not share the same recipient value"
            );
        });
    }

    #[test]
    fn test_write_creator_fee_recipient_triple_overwrite_replaces_previous() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let creator = Address::generate(&env);
        let recipient_a = Address::generate(&env);
        let recipient_b = Address::generate(&env);
        let recipient_c = Address::generate(&env);

        let profile = super::CreatorProfile {
            creator: creator.clone(),
            handle: soroban_sdk::String::from_str(&env, "alice"),
            supply: 0,
            holder_count: 0,
            fee_recipient: recipient_a.clone(),
            registered_at: 0,
        };

        env.as_contract(&contract_id, || {
            // Write initial profile with recipient A
            env.storage()
                .persistent()
                .set(&super::constants::storage::creator(&creator), &profile);

            // First overwrite: A -> B
            super::write_creator_fee_recipient(&env, &creator, &recipient_b);
            let read_after_b = super::read_creator_fee_recipient(&env, &creator);
            assert_eq!(
                read_after_b.clone(),
                Some(recipient_b.clone()),
                "should return address B after overwrite A -> B"
            );
            assert_ne!(
                read_after_b,
                Some(recipient_a.clone()),
                "should no longer return address A after B overwrite"
            );

            // Second overwrite: B -> C
            super::write_creator_fee_recipient(&env, &creator, &recipient_c);
            let read_after_c = super::read_creator_fee_recipient(&env, &creator);
            assert_eq!(
                read_after_c.clone(),
                Some(recipient_c.clone()),
                "should return address C after overwrite B -> C"
            );
            assert_ne!(
                read_after_c.clone(),
                Some(recipient_b.clone()),
                "should no longer return address B after C overwrite"
            );
            assert_ne!(
                read_after_c,
                Some(recipient_a.clone()),
                "should no longer return address A after C overwrite"
            );
        });
    }

    #[test]
    fn test_write_creator_fee_recipient_replaces_single_storage_entry() {
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let creator = Address::generate(&env);
        let recipient_a = Address::generate(&env);
        let recipient_b = Address::generate(&env);
        let recipient_c = Address::generate(&env);

        let profile = super::CreatorProfile {
            creator: creator.clone(),
            handle: soroban_sdk::String::from_str(&env, "alice"),
            supply: 0,
            holder_count: 0,
            fee_recipient: recipient_a.clone(),
            registered_at: 0,
        };

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&super::constants::storage::creator(&creator), &profile);

            // Overwrite twice
            super::write_creator_fee_recipient(&env, &creator, &recipient_b);
            super::write_creator_fee_recipient(&env, &creator, &recipient_c);

            // Read the full profile directly — the fee_recipient field must be C,
            // not an accumulation of all three addresses.
            let stored: super::CreatorProfile = env
                .storage()
                .persistent()
                .get(&super::constants::storage::creator(&creator))
                .expect("creator profile should exist");

            assert_eq!(
                stored.fee_recipient, recipient_c,
                "profile.fee_recipient should be the most recently written address (C)"
            );
            assert_ne!(
                stored.fee_recipient, recipient_a,
                "profile.fee_recipient should not be the overwritten address A"
            );
            assert_ne!(
                stored.fee_recipient, recipient_b,
                "profile.fee_recipient should not be the overwritten address B"
            );
        });
    }

    // --- write_creator_supply helper tests ---

    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    #[test]
    fn test_write_creator_supply_overwrites_existing_value() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let client = super::CreatorKeysContractClient::new(&env, &contract_id);
        let creator = Address::generate(&env);

        client.register_creator(
            &super::RegisterCreatorParams {
                creator: creator.clone(),
                handle: String::from_str(&env, "alice"),
            },
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        let supply = env.as_contract(&contract_id, || {
            super::write_creator_supply(&env, &creator, 5);
            super::write_creator_supply(&env, &creator, 3);
            super::read_creator_supply(&env, &creator)
        });
        assert_eq!(
            supply, 3,
            "overwrite should replace previous value 5 with 3"
        );
    }

    #[test]
    fn test_write_creator_supply_zero_explicitly_stored() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let client = super::CreatorKeysContractClient::new(&env, &contract_id);
        let creator = Address::generate(&env);

        client.register_creator(
            &super::RegisterCreatorParams {
                creator: creator.clone(),
                handle: String::from_str(&env, "bob"),
            },
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        let supply = env.as_contract(&contract_id, || {
            super::write_creator_supply(&env, &creator, 5);
            super::write_creator_supply(&env, &creator, 0);
            super::read_creator_supply(&env, &creator)
        });
        assert_eq!(
            supply, 0,
            "writing zero should return zero, not previous value 5"
        );
    }

    #[test]
    fn test_write_creator_supply_independent_per_creator() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let client = super::CreatorKeysContractClient::new(&env, &contract_id);
        let creator_a = Address::generate(&env);
        let creator_b = Address::generate(&env);

        client.register_creator(
            &super::RegisterCreatorParams {
                creator: creator_a.clone(),
                handle: String::from_str(&env, "alice"),
            },
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );
        client.register_creator(
            &super::RegisterCreatorParams {
                creator: creator_b.clone(),
                handle: String::from_str(&env, "bob"),
            },
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        let (supply_a, supply_b) = env.as_contract(&contract_id, || {
            super::write_creator_supply(&env, &creator_a, 10);
            super::write_creator_supply(&env, &creator_b, 20);
            (
                super::read_creator_supply(&env, &creator_a),
                super::read_creator_supply(&env, &creator_b),
            )
        });
        assert_eq!(supply_a, 10, "creator A should hold its independent value");
        assert_eq!(supply_b, 20, "creator B should hold its independent value");
    }

    #[test]
    fn test_write_creator_supply_zero_does_not_panic() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let client = super::CreatorKeysContractClient::new(&env, &contract_id);
        let creator = Address::generate(&env);

        client.register_creator(
            &super::RegisterCreatorParams {
                creator: creator.clone(),
                handle: String::from_str(&env, "alice"),
            },
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        let supply = env.as_contract(&contract_id, || {
            super::write_creator_supply(&env, &creator, 5);
            super::write_creator_supply(&env, &creator, 0);
            super::read_creator_supply(&env, &creator)
        });
        assert_eq!(
            supply, 0,
            "read after zero write should return 0 without error"
        );
    }

    // --- read_creator_supply zero-supply unit tests (#625) ---
    //
    // Distinguishes a creator that was never written (no profile in storage)
    // from a creator whose supply was explicitly written as 0 (profile
    // present, supply == 0), and confirms every path returns 0 without
    // panicking or returning an error.

    #[test]
    fn test_read_creator_supply_returns_zero_for_creator_never_written() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let contract_id = env.register(super::CreatorKeysContract, ());

        let supply = env.as_contract(&contract_id, || super::read_creator_supply(&env, &creator));

        assert_eq!(
            supply, 0,
            "a creator with no stored profile should read as 0"
        );
    }

    #[test]
    fn test_read_creator_supply_returns_zero_after_explicit_zero_write() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let client = super::CreatorKeysContractClient::new(&env, &contract_id);
        let creator = Address::generate(&env);

        client.register_creator(
            &super::RegisterCreatorParams {
                creator: creator.clone(),
                handle: String::from_str(&env, "zerowriter"),
            },
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        let supply = env.as_contract(&contract_id, || {
            super::write_creator_supply(&env, &creator, 0);
            super::read_creator_supply(&env, &creator)
        });

        assert_eq!(
            supply, 0,
            "explicitly writing 0 should read back as 0, just like the never-written case"
        );
    }

    #[test]
    fn test_read_creator_supply_returns_zero_after_overwrite_from_nonzero() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let client = super::CreatorKeysContractClient::new(&env, &contract_id);
        let creator = Address::generate(&env);

        client.register_creator(
            &super::RegisterCreatorParams {
                creator: creator.clone(),
                handle: String::from_str(&env, "overwritetozero"),
            },
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        let supply = env.as_contract(&contract_id, || {
            super::write_creator_supply(&env, &creator, 5);
            super::write_creator_supply(&env, &creator, 0);
            super::read_creator_supply(&env, &creator)
        });

        assert_eq!(
            supply, 0,
            "overwriting a non-zero supply with 0 should read back as 0, not the stale value 5"
        );
    }

    // --- creator fee bps computation unit tests (#580) ---

    #[test]
    fn test_creator_fee_500_bps_on_1000_returns_50() {
        // 500 bps = 5%; protocol_bps=500, creator_bps=9500
        // creator_fee = price - protocol_fee = 1000 - 50 = 950
        // But the issue asks for the fee amount at 500 bps on 1000 → 50
        // apply_percentage_fee computes: 1000 * 500 / 10000 = 50
        assert_eq!(fee::apply_percentage_fee(1000, 500), Some(50));
    }

    #[test]
    fn test_creator_fee_250_bps_on_1000_returns_25() {
        assert_eq!(fee::apply_percentage_fee(1000, 250), Some(25));
    }

    #[test]
    fn test_creator_fee_100_bps_on_999_floors_to_9() {
        // 999 * 100 / 10000 = 9.99 → floor = 9
        assert_eq!(fee::apply_percentage_fee(999, 100), Some(9));
    }

    #[test]
    fn test_creator_fee_0_bps_always_returns_0() {
        assert_eq!(fee::apply_percentage_fee(1000, 0), Some(0));
        assert_eq!(fee::apply_percentage_fee(1, 0), Some(0));
        assert_eq!(fee::apply_percentage_fee(i128::MAX / 10000, 0), Some(0));
    }

    // --- read_protocol_fee_bps uninitialized panic unit tests (#646) ---

    #[test]
    #[should_panic(
        expected = "read_protocol_fee_bps: contract is uninitialized (protocol_fee_bps not set)"
    )]
    fn test_read_protocol_fee_bps_panics_when_uninitialized() {
        let env = Env::default();
        let contract_id = env.register(super::CreatorKeysContract, ());

        env.as_contract(&contract_id, || {
            super::read_protocol_fee_bps(&env);
        });
    }

    #[test]
    fn test_read_protocol_fee_bps_succeeds_when_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let client = super::CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &9000, &1000);

        let bps = env.as_contract(&contract_id, || super::read_protocol_fee_bps(&env));
        assert_eq!(bps, 1000, "must return stored protocol_fee_bps");
    }

    // --- retention policy unit tests (#724) ---

    #[test]
    fn test_read_retention_policy_returns_default_when_unset() {
        let env = Env::default();
        let contract_id = env.register(super::CreatorKeysContract, ());

        let policy = env.as_contract(&contract_id, || super::read_retention_policy(&env));
        assert_eq!(
            policy.retention_days,
            super::retention::DEFAULT_RETENTION_DAYS
        );
        assert_eq!(
            policy.partition_strategy,
            super::retention::DEFAULT_PARTITION_STRATEGY
        );
        assert_eq!(
            policy.compression_enabled,
            super::retention::DEFAULT_COMPRESSION_ENABLED
        );
        assert_eq!(policy.batch_size, super::retention::DEFAULT_BATCH_SIZE);
    }

    #[test]
    fn test_get_retention_policy_view_returns_configured_values() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(super::CreatorKeysContract, ());
        let client = super::CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.set_protocol_admin(&admin, &admin);
        client.set_retention_policy(
            &admin,
            &90u32,
            &super::PartitionStrategy::Monthly,
            &false,
            &500u32,
        );

        let policy = client.get_retention_policy();
        assert_eq!(policy.retention_days, 90);
        assert_eq!(policy.partition_strategy, super::PartitionStrategy::Monthly);
        assert!(!policy.compression_enabled);
        assert_eq!(policy.batch_size, 500);
    }
}

#[cfg(test)]
mod test_issues;
