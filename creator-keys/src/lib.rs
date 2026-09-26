#![no_std]
#![allow(clippy::enum_variant_names)] // `contracttype` macro-generated enums share prefixes by design
pub mod quote_view_errors;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, String, Vec,
};

pub mod events;
pub mod test_feature_impl;
pub mod test_new_features;

// Contract error variants stability and ordering:
//
// IMPORTANT: New error variants MUST be appended to the end of this enum and NEVER
// inserted mid-enum. The numeric discriminant values are part of the contract's ABI and
// are exposed to clients, indexers, and monitoring tools.
//
// Consequences of Reordering:
// If a variant is inserted mid-enum or existing variants are reordered:
// - Existing clients that match on numeric error codes will break
// - Indexers and monitoring tools will misinterpret error types
// - Historical error logs will become inconsistent with current definitions
// - Contract upgrades will introduce silent behavioral changes
//
// Safe Extension Pattern:
// Append new variants at the end.
// Contract error variants.
#[contracterror(export = false)]
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
    CooldownActive = 36,
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
    NotWhitelisted = 49,
    CircuitBreakerTriggered = 50,
    MaxHoldingExceeded = 51,
    LockupPeriodActive = 52,
    InvalidHolderCap = 53,
    GlobalTradingHalted = 54,
    FlashLoanDetected = 55,
    FreezeQuantityExceedsBalance = 56,
    SnapshotHolderLimitExceeded = 57,
    SnapshotAlreadyExists = 58,
    SplitTooHigh = 59,
    NameTooLong = 60,
    BioTooLong = 61,
    KeyAlreadyInitialised = 62,
    /// The key has been deprecated by its creator; new buys are no longer accepted.
    KeyDeprecated = 63,
    /// The creator did not provide enough XLM to cover the full buyback escrow
    /// (`circulating_supply * buyback_price_per_key`).
    InsufficientEscrow = 64,
    /// The requested buy quantity exceeds the per-transaction max set by the creator.
    QuantityExceedsLimit = 65,
    /// The max buy quantity value is above the allowed ceiling (10 000).
    LimitTooHigh = 66,
    /// The caller is not in the approved-caller allowlist for the price oracle.
    CallerNotApproved = 67,
    /// Emitted when a `batch_transfer_keys` call contains more than the allowed
    /// number of `(recipient, quantity)` pairs.
    BatchTransferSizeExceeded = 68,
    /// Emitted when a `batch_transfer_keys` call contains a recipient address
    /// that is the same as the sender (self-transfer inside a batch).
    InvalidRecipient = 69,
    /// Emitted when a `batch_sell` call contains fewer than 1 or more than 5 orders.
    BatchSizeExceeded = 70,
    /// The requested holder snapshot does not exist.
    SnapshotNotFound = 71,
    /// The sender's keys are frozen and cannot be transferred.
    FrozenPosition = 72,
    /// The requested buy cooldown exceeds `MAX_BUY_COOLDOWN_LEDGERS` at registration.
    InvalidCooldown = 73,
    /// `execute_action` was called before the timelock delay elapsed.
    TimelockNotElapsed = 74,
    /// The timelocked action was already executed or cancelled.
    ActionNotPending = 75,
    /// The timelock delay must be between 1 second and 30 days.
    InvalidTimelockDelay = 76,
    /// No oracle price has been published yet.
    OraclePriceNotSet = 77,
    /// Vault deposit or withdraw input vectors differ in length.
    InvalidVaultInput = 78,
    /// The configured spread exceeds the maximum allowed (`MAX_SPREAD_BPS`).
    SpreadExceedsMax = 79,
    /// The fee router address has not been configured.
    FeeRouterNotSet = 80,
    /// The spread basis-points value is invalid (reserved for future validation).
    InvalidSpreadConfig = 81,
    /// `redeem` was called on a key that has not been deprecated by its creator.
    KeyNotDeprecated = 82,
    /// A timed pause duration was invalid: `pause_with_expiry` requires
    /// `duration_ledgers` in the inclusive range `1..=17_280`.
    PauseTooLong = 83,
}

/// Errors raised by the staking entrypoints
/// ([`CreatorKeysContract::stake_keys_locked`], [`CreatorKeysContract::stake_extend`],
/// [`CreatorKeysContract::early_unstake`] and [`CreatorKeysContract::claim_stake_reward`]).
///
/// Kept separate from [`ContractError`] because Soroban caps `#[contracterror]`
/// enums at 50 variants and `ContractError` is already at that limit.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StakingError {
    /// Arithmetic overflow.
    Overflow = 1,
    /// The requested amount (or lock period) was zero.
    NotPositiveAmount = 2,
    /// The holder's liquid (non-staked) balance is smaller than the staked amount.
    InsufficientBalance = 3,
    /// No staking position exists for the given `(creator, holder, stake_id)`.
    PositionNotFound = 4,
    /// The position is no longer locked, so `early_unstake` cannot be used.
    PositionNotLocked = 5,
    /// The position is still locked, so `claim_stake_reward` cannot be used yet.
    PositionLocked = 6,
    /// The creator is not registered.
    NotRegistered = 7,
    /// The contract is paused.
    ProtocolPaused = 8,
}

/// Errors raised by co-creator and auction lifecycle entrypoints.
///
/// Kept separate from [`ContractError`] because that enum is already at the
/// Soroban 50-variant cap.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FeatureError {
    /// The `caller` is not the `creator` owning the key.
    Unauthorized = 1,
    /// `remove_co_creator` was called but no co-creator is configured.
    NoCoCreatorSet = 2,
    /// The creator is not registered.
    NotRegistered = 3,
    /// An auction is already in progress (supply > 0) and cannot be configured or cancelled.
    AuctionAlreadyStarted = 4,
    /// A price or quantity was zero when a positive value was required.
    NotPositiveAmount = 5,
    /// The auction supply was outside the valid range (1..=10_000).
    InvalidAuctionConfig = 6,
    /// `cancel_auction` or `buy_key` (auction path) but no auction is configured.
    NoAuctionConfigured = 7,
}

/// Errors raised by the buy-cooldown entrypoints
/// ([`CreatorKeysContract::set_buy_cooldown`], [`CreatorKeysContract::buy_key`]).
///
/// Kept separate from [`ContractError`] because Soroban caps `#[contracterror]`
/// enums at 50 variants and `ContractError` is already at that limit.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CooldownError {
    /// The buyer's last purchase was too recent; the per-key cooldown period
    /// has not yet elapsed.
    CooldownActive = 1,
    /// The requested cooldown exceeds the maximum of 720 ledgers (~1 hour).
    CooldownTooLong = 2,
    /// The creator address is not registered.
    NotRegistered = 3,
}

/// Errors raised by the reputation-scoring entrypoints
/// ([`CreatorKeysContract::get_reputation`], [`CreatorKeysContract::apply_governance_violation`]).
///
/// Kept separate from [`ContractError`] because Soroban caps `#[contracterror]`
/// enums at 50 variants and `ContractError` is already at that limit.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReputationError {
    /// Arithmetic overflow while accumulating a reputation delta.
    Overflow = 1,
    /// The creator address is not registered.
    NotRegistered = 2,
    /// The caller is not the protocol admin.
    Unauthorized = 3,
    /// The supplied violation penalty is not positive.
    NotPositiveAmount = 4,
}

/// Errors raised by the allowance entrypoints
/// ([`CreatorKeysContract::approve`], [`CreatorKeysContract::transfer_from`]).
///
/// Kept separate from [`ContractError`] because Soroban caps `#[contracterror]`
/// enums at 50 variants and `ContractError` is already at that limit.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AllowanceError {
    /// Arithmetic overflow while decrementing a balance or allowance.
    Overflow = 1,
    /// The requested transfer amount was zero.
    ZeroAmount = 2,
    /// The spender attempted to transfer to itself.
    SelfTransfer = 3,
    /// The owner's live (non-frozen) key balance is smaller than the amount.
    InsufficientBalance = 4,
    /// The spender's approved allowance is smaller than the amount.
    InsufficientAllowance = 5,
    /// The creator address is not registered.
    NotRegistered = 6,
    /// The contract is paused.
    ProtocolPaused = 7,
    /// The sender's keys are frozen and cannot be transferred.
    FrozenPosition = 8,
    /// The recipient would exceed the creator's per-wallet holding cap.
    HoldingCapExceeded = 9,
    /// The spender address was the zero address.
    ZeroAddress = 10,
    /// The sender is still inside the creator's post-buy cooldown window.
    CooldownActive = 11,
}

/// Errors raised by the sell-tax entrypoints
/// ([`CreatorKeysContract::set_sell_tax_bps`], [`CreatorKeysContract::get_sell_tax_bps`]).
///
/// Kept separate from [`ContractError`] because Soroban caps `#[contracterror]`
/// enums at 50 variants and `ContractError` is already at that limit.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SellTaxError {
    /// The caller is not the creator owning the key.
    Unauthorized = 1,
    /// The creator address is not registered.
    NotRegistered = 2,
    /// The requested tax exceeds the protocol ceiling (`MAX_SELL_TAX_BPS`).
    TaxExceedsMax = 3,
    /// Arithmetic overflow while accruing the tax into the buyback pool.
    Overflow = 4,
}

/// Errors raised by the poll quorum-escalation entrypoints
/// ([`CreatorKeysContract::evaluate_poll_escalation`],
/// [`CreatorKeysContract::set_escalation_config`]).
///
/// Kept separate from [`ContractError`] because Soroban caps `#[contracterror]`
/// enums at 50 variants and `ContractError` is already at that limit.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscalationError {
    /// The caller is not the protocol admin.
    Unauthorized = 1,
    /// The poll does not exist for the creator.
    PollNotFound = 2,
    /// The poll has already been closed.
    AlreadyClosed = 3,
    /// Arithmetic overflow while extending a deadline.
    Overflow = 4,
    /// The proposal is not close enough to its deadline to be evaluated.
    TooEarlyToEscalate = 5,
    /// Participation is not within the escalation threshold of quorum.
    BelowEscalationThreshold = 6,
    /// The proposal already consumed its maximum number of extensions.
    MaxExtensionsReached = 7,
    /// Quorum escalation is disabled because no config is set.
    EscalationDisabled = 8,
    /// The supplied escalation configuration is invalid.
    InvalidEscalationConfig = 9,
    /// The creator address is not registered.
    NotRegistered = 10,
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

/// Staking rewards configuration knobs.
pub mod staking {
    /// Share of each collected protocol trade fee routed into the staking
    /// rewards pool (10%).
    pub const REWARDS_SHARE_BPS: u32 = 1_000;
    /// Penalty applied to the pro-rata reward entitlement when a position is
    /// unstaked before its lock period elapses (20%). The penalty is deducted
    /// from the pool and retained on behalf of remaining stakers.
    pub const EARLY_UNSTAKE_PENALTY_BPS: u32 = 2_000;
}

pub mod constants {
    use super::DataKey;
    use soroban_sdk::Address;

    pub mod storage {
        use super::{creator_key, key_balance_key, DataKey};
        use crate::StakingKey;
        use soroban_sdk::Address;

        pub const FEE_CONFIG: DataKey = DataKey::FeeConfig;
        pub const KEY_PRICE: DataKey = DataKey::KeyPrice;
        pub const TREASURY_ADDRESS: DataKey = DataKey::TreasuryAddress;
        pub const ADMIN_ADDRESS: DataKey = DataKey::AdminAddress;
        pub const PROTOCOL_FEE_RECIPIENT: DataKey = DataKey::ProtocolFeeRecipient;
        pub const PROTOCOL_FEE_RECIPIENT_BALANCE: DataKey = DataKey::ProtocolFeeRecipientBalance;
        pub const PROTOCOL_STATE_VERSION: DataKey = DataKey::ProtocolStateVersion;
        pub const PAUSED: DataKey = DataKey::Paused;
        pub const CONTRACT_VERSION: DataKey = DataKey::ContractVersion;
        pub const SUPPLY_MILESTONES: DataKey = DataKey::SupplyMilestones;
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

        /// (creator, holder) -> unclaimed claim-based dividend balance (issue #857).
        pub fn unclaimed_dividend(creator: &Address, holder: &Address) -> DataKey {
            DataKey::UnclaimedDividend(creator.clone(), holder.clone())
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

        pub fn staking_position(creator: &Address, holder: &Address, stake_id: u32) -> DataKey {
            DataKey::StakePosition(creator.clone(), holder.clone(), stake_id)
        }

        pub fn staking_rewards_pool(creator: &Address) -> DataKey {
            DataKey::StakingRewardsPool(creator.clone())
        }

        pub fn total_staked(creator: &Address) -> DataKey {
            DataKey::TotalStaked(creator.clone())
        }

        pub fn stake_unlock_ledger(creator: &Address, holder: &Address) -> DataKey {
            DataKey::StakeUnlockLedger(creator.clone(), holder.clone())
        }

        pub fn created_at_ledger(creator: &Address) -> DataKey {
            DataKey::CreatedAtLedger(creator.clone())
        }

        pub fn launch_penalty_bps(creator: &Address) -> DataKey {
            DataKey::LaunchPenaltyBps(creator.clone())
        }

        pub fn auction_config(creator: &Address) -> DataKey {
            DataKey::AuctionConfig(creator.clone())
        }

        pub fn next_stake_id(creator: &Address, holder: &Address) -> StakingKey {
            StakingKey::NextStakeId(creator.clone(), holder.clone())
        }

        pub fn early_exit_penalty_bps(key_id: &Address) -> DataKey {
            DataKey::EarlyExitPenaltyBps(key_id.clone())
        }

        pub fn key_balance(creator: &Address, holder: &Address) -> DataKey {
            key_balance_key(creator, holder)
        }

        pub fn snapshot_meta(creator: &Address, snapshot_id: u32) -> DataKey {
            DataKey::HolderSnapshotMeta(creator.clone(), snapshot_id)
        }

        pub fn snapshot_balance(creator: &Address, snapshot_id: u32, holder: &Address) -> DataKey {
            DataKey::HolderSnapshotBalance(creator.clone(), snapshot_id, holder.clone())
        }

        pub fn snapshot_staked_balance(
            creator: &Address,
            snapshot_id: u32,
            holder: &Address,
        ) -> DataKey {
            DataKey::HolderSnapshotStakedBalance(creator.clone(), snapshot_id, holder.clone())
        }

        pub fn snapshot_holders(creator: &Address, snapshot_id: u32) -> DataKey {
            DataKey::HolderSnapshotHolders(creator.clone(), snapshot_id)
        }

        pub fn key_metadata(creator: &Address) -> DataKey {
            DataKey::KeyMetadata(creator.clone())
        }

        pub fn last_buy_ledger(creator: &Address, holder: &Address) -> DataKey {
            DataKey::LastBuyLedger(creator.clone(), holder.clone())
        }

        pub fn last_buy_timestamp(creator: &Address, holder: &Address) -> DataKey {
            DataKey::LastBuyTimestamp(creator.clone(), holder.clone())
        }

        pub fn max_keys_per_wallet(creator: &Address) -> DataKey {
            DataKey::MaxKeysPerWallet(creator.clone())
        }

        pub fn max_buy_quantity(creator: &Address) -> DataKey {
            DataKey::MaxBuyQuantity(creator.clone())
        }

        pub fn referral_fee_bps() -> DataKey {
            DataKey::ReferralFeeBps
        }

        pub fn royalty_config(creator: &Address) -> DataKey {
            DataKey::RoyaltyConfig(creator.clone())
        }

        pub fn curve_exponent(_creator: &Address) -> soroban_sdk::Symbol {
            soroban_sdk::symbol_short!("crv_exp")
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

        pub fn pause_state(creator: &Address) -> DataKey {
            DataKey::PauseState(creator.clone())
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

        pub fn self_frozen_balance(key_id: &Address, wallet: &Address) -> DataKey {
            DataKey::SelfFrozenBalance(key_id.clone(), wallet.clone())
        }

        pub fn whitelist_mode(key_id: &Address) -> DataKey {
            DataKey::WhitelistMode(key_id.clone())
        }

        pub fn vesting_claimed(creator: &Address, beneficiary: &Address) -> DataKey {
            DataKey::VestingClaimed(creator.clone(), beneficiary.clone())
        }

        pub fn holder_cap_bps(creator: &Address) -> DataKey {
            DataKey::HolderCapBps(creator.clone())
        }

        pub const MAX_HOLDING_BOUND: DataKey = DataKey::MaxHoldingBound;

        pub fn referrer_of(referee: &Address) -> DataKey {
            DataKey::Referrer(referee.clone())
        }

        pub fn referral_settled(referee: &Address) -> DataKey {
            DataKey::ReferralSettled(referee.clone())
        }
        pub fn quorum_bps(creator: &Address) -> DataKey {
            DataKey::QuorumBps(creator.clone())
        }

        pub fn buy_cooldown(creator: &Address) -> DataKey {
            DataKey::BuyCooldown(creator.clone())
        }

        /// Storage key for a creator's deprecation marker; value is `buyback_price_per_key` (i128).
        pub fn deprecated_key(creator: &Address) -> DataKey {
            DataKey::DeprecatedKey(creator.clone())
        }

        /// Storage key for the escrow balance held for a deprecated key's buyback pool.
        pub fn deprecation_escrow(creator: &Address) -> DataKey {
            DataKey::DeprecationEscrow(creator.clone())
        }

        /// Storage key for the price-oracle approved-caller allowlist.
        pub const APPROVED_CALLERS: DataKey = DataKey::ApprovedCallers;

        /// Storage key for a creator's price observation history (TWAP).
        pub fn price_history(creator: &Address) -> DataKey {
            DataKey::PriceHistory(creator.clone())
        }

        /// Storage key for the owner-set freeze flag on a wallet's key position.
        pub fn position_frozen(key_id: &Address, wallet: &Address) -> DataKey {
            DataKey::PositionFrozen(key_id.clone(), wallet.clone())
        }

        /// Storage key for the `auction_pending` flag set by `register_key`.
        pub fn auction_pending(creator: &Address) -> DataKey {
            DataKey::AuctionPending(creator.clone())
        }

        /// Storage key for the price snapshot retention age, in ledgers.
        pub const PRICE_RETENTION_LEDGERS: DataKey = DataKey::PriceRetentionLedgers;
        pub const ORACLE_ADDRESS: DataKey = DataKey::OracleAddress;
        pub const ORACLE_PRICE: DataKey = DataKey::OraclePrice;
        pub const ORACLE_STALENESS_SECS: DataKey = DataKey::OracleStalenessSecs;
        pub const ACTION_NEXT_ID: DataKey = DataKey::ActionNextId;
        pub const TIMELOCK_DELAY_SECS: DataKey = DataKey::TimelockDelaySecs;
        pub const GOVERNANCE_ADDRESS: DataKey = DataKey::GovernanceAddress;
        pub const SNAPSHOT_RETENTION_LEDGERS: DataKey = DataKey::SnapshotRetentionLedgers;

        pub fn next_snapshot_id(creator: &Address) -> DataKey {
            DataKey::NextSnapshotId(creator.clone())
        }

        pub fn oldest_snapshot_id(creator: &Address) -> DataKey {
            DataKey::OldestSnapshotId(creator.clone())
        }

        pub fn action_proposal(action_id: u32) -> DataKey {
            DataKey::ActionProposal(action_id)
        }

        pub fn vault_shares(creator: &Address, holder: &Address) -> DataKey {
            DataKey::VaultShares(creator.clone(), holder.clone())
        }

        pub fn vault_total_shares(creator: &Address) -> DataKey {
            DataKey::VaultTotalShares(creator.clone())
        }

        pub fn vault_reward_acc(creator: &Address) -> DataKey {
            DataKey::VaultRewardAcc(creator.clone())
        }

        pub fn vault_reward_checkpoint(creator: &Address, holder: &Address) -> DataKey {
            DataKey::VaultRewardCheckpoint(creator.clone(), holder.clone())
        }

        pub fn vault_reward_pending(creator: &Address, holder: &Address) -> DataKey {
            DataKey::VaultRewardPending(creator.clone(), holder.clone())
        }

        pub fn delegate(creator: &Address, delegator: &Address) -> DataKey {
            DataKey::Delegate(creator.clone(), delegator.clone())
        }

        pub fn fee_router() -> DataKey {
            DataKey::FeeRouter
        }

        pub fn reward_pool_balance() -> DataKey {
            DataKey::RewardPoolBalance
        }

        pub fn spread_bps(creator: &Address) -> DataKey {
            DataKey::SpreadBps(creator.clone())
        }

        pub fn trade_count(creator: &Address) -> DataKey {
            DataKey::TradeCount(creator.clone())
        }

        pub fn unique_trader_count(creator: &Address) -> DataKey {
            DataKey::UniqueTraderCount(creator.clone())
        }

        pub fn has_traded(creator: &Address, trader: &Address) -> DataKey {
            DataKey::HasTraded(creator.clone(), trader.clone())
        }

        /// Storage key for a creator's accumulated reputation score (`i128`).
        pub fn reputation_score(creator: &Address) -> DataKey {
            DataKey::ReputationScore(creator.clone())
        }

        /// Storage key for a creator's reputation contribution breakdown.
        pub fn reputation_breakdown(creator: &Address) -> DataKey {
            DataKey::ReputationBreakdown(creator.clone())
        }

        /// Storage key for an `(owner, spender, key_id)` transfer allowance.
        pub fn key_allowance(owner: &Address, spender: &Address, key_id: &Address) -> DataKey {
            DataKey::KeyAllowance(owner.clone(), spender.clone(), key_id.clone())
        }

        /// Storage key for a creator's sell tax in basis points.
        pub fn sell_tax_bps(creator: &Address) -> DataKey {
            DataKey::SellTaxBps(creator.clone())
        }

        /// Storage key for the protocol-wide buyback pool balance (`i128`).
        pub const BUYBACK_POOL_BALANCE: DataKey = DataKey::BuybackPoolBalance;

        /// Storage key for the address credited with the buyback pool balance.
        pub const BUYBACK_POOL_ADDRESS: DataKey = DataKey::BuybackPoolAddress;

        /// Storage key for the protocol-wide poll quorum-escalation config.
        pub const ESCALATION_CONFIG: DataKey = DataKey::EscalationConfig;

        pub fn creator_volume(creator: &Address) -> DataKey {
            DataKey::CreatorVolume(creator.clone())
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

/// Aggregated read-only snapshot of all key-level fields for a registered creator.
///
/// Returned by [`CreatorKeysContract::get_key_stats`] in a single RPC call so server
/// sync and admin snapshot endpoints no longer need multiple round trips.
///
/// # Field Stability
///
/// Fields are append-only. Do not reorder existing fields; the Soroban XDR encoder
/// serialises struct fields in declaration order and downstream indexers rely on
/// positional stability.
///
/// # Auction fields
///
/// `auction_price`, `auction_supply`, and `auction_sold` are populated only when a
/// pre-launch auction is configured for the creator. When no auction is configured,
/// all three fields are `0` and `has_auction` is `false`.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct KeyStatsView {
    /// Current bonding-curve (or auction) price for the next key purchase, in stroops.
    pub current_price: i128,
    /// Number of keys currently in circulation (does not include locked/unclaimed allocation).
    pub circulating_supply: u32,
    /// Number of distinct wallets holding at least one key.
    pub holder_count: u32,
    /// Whether protocol-wide trading is paused (`true` means buys/sells are blocked).
    pub trading_paused: bool,
    /// Hard supply ceiling configured by the creator, or `0` when uncapped.
    pub supply_cap: u32,
    /// Per-wallet holding cap in basis points (e.g. `1000` = 10 % of supply), or `0` when uncapped.
    pub holder_cap_bps: u32,
    /// Circuit-breaker price-jump threshold percentage (default `30`).
    pub circuit_breaker_threshold_bps: u32,
    /// Sell lockup window in seconds; `0` means no lockup is configured.
    pub lockup_duration_seconds: u64,
    /// Launch-penalty basis points applied to early sellers; `0` means no penalty is configured.
    pub launch_penalty_bps: u32,
    /// Per-wallet buy cooldown in ledgers; `0` means no cooldown is configured.
    pub buy_cooldown_ledgers: u32,
    /// Per-transaction maximum buy quantity; `0` means no limit is configured.
    pub max_buy_quantity: u32,
    /// `true` when a pre-launch auction is currently configured for this creator.
    pub has_auction: bool,
    /// Fixed auction price per key, in stroops. `0` when `has_auction` is `false`.
    pub auction_price: i128,
    /// Total keys available at the fixed auction price. `0` when `has_auction` is `false`.
    pub auction_supply: u32,
    /// Keys already sold through the auction. `0` when `has_auction` is `false`.
    pub auction_sold: u32,
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

/// Response for bonding curve price simulation (simulate_buy / simulate_sell).
///
/// Returns both the total cost (or proceeds) and the per-unit price for the
/// requested quantity, computed from the current bonding curve formula without
/// writing to any storage entry.
///
/// # Fields
/// - `total_cost` – total cost (buy) or net proceeds (sell) for `quantity` keys
/// - `per_unit_price` – price per single key (before fees)
/// - `creator_fee` – total creator fee portion for `quantity`
/// - `protocol_fee` – total protocol fee portion for `quantity`
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct SimulateResponse {
    pub total_cost: i128,
    pub per_unit_price: i128,
    pub creator_fee: i128,
    pub protocol_fee: i128,
}

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

/// Maximum staking lock extension from the current ledger (~180 days at 5 seconds per ledger).
pub const MAX_STAKE_LOCK_LEDGERS: u32 = 3_110_400;

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

/// Maximum byte-length of the `name` field in [`KeyMetadata`].
pub const METADATA_NAME_MAX_LEN: u32 = 64;

/// Maximum byte-length of the `bio` field in [`KeyMetadata`].
pub const METADATA_BIO_MAX_LEN: u32 = 256;

/// Maximum byte-length of the `avatar_uri` field in [`KeyMetadata`].
pub const METADATA_AVATAR_URI_MAX_LEN: u32 = 256;
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

/// Maximum number of entries in a single batch sell call.
pub const MAX_BATCH_SELL_SIZE: usize = 5;

/// Maximum number of `(recipient, quantity)` pairs accepted by a single
/// [`CreatorKeysContract::batch_transfer_keys`] call.
///
/// Larger lists revert with [`ContractError::BatchTransferSizeExceeded`].
pub const MAX_BATCH_TRANSFER_SIZE: u32 = 10;

/// Maximum royalty fee basis points (5%).
pub const MAX_ROYALTY_BPS: u32 = 500;

/// Lock duration for staked keys before a reward claim is permitted (30 days
/// at 5s per ledger).
pub const STAKE_LOCK_LEDGERS: u32 = 518_400;

/// Share of each protocol fee collection routed into a creator's staking
/// rewards pool (10%), on top of the existing treasury/recipient split.
pub const STAKING_REWARD_SHARE_BPS: u32 = 1_000;

/// Launch penalty window in ledgers (~7 days at 5s per ledger).
pub const LAUNCH_PENALTY_WINDOW_LEDGERS: u32 = 120_960;

/// Default launch penalty basis points (5%).
pub const DEFAULT_LAUNCH_PENALTY_BPS: u32 = 500;

/// Maximum launch penalty basis points (20%).
pub const MAX_LAUNCH_PENALTY_BPS: u32 = 2_000;

/// Maximum per-wallet buy cooldown in ledgers (~1 hour at 5 s/ledger).
///
/// Creators cannot configure a cooldown longer than this value via
/// [`CreatorKeysContract::set_buy_cooldown`]. A cooldown of 0 means no
/// restriction (the default when no cooldown has been configured).
pub const MAX_BUY_COOLDOWN_LEDGERS: u32 = 720;

/// Maximum allowed per-transaction buy quantity limit.
pub const MAX_BUY_QUANTITY_LIMIT: u32 = 10_000;

/// Maximum number of keys a pre-launch auction can allocate at the fixed
/// auction price before the bonding curve takes over.
pub const MAX_AUCTION_SUPPLY: u32 = 10_000;

// ---------------------------------------------------------------------------
// Reputation scoring
// ---------------------------------------------------------------------------

/// Reputation awarded for a successful key launch.
pub const REPUTATION_KEY_LAUNCH_POINTS: i128 = 100;

/// Reputation awarded for each supply milestone crossed upward.
pub const REPUTATION_MILESTONE_POINTS: i128 = 25;

/// Reputation awarded for participating in a governance vote.
pub const REPUTATION_GOVERNANCE_PARTICIPATION_POINTS: i128 = 10;

/// Reputation awarded for each completed trade on the creator's own key.
pub const REPUTATION_TRADE_POINTS: i128 = 5;

/// Reputation penalty applied when a creator deprecates their own key.
pub const REPUTATION_DEPRECATION_PENALTY: i128 = 50;

/// Lowest reputation score a creator can hold. Decrements saturate at zero
/// rather than pushing the score negative, so the scale is a floor-bounded
/// `i128` rather than a signed balance.
pub const REPUTATION_MIN_SCORE: i128 = 0;

/// Upper bound on the penalty a single governance violation may apply, so a
/// misconfigured [`CreatorKeysContract::apply_governance_violation`] call cannot
/// wipe out a creator's accumulated standing in one transaction.
pub const MAX_GOVERNANCE_VIOLATION_PENALTY: i128 = 1_000;

// ---------------------------------------------------------------------------
// Sell tax and buyback pool
// ---------------------------------------------------------------------------

/// Maximum per-key sell tax in basis points (10%).
///
/// Creators configure their own sell tax via
/// [`CreatorKeysContract::set_sell_tax_bps`] but cannot exceed this ceiling,
/// which keeps the tax bounded relative to the creator's own payout.
pub const MAX_SELL_TAX_BPS: u32 = 1_000;

// ---------------------------------------------------------------------------
// Governance quorum escalation
// ---------------------------------------------------------------------------

/// Default quorum-escalation threshold in basis points (50% of the required
/// quorum). A proposal whose participation has reached this fraction of its
/// quorum requirement inside the evaluation window is eligible for an extension.
pub const DEFAULT_ESCALATION_THRESHOLD_BPS: u32 = 5_000;

/// Default number of ledgers added per quorum-escalation extension (~12 hours
/// at 5 s per ledger).
pub const DEFAULT_ESCALATION_EXTENSION_LEDGERS: u32 = 8_640;

/// Default maximum number of times a single proposal may be extended.
pub const DEFAULT_MAX_ESCALATION_EXTENSIONS: u32 = 3;

/// Floor on the escalation threshold (1% of quorum).
pub const MIN_ESCALATION_THRESHOLD_BPS: u32 = 100;

/// Ceiling on the escalation threshold (100% of quorum). A threshold of
/// 10 000 means "extend only once the proposal has fully reached quorum",
/// which effectively disables the mechanism without turning it off.
pub const MAX_ESCALATION_THRESHOLD_BPS: u32 = 10_000;

/// Upper bound on the number of extensions a single proposal may consume,
/// preventing indefinite postponement of a vote.
pub const MAX_ESCALATION_EXTENSIONS_BOUND: u32 = 10;

/// Upper bound on a single extension duration in ledgers (~30 days at 5 s
/// per ledger) so a configured extension cannot stall governance forever.
pub const MAX_ESCALATION_EXTENSION_LEDGERS: u32 = 518_400;

/// How many ledgers before the deadline a proposal becomes eligible for
/// escalation evaluation.
pub const ESCALATION_EVALUATION_WINDOW_LEDGERS: u32 = 1_080;

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
#[contracttype(export = false)]
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
    /// Contract upgrade version counter (#884).
    ContractVersion,
    /// Ascending supply thresholds that emit `MilestoneCrossed` events (#887).
    SupplyMilestones,
    DividendPerKeyAccumulated(Address),
    HolderDividendCheckpoint(Address, Address),
    HolderDividendPending(Address, Address),
    /// (creator, holder) -> unclaimed claim-based dividend balance (issue #857).
    UnclaimedDividend(Address, Address),
    LockedAllocation(Address),
    MaxSupply(Address),
    CurveSlope,
    CurvePreset(Address),
    /// Per-creator royalty configuration (buy/sell fee bps) (PR #795).
    RoyaltyConfig(Address),
    /// Per-creator bonding curve exponent override (PR #795).
    CurveExponent(Address),
    TreasuryBalance,
    CoCreator(Address),
    CoCreatorFeeBalance(Address, Address),
    Whitelist(Address),
    StakedBalance(Address, Address), // (creator, holder) -> staked amount
    MaxKeysPerWallet(Address),
    ReferralFeeBps,
    DiscountTiers,
    CreatorVolume(Address),
    ApprovedCallers,
    PriceHistory(Address),
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
    PauseState(Address),
    VestingSchedule(Address, Address),
    VestingClaimed(Address, Address),
    TimelockProposal(u32),
    TimelockNextId,
    VoteSnapshot(Address, u32, Address),
    CircuitBreakerThreshold,
    ReferralEarnings(Address),
    WhitelistMap(Address, Address),
    WhitelistMode(Address),
    /// Pre-launch auction configuration for a creator's keys.
    AuctionConfig(Address),
    /// (creator, snapshot_id) -> `HolderSnapshotMeta` (issue #778).
    HolderSnapshotMeta(Address, u32),
    /// (creator, snapshot_id, holder) -> balance at snapshot time (issue #778).
    HolderSnapshotBalance(Address, u32, Address),
    /// (creator, snapshot_id, holder) -> staked balance at snapshot time.
    HolderSnapshotStakedBalance(Address, u32, Address),
    /// (creator, snapshot_id) -> holder list captured by the snapshot.
    HolderSnapshotHolders(Address, u32),
    /// (creator) -> on-chain identity metadata set via `initialise_key` (issue #779).
    KeyMetadata(Address),
    /// (creator, holder) -> ledger of the holder's most recent buy (issue #781).
    LastBuyLedger(Address, Address),
    /// (creator, holder) -> timestamp of the holder's most recent buy, used by
    /// the anti-flash-trade sell lockup window (#784).
    LastBuyTimestamp(Address, Address),
    /// Lockup duration in seconds for sell lockup enforcement.
    LockupDurationSecs,
    QuorumBps(Address),
    /// Per-creator holder cap in basis points (max % of supply one wallet may hold).
    HolderCapBps(Address),
    /// Protocol fee basis points.
    ProtocolFeeBps,
    /// Protocol-wide emergency trading halt flag (#784). When `true`, every
    /// buy and sell is rejected regardless of per-key pause state.
    GlobalTradingPaused,
    GlobalPauseAdmins,
    GlobalPauseVote(Address),
    GlobalResumeVote(Address),
    SelfFrozenBalance(Address, Address),
    /// Per-creator staking position. Keyed `(creator, holder, stake_id)`.
    StakePosition(Address, Address, u32),
    /// Per-creator staking rewards pool and cross-holder staked-key total.
    StakingRewardsPool(Address),
    /// Ledger sequence when the first key was bought for a creator.
    CreatedAtLedger(Address),
    /// Custom launch penalty basis points for a creator (0 = use default).
    LaunchPenaltyBps(Address),
    /// Per-(creator, holder) stake unlock ledger sequence.
    StakeUnlockLedger(Address, Address),
    /// Total keys currently staked for a creator across all holders.
    TotalStaked(Address),
    /// Per-creator buy cooldown in ledgers. A value of `0` (or absent) means
    /// no cooldown is configured. Set via `set_buy_cooldown`.
    BuyCooldown(Address),
    /// Marks a creator key as deprecated. Value is the fixed `buyback_price_per_key` (i128).
    DeprecatedKey(Address),
    /// Escrow balance held on behalf of a deprecated key's creator.
    /// Funds are paid out to redeeming holders and any remainder is returned on full redemption.
    DeprecationEscrow(Address),
    /// Configured early exit penalty bps for key.
    EarlyExitPenaltyBps(Address),
    /// Maximum buy quantity per transaction for a creator.
    MaxBuyQuantity(Address),
    /// Admin-defined upper bound for a creator's per-wallet holding cap.
    MaxHoldingBound,
    /// Registered referrer for a referee wallet.
    Referrer(Address),
    /// Set once a referee's first referred trade has paid its referral reward.
    ReferralSettled(Address),
    /// Owner-set freeze flag on a `(key_id, wallet)` position.
    PositionFrozen(Address, Address),
    /// `true` when a key was registered via `register_key` in auction mode.
    AuctionPending(Address),
    /// Age in ledgers after which price snapshots are pruned (`0` = no age limit).
    PriceRetentionLedgers,
    /// Address authorised to publish oracle prices.
    OracleAddress,
    /// Latest oracle price and the timestamp it was published at.
    OraclePrice,
    /// Age in seconds after which the oracle price is flagged stale.
    OracleStalenessSecs,
    /// Timelocked admin action keyed by action id.
    ActionProposal(u32),
    /// Next sequential timelocked action id.
    ActionNextId,
    /// Configured timelock delay in seconds for new actions.
    TimelockDelaySecs,
    /// Address of the authorised governance contract that may call `take_snapshot`.
    GovernanceAddress,
    /// Snapshot retention window in ledgers; snapshots older than this are pruned.
    /// `0` means no age-based pruning (the default).
    SnapshotRetentionLedgers,
    /// (creator) -> next sequential snapshot id used to track the oldest snapshot for pruning.
    NextSnapshotId(Address),
    /// (creator) -> oldest snapshot id still present (used for pruning).
    OldestSnapshotId(Address),
    /// (creator, holder) -> keys the holder has deposited in the staking vault.
    VaultShares(Address, Address),
    /// creator -> total keys deposited in the staking vault.
    VaultTotalShares(Address),
    /// creator -> vault reward accumulator per share (scaled).
    VaultRewardAcc(Address),
    /// (creator, holder) -> vault reward accumulator at the last settlement.
    VaultRewardCheckpoint(Address, Address),
    /// (creator, holder) -> settled but unclaimed vault rewards.
    VaultRewardPending(Address, Address),
    /// (creator, delegator) -> delegate wallet for governance.
    Delegate(Address, Address),
    /// Address of the authorised fee router that may call `topup_reward_pool`.
    FeeRouter,
    /// Global staker reward pool balance (in stroops).
    RewardPoolBalance,
    /// Per-creator bid-ask spread in basis points.
    SpreadBps(Address),
    /// Per-creator cumulative trade count (buy + sell).
    TradeCount(Address),
    /// Per-creator count of unique wallets that have ever traded.
    UniqueTraderCount(Address),
    /// Per-creator per-wallet flag: true if this wallet has ever traded.
    HasTraded(Address, Address),

    /// (creator) -> accumulated reputation score (`i128`, floored at zero).
    ReputationScore(Address),
    /// (creator) -> per-reason contribution breakdown backing the reputation score.
    ReputationBreakdown(Address),
    /// (owner, spender, key_id) -> approved transfer allowance in whole keys (`u32`).
    KeyAllowance(Address, Address, Address),
    /// (creator) -> per-key sell tax in basis points.
    SellTaxBps(Address),
    /// Protocol-wide buyback pool balance in stroops (`i128`).
    BuybackPoolBalance,
    /// Address credited with the buyback pool balance.
    BuybackPoolAddress,
    /// Protocol-wide poll quorum-escalation configuration.
    EscalationConfig,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ReinvestResult {
    pub keys_bought: u32,
    pub remainder_returned: i128,
}

/// Internal staking account keys that are not part of the public data-key ABI.
///
/// Used to keep [`DataKey`] within Soroban's 50-variant `#[contracttype]` cap;
/// `NextStakeId` is keyed per `(creator, holder)` pair.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum StakingKey {
    /// Next sequential stake id for a `(creator, holder)` pair -> `u32`.
    NextStakeId(Address, Address),
}

/// Storage keys for the cycle-based protocol revenue distribution (#877).
///
/// Kept separate from [`DataKey`] to stay within Soroban's 50-variant cap.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum RevenueKey {
    /// Admin-configured cycle length in ledgers -> `u32`.
    CycleLength,
    /// creator -> undistributed pool balance (`i128`).
    Pool(Address),
    /// creator -> number of cycles distributed so far (`u32`).
    CycleCount(Address),
    /// creator -> ledger sequence of the last distribution (`u32`).
    LastDistribution(Address),
    /// (creator, cycle) -> pool balance snapshotted for that cycle (`i128`).
    CyclePool(Address, u32),
    /// (creator, cycle, holder) -> allocated share (`i128`).
    CycleShare(Address, u32, Address),
    /// (creator, cycle, holder) -> `true` once the share has been claimed.
    CycleClaimed(Address, u32, Address),
}

/// Configuration for a creator's fixed-price pre-launch auction phase.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AuctionConfig {
    pub auction_price: i128,
    pub auction_supply: u32,
    pub auction_sold: u32,
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

/// A single locked staking position held by a holder.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct StakePosition {
    /// Sequential id scoped to the `(creator, holder)` pair.
    pub stake_id: u32,
    /// Number of keys locked in this position.
    pub amount: u32,
    /// Ledger sequence at which the position matures and can be claimed.
    pub unlock_ledger: u32,
}

/// Per-creator staking rewards accounting.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct StakingRewardsState {
    /// Accumulated reward pool, funded from a share of protocol trade fees.
    pub pool: i128,
    /// Total keys currently staked for the creator across all holders.
    pub total_staked: u32,
}

/// Result of [`CreatorKeysContract::early_unstake`].
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct StakeExit {
    /// Id of the closed position.
    pub stake_id: u32,
    /// Keys released back to the holder's liquid balance.
    pub amount: u32,
    /// Pro-rata reward entitlement removed from the pool.
    pub forgone_reward: i128,
    /// Penalty retained in the pool (added back after the forgone reward is removed).
    pub penalty: i128,
}

/// Result of [`CreatorKeysContract::claim_stake_reward`].
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct StakeRewardClaim {
    /// Id of the closed position.
    pub stake_id: u32,
    /// Keys released back to the holder's liquid balance.
    pub amount: u32,
    /// Reward paid out to the staker from the pool.
    pub reward: i128,
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

/// Metadata for a completed holder snapshot (issue #778).
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct HolderSnapshotMeta {
    pub snapshot_ledger: u32,
    pub total_holders: u32,
}

/// Maximum number of holder addresses accepted per `take_snapshot` call.
///
/// Soroban contract storage cannot be enumerated on-chain (there is no
/// "iterate all keys with this prefix"), so unlike the issue's literal
/// wording, `take_snapshot` takes the holder list as a caller-supplied
/// argument (sourced off-chain, e.g. from an indexer) rather than iterating
/// a registry internally. This cap bounds the batch the same way
/// [`MAX_AIRDROP_RECIPIENTS`] already bounds `airdrop_keys` — without it, a
/// popular key's full holder set could exceed the per-transaction
/// instruction budget in one call. A key with more holders than this needs
/// its indexer-supplied list paginated by the caller across a fresh
/// `snapshot_id` per page — `take_snapshot` itself is a one-shot operation
/// per `snapshot_id` (a second call with the same id fails with
/// [`ContractError::SnapshotAlreadyExists`], per the acceptance criteria).
pub const MAX_SNAPSHOT_HOLDERS: u32 = 100;

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

/// Latest price published by the authorised oracle address.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct OraclePrice {
    pub price: i128,
    /// Ledger timestamp (seconds) at which the price was published.
    pub updated_at: u64,
}

/// Oracle price together with its staleness state, returned by `get_oracle_price`.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct OraclePriceView {
    pub price: i128,
    pub updated_at: u64,
    /// `true` when the price is older than the configured staleness threshold.
    pub is_stale: bool,
}

/// A timelocked admin action awaiting its execution timestamp.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct TimelockAction {
    pub change_type: TimelockChangeType,
    pub payload: soroban_sdk::Bytes,
    pub proposer: Address,
    pub proposed_at: u64,
    /// Earliest ledger timestamp (seconds) at which the action may execute.
    pub execution_not_before: u64,
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

/// Live pause state for a key's trading.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PauseState {
    pub trading_paused: bool,
    pub pause_expires_at: u32,
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

/// A single price observation recorded for a creator, used to compute TWAP.
///
/// `ledger` is the Soroban ledger sequence number at which the observation was
/// recorded and `price` is the bonding-curve price observed at that ledger.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceObservation {
    pub ledger: u32,
    pub price: i128,
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

/// Result of a single order in a batch sell.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BatchSellOrderResult {
    pub key_id: Address,
    pub quantity: u32,
    pub proceeds: i128,
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

/// Metadata associated with a creator key that can be set at initialisation
/// and updated later via [`update_metadata`].
///
/// Only fields wrapped in `Some` are updated; `None` fields are left unchanged.
/// Byte-length limits mirror the handle validation enforced by
/// [`validate_creator_handle`] for `name` and use dedicated caps for `bio`
/// and `avatar_uri`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyMetadata {
    pub name: String,
    pub bio: String,
    pub avatar_uri: String,
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

fn read_self_frozen_balance(env: &Env, key_id: &Address, wallet: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&constants::storage::self_frozen_balance(key_id, wallet))
        .unwrap_or(0)
}

fn available_holder_balance(env: &Env, key_id: &Address, wallet: &Address) -> u32 {
    let total = env
        .storage()
        .persistent()
        .get(&constants::storage::holder_balance_key(key_id, wallet))
        .unwrap_or(0u32);
    let staked = env
        .storage()
        .persistent()
        .get(&constants::storage::staked_balance(key_id, wallet))
        .unwrap_or(0u32);
    total
        .saturating_sub(staked)
        .saturating_sub(read_self_frozen_balance(env, key_id, wallet))
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

/// Credits `amount` to the creator fee balance for `creator`.
fn credit_creator_fee_balance(
    env: &Env,
    creator: &Address,
    amount: i128,
) -> Result<(), ContractError> {
    credit_creator_fee_recipient_balance(env, creator, amount)
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

/// Reads creator key metadata from persistent storage.
///
/// Returns `None` when no metadata has been initialised for the creator.
pub fn read_creator_metadata(env: &Env, creator: &Address) -> Option<KeyMetadata> {
    use soroban_sdk::symbol_short;
    let key = (symbol_short!("md"), creator.clone());
    env.storage().persistent().get(&key)
}

/// Validates the byte-length of a metadata string field.
///
/// Returns [`ContractError::HandleTooLong`] when `value.len()` exceeds `max_len`.
fn assert_metadata_field_length(
    value: &String,
    max_len: u32,
    error: ContractError,
) -> Result<(), ContractError> {
    if value.len() > max_len {
        return Err(error);
    }
    Ok(())
}

/// Validates a complete [`KeyMetadata`] payload.
///
/// Rejects an empty `name` (blank or whitespace-only) with
/// [`ContractError::DisplayNameEmpty`] and enforces per-field byte-length
/// limits consistent with the handle rules used at registration.
fn validate_key_metadata(metadata: &KeyMetadata) -> Result<(), ContractError> {
    if metadata.name.is_empty() {
        return Err(ContractError::DisplayNameEmpty);
    }
    assert_metadata_field_length(
        &metadata.name,
        METADATA_NAME_MAX_LEN,
        ContractError::NameTooLong,
    )?;
    assert_metadata_field_length(
        &metadata.bio,
        METADATA_BIO_MAX_LEN,
        ContractError::BioTooLong,
    )?;
    assert_metadata_field_length(
        &metadata.avatar_uri,
        METADATA_AVATAR_URI_MAX_LEN,
        ContractError::NameTooLong,
    )?;
    Ok(())
}

/// Writes creator key metadata to persistent storage.
fn write_creator_metadata(env: &Env, creator: &Address, metadata: &KeyMetadata) {
    use soroban_sdk::symbol_short;
    let key = (symbol_short!("md"), creator.clone());
    env.storage().persistent().set(&key, metadata);
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

/// Emits one `MilestoneCrossed` event per configured supply milestone crossed
/// when a trade moves supply from `old_supply` to `new_supply` (#887).
fn emit_milestone_crossings(
    env: &Env,
    creator: &Address,
    old_supply: u32,
    new_supply: u32,
) -> Result<(), ContractError> {
    let milestones: Vec<u32> = env
        .storage()
        .persistent()
        .get(&constants::storage::SUPPLY_MILESTONES)
        .unwrap_or(Vec::new(env));
    let up = new_supply > old_supply;
    let count = milestones.len();
    for i in 0..count {
        // Emit in the order the supply travels: ascending on buys, descending on sells.
        let idx = if up { i } else { count - 1 - i };
        let milestone = milestones.get(idx).ok_or(ContractError::Overflow)?;
        let crossed = if up {
            old_supply < milestone && milestone <= new_supply
        } else {
            new_supply < milestone && milestone <= old_supply
        };
        if crossed {
            env.events().publish(
                events::milestone_crossed_topics(creator),
                events::MilestoneCrossedEvent {
                    key_id: creator.clone(),
                    tier: idx + 1,
                    direction: if up {
                        events::MILESTONE_DIRECTION_UP
                    } else {
                        events::MILESTONE_DIRECTION_DOWN
                    },
                    supply: new_supply,
                },
            );
            // Only upward crossings are a positive signal; a sell that drops
            // supply back below a tier must not earn reputation for it.
            if up {
                accrue_reputation_on_milestone(env, creator)?;
            }
        }
    }
    Ok(())
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
        return Err(ContractError::GlobalTradingHalted);
    }
    Ok(())
}

/// Read-only helper for the per-key pause state. A key is considered active only
/// while `trading_paused` is `true` and the current ledger is still before the
/// configured expiry.
fn read_pause_state(env: &Env, key_id: &Address) -> PauseState {
    env.storage()
        .persistent()
        .get(&constants::storage::pause_state(key_id))
        .unwrap_or(PauseState {
            trading_paused: false,
            pause_expires_at: 0,
        })
}

fn is_key_trading_paused(env: &Env, key_id: &Address) -> bool {
    let state = read_pause_state(env, key_id);
    state.trading_paused && env.ledger().sequence() < state.pause_expires_at
}

fn assert_key_trading_not_paused(env: &Env, key_id: &Address) -> Result<(), ContractError> {
    if is_key_trading_paused(env, key_id) {
        return Err(ContractError::GlobalTradingHalted);
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

/// Returns `true` when the wallet has frozen its own position for this key.
fn is_position_frozen(env: &Env, key_id: &Address, wallet: &Address) -> bool {
    env.storage()
        .persistent()
        .get::<DataKey, bool>(&constants::storage::position_frozen(key_id, wallet))
        .unwrap_or(false)
}

fn assert_position_not_frozen(
    env: &Env,
    key_id: &Address,
    wallet: &Address,
) -> Result<(), ContractError> {
    if is_position_frozen(env, key_id, wallet) {
        return Err(ContractError::FrozenPosition);
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

/// Rejects a transfer that would leave the recipient above the creator's
/// per-wallet holding cap (the same cap `buy_key` enforces).
fn assert_within_holding_cap(
    env: &Env,
    creator: &Address,
    new_balance: u32,
) -> Result<(), ContractError> {
    if let Some(cap) = env
        .storage()
        .persistent()
        .get::<DataKey, u32>(&constants::storage::max_keys_per_wallet(creator))
    {
        if new_balance > cap {
            return Err(ContractError::WalletCapExceeded);
        }
    }
    Ok(())
}

/// Resolves the referrer for a buy. An explicit referrer wins; otherwise the
/// buyer's registered referrer is used once, on their first referred trade.
/// Returns the referrer and whether it came from the one-time registration.
fn resolve_registered_referrer(
    env: &Env,
    buyer: &Address,
    explicit: Option<Address>,
) -> (Option<Address>, bool) {
    if explicit.is_some() {
        return (explicit, false);
    }
    let settled_key = constants::storage::referral_settled(buyer);
    if env.storage().persistent().has(&settled_key) {
        return (None, false);
    }
    let registered: Option<Address> = env
        .storage()
        .persistent()
        .get(&constants::storage::referrer_of(buyer));
    if registered.is_some() {
        env.storage().persistent().set(&settled_key, &true);
        extend_key_ttl_to_full_window(env, &settled_key);
    }
    let is_registered = registered.is_some();
    (registered, is_registered)
}

fn assert_creator_or_admin(
    env: &Env,
    caller: &Address,
    creator: &Address,
) -> Result<(), ContractError> {
    read_registered_creator_profile(env, creator)?;
    if caller == creator {
        return Ok(());
    }
    assert_is_admin(env, caller)
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
/// Returns the canonical Stellar zero address, used as the placeholder
/// destination for funds the contract holds on behalf of an unassigned
/// recipient.
fn zero_address(env: &Env) -> Address {
    Address::from_string(&String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ))
}

fn validate_non_zero_address(env: &Env, addr: &Address) -> Result<(), ContractError> {
    if *addr == zero_address(env) {
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
/// treasury balance, routes a fixed share into the creator's staking rewards
/// pool and emits a [`events::FEE_COLLECTED_EVENT_NAME`] event.
///
/// Returns the net remainder that flows into the creator/seller payout math.
/// With a rate of 0 bps no treasury credit, pool credit or event is produced
/// and the full amount is returned unchanged.
fn collect_protocol_trade_fee(
    env: &Env,
    creator: &Address,
    amount: i128,
) -> Result<i128, ContractError> {
    let trade_fee = compute_trade_fee(env, amount)?;
    if trade_fee == 0 {
        return Ok(amount);
    }
    let (_, treasury) = read_trade_fee_config(env).ok_or(ContractError::FeeConfigNotSet)?;
    credit_treasury_balance(env, trade_fee)?;
    credit_staking_rewards_pool(env, creator, trade_fee)?;
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

/// Credits the configured share of a protocol trade fee into the creator's
/// staking rewards pool. No-op for zero share/dormant pools (the pool entry is
/// only created once a fee actually accrues).
fn credit_staking_rewards_pool(
    env: &Env,
    creator: &Address,
    trade_fee: i128,
) -> Result<(), ContractError> {
    let share = fee::apply_percentage_fee(trade_fee, crate::staking::REWARDS_SHARE_BPS)
        .ok_or(ContractError::Overflow)?;
    if share == 0 {
        return Ok(());
    }
    let pool_key = constants::storage::staking_rewards_pool(creator);
    let mut state: StakingRewardsState =
        env.storage()
            .persistent()
            .get(&pool_key)
            .unwrap_or(StakingRewardsState {
                pool: 0,
                total_staked: 0,
            });
    state.pool = state
        .pool
        .checked_add(share)
        .ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&pool_key, &state);
    extend_key_ttl_to_full_window(env, &pool_key);
    Ok(())
}

/// Maps [`ContractError`] values raised by shared guards (`assert_not_paused`,
/// `read_registered_creator_profile`) into the [`StakingError`] surface used by
/// the staking lifecycle entrypoints.
fn map_staking_error(err: ContractError) -> StakingError {
    match err {
        ContractError::Overflow => StakingError::Overflow,
        ContractError::NotRegistered => StakingError::NotRegistered,
        ContractError::ProtocolPaused => StakingError::ProtocolPaused,
        ContractError::Unauthorized => StakingError::Overflow,
        _ => StakingError::Overflow,
    }
}

/// Decrements the holder's staked balance when a position is closed, removing
/// the storage entry once it hits zero (mirroring `unstake_keys`).
fn sub_staked_balance(env: &Env, creator: &Address, holder: &Address, amount: u32) {
    let staked_balance_key = constants::storage::staked_balance(creator, holder);
    let current_staked: u32 = env
        .storage()
        .persistent()
        .get(&staked_balance_key)
        .unwrap_or(0);
    if let Some(new_staked) = current_staked.checked_sub(amount) {
        if new_staked == 0 {
            env.storage().persistent().remove(&staked_balance_key);
        } else {
            env.storage()
                .persistent()
                .set(&staked_balance_key, &new_staked);
            extend_key_ttl_to_full_window(env, &staked_balance_key);
        }
    }
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

/// Reads the total keys currently staked across all holders for a creator.
pub fn read_total_staked(env: &Env, creator: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&constants::storage::total_staked(creator))
        .unwrap_or(0)
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

fn assert_buy_price_slippage(
    env: &Env,
    creator: &Address,
    price: i128,
    max_price: Option<i128>,
) -> Result<(), ContractError> {
    if let Some(max) = max_price {
        if price > max {
            return Err(ContractError::SlippageExceeded);
        }
        env.events().publish(
            events::slippage_check_passed_topics(creator),
            events::SlippageCheckPassedEvent {
                creator_id: creator.clone(),
                actual_amount: price,
                bound: max,
                ledger: env.ledger().sequence(),
            },
        );
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
    creator: &Address,
    price: i128,
    min_proceeds: Option<i128>,
) -> Result<(), ContractError> {
    if let Some(min) = min_proceeds {
        let proceeds = compute_sell_proceeds(env, price)?;
        if proceeds < min {
            return Err(ContractError::SlippageExceeded);
        }
        env.events().publish(
            events::slippage_check_passed_topics(creator),
            events::SlippageCheckPassedEvent {
                creator_id: creator.clone(),
                actual_amount: proceeds,
                bound: min,
                ledger: env.ledger().sequence(),
            },
        );
    }
    Ok(())
}

fn accrue_sell_trade_fees(env: &Env, creator: &Address, price: i128) -> Result<(), ContractError> {
    // Deduct the protocol trade fee first so the treasury is paid before the
    // creator/seller split is computed on the remainder.
    let net_price = collect_protocol_trade_fee(env, creator, price)?;

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
pub fn extend_key_ttl_to_full_window<K: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(
    env: &Env,
    key: &K,
) {
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

    let created_at_key = constants::storage::created_at_ledger(creator);
    if env.storage().persistent().has(&created_at_key) {
        env.storage()
            .persistent()
            .extend_ttl(&created_at_key, threshold, extend_to);
    }

    let launch_penalty_key = constants::storage::launch_penalty_bps(creator);
    if env.storage().persistent().has(&launch_penalty_key) {
        env.storage()
            .persistent()
            .extend_ttl(&launch_penalty_key, threshold, extend_to);
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

/// Maximum number of price observations retained per creator for TWAP.
///
/// Bounding the history prevents unbounded persistent storage growth while a
/// creator's `get_price` / `get_twap_price` is queried over time. Older
/// observations beyond this limit are pruned from the front.
pub const MAX_PRICE_OBSERVATIONS: u32 = 100;

/// Reads the current approved-caller allowlist from persistent storage.
///
/// Returns an empty vector when no allowlist has been configured (i.e. no
/// callers approved yet).
fn read_approved_callers(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get::<DataKey, Vec<Address>>(&constants::storage::APPROVED_CALLERS)
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns `true` when `caller` is present in the approved-caller allowlist.
fn is_caller_approved(env: &Env, caller: &Address) -> bool {
    read_approved_callers(env).iter().any(|c| &c == caller)
}

/// Guard used by the price oracle entrypoints: rejects callers that are not
/// present in the admin-maintained allowlist.
///
/// # Errors
///
/// - [`ContractError::CallerNotApproved`] if `caller` is not approved.
fn assert_caller_approved(env: &Env, caller: &Address) -> Result<(), ContractError> {
    if is_caller_approved(env, caller) {
        Ok(())
    } else {
        Err(ContractError::CallerNotApproved)
    }
}

/// Reads the recorded price observation history for a creator.
fn read_price_history(env: &Env, creator: &Address) -> Vec<PriceObservation> {
    env.storage()
        .persistent()
        .get::<DataKey, Vec<PriceObservation>>(&constants::storage::price_history(creator))
        .unwrap_or_else(|| Vec::new(env))
}

/// Records a price observation for `creator` at the current ledger.
///
/// A repeated observation at the same ledger replaces the prior entry (the
/// price cannot change within a ledger). Appends to the tail and prunes from
/// the front once [`MAX_PRICE_OBSERVATIONS`] is exceeded so the history stays
/// bounded. These observations drive [`CreatorKeysContract::get_twap_price`].
fn record_price_observation(env: &Env, creator: &Address, price: i128) {
    let current_ledger = env.ledger().sequence();
    let mut history = read_price_history(env, creator);

    if let Some(last) = history.last() {
        if last.ledger == current_ledger && last.price == price {
            return;
        }
    }

    history.push_back(PriceObservation {
        ledger: current_ledger,
        price,
    });

    while history.len() > MAX_PRICE_OBSERVATIONS {
        history.remove(0);
    }

    let history_key = constants::storage::price_history(creator);
    env.storage().persistent().set(&history_key, &history);
    extend_key_ttl_to_full_window(env, &history_key);
}

/// Drops price observations older than the configured retention age.
///
/// A retention of `0` (the default) disables age-based pruning; the
/// [`MAX_PRICE_OBSERVATIONS`] bound still applies.
fn prune_price_history(env: &Env, creator: &Address) {
    let retention: u32 = env
        .storage()
        .persistent()
        .get(&constants::storage::PRICE_RETENTION_LEDGERS)
        .unwrap_or(0);
    if retention == 0 {
        return;
    }
    let cutoff = env.ledger().sequence().saturating_sub(retention);
    let mut history = read_price_history(env, creator);
    let before = history.len();
    while history.first().is_some_and(|o| o.ledger < cutoff) {
        history.remove(0);
    }
    if history.len() != before {
        env.storage()
            .persistent()
            .set(&constants::storage::price_history(creator), &history);
    }
}

/// Records the post-trade price as a snapshot and prunes aged snapshots.
///
/// Called from the buy and sell paths in the same invocation as the trade, so
/// the snapshot is stored atomically with it.
fn record_trade_price_snapshot(env: &Env, creator: &Address) {
    let base_price: Option<i128> = env
        .storage()
        .persistent()
        .get(&constants::storage::KEY_PRICE);
    if let (Some(base_price), Ok(profile)) =
        (base_price, read_registered_creator_profile(env, creator))
    {
        if let Ok(price) = compute_bonding_curve_price(env, creator, base_price, profile.supply) {
            record_price_observation(env, creator, price);
            prune_price_history(env, creator);
        }
    }
}

/// Computes the time-weighted average price for `creator` over
/// `window_ledgers` ledgers ending at the current ledger.
///
/// Each observed price is weighted by the number of ledgers it was in effect
/// within the window. When no observation is available inside the window, the
/// current price is returned unchanged (a single-observation TWAP).
fn compute_twap_price(
    env: &Env,
    creator: &Address,
    current_price: i128,
    window_ledgers: u32,
) -> i128 {
    if window_ledgers == 0 {
        return current_price;
    }

    let current_ledger = env.ledger().sequence();
    let history = read_price_history(env, creator);
    if history.is_empty() {
        return current_price;
    }

    let window_start = current_ledger.saturating_sub(window_ledgers);

    // Only consider observations at or after the window start. The current
    // price is treated as an observation extending to the current ledger.
    let mut total_weighted: i128 = 0;
    let mut total_ledgers: u32 = 0;

    if history.len() >= 2 {
        let mut i = 1u32;
        while i < history.len() {
            let begin = history.get(i - 1).unwrap();
            let end = history.get(i).unwrap();
            let seg_start = if begin.ledger > window_start {
                begin.ledger
            } else {
                window_start
            };
            let seg_end = end.ledger;
            if seg_end > seg_start {
                let weight = seg_end.saturating_sub(seg_start);
                total_weighted =
                    total_weighted.saturating_add(begin.price.saturating_mul(i128::from(weight)));
                total_ledgers = total_ledgers.saturating_add(weight);
            }
            i += 1;
        }
    }

    // Tail segment: from the last recorded observation (or window start) to the
    // current ledger at the current price.
    let last = history.last().unwrap();
    let tail_start = if last.ledger > window_start {
        last.ledger
    } else {
        window_start
    };
    if current_ledger > tail_start {
        let weight = current_ledger.saturating_sub(tail_start);
        total_weighted =
            total_weighted.saturating_add(current_price.saturating_mul(i128::from(weight)));
        total_ledgers = total_ledgers.saturating_add(weight);
    }

    if total_ledgers == 0 {
        return current_price;
    }
    total_weighted / i128::from(total_ledgers)
}

/// Publishes the [`events::PRICE_QUERIED_EVENT_NAME`] event for an oracle read.
///
/// `caller` is the contract that invoked the oracle entrypoint, `creator` is
/// the key whose price was read, and `price` is the value returned to the
/// caller.
fn emit_price_queried(env: &Env, caller: &Address, creator: &Address, price: i128) {
    env.events().publish(
        events::price_queried_topics(caller),
        events::PriceQueriedEvent {
            caller: caller.clone(),
            creator: creator.clone(),
            price,
        },
    );
}

/// Scaling factor for the per-share vault reward accumulator.
const VAULT_REWARD_PRECISION: i128 = 1_000_000;

/// Maximum number of creator keys accepted by one vault deposit or withdraw.
const VAULT_MAX_BATCH: u32 = 10;

/// Default age in seconds after which an oracle price is flagged stale.
const DEFAULT_ORACLE_STALENESS_SECS: u64 = 3_600;

/// Default timelock delay in seconds (48 hours).
const DEFAULT_TIMELOCK_DELAY_SECS: u64 = 172_800;

/// Maximum configurable timelock delay in seconds (30 days).
const MAX_TIMELOCK_DELAY_SECS: u64 = 2_592_000;

fn read_vault_shares(env: &Env, creator: &Address, holder: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&constants::storage::vault_shares(creator, holder))
        .unwrap_or(0)
}

fn read_vault_total_shares(env: &Env, creator: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&constants::storage::vault_total_shares(creator))
        .unwrap_or(0)
}

/// Moves the rewards a holder earned since their last checkpoint into the
/// pending balance. Must run before a holder's vault shares change.
fn settle_vault_rewards(
    env: &Env,
    creator: &Address,
    holder: &Address,
    shares: u32,
) -> Result<(), ContractError> {
    let acc: i128 = env
        .storage()
        .persistent()
        .get(&constants::storage::vault_reward_acc(creator))
        .unwrap_or(0);
    let checkpoint_key = constants::storage::vault_reward_checkpoint(creator, holder);
    let checkpoint: i128 = env.storage().persistent().get(&checkpoint_key).unwrap_or(0);
    let earned = i128::from(shares)
        .checked_mul(acc.checked_sub(checkpoint).ok_or(ContractError::Overflow)?)
        .ok_or(ContractError::Overflow)?
        / VAULT_REWARD_PRECISION;

    let pending_key = constants::storage::vault_reward_pending(creator, holder);
    let pending: i128 = env.storage().persistent().get(&pending_key).unwrap_or(0);
    let new_pending = pending.checked_add(earned).ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&pending_key, &new_pending);
    env.storage().persistent().set(&checkpoint_key, &acc);
    extend_key_ttl_to_full_window(env, &pending_key);
    extend_key_ttl_to_full_window(env, &checkpoint_key);
    Ok(())
}

/// Writes a holder's vault shares, the creator's total vault shares and the
/// holder's staked balance after a deposit or withdrawal.
fn write_vault_position(
    env: &Env,
    creator: &Address,
    holder: &Address,
    holder_shares: u32,
    total_shares: u32,
    staked_balance: u32,
) {
    let shares_key = constants::storage::vault_shares(creator, holder);
    let total_key = constants::storage::vault_total_shares(creator);
    let staked_key = constants::storage::staked_balance(creator, holder);
    env.storage().persistent().set(&shares_key, &holder_shares);
    env.storage().persistent().set(&total_key, &total_shares);
    env.storage().persistent().set(&staked_key, &staked_balance);
    extend_key_ttl_to_full_window(env, &shares_key);
    extend_key_ttl_to_full_window(env, &total_key);
    extend_key_ttl_to_full_window(env, &staked_key);
}

/// Parameters accepted by `update_config` for validating and applying
/// protocol configuration changes in a single call.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ConfigUpdateParams {
    /// New creator fee basis points (0..=10000). Must sum with `protocol_bps` to ≤ 10000.
    pub creator_bps: u32,
    /// New protocol fee basis points (0..=10000). Must sum with `creator_bps` to ≤ 10000.
    pub protocol_bps: u32,
    /// New bonding curve slope (≥ 0).
    pub curve_slope: i128,
}

/// Reads the configured governance contract address, returning `None` when unset.
pub fn read_governance_address(env: &Env) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&constants::storage::GOVERNANCE_ADDRESS)
}

/// Asserts that `caller` is the registered governance contract address.
fn assert_is_governance(env: &Env, caller: &Address) -> Result<(), ContractError> {
    let governance = read_governance_address(env).ok_or(ContractError::Unauthorized)?;
    if *caller != governance {
        return Err(ContractError::Unauthorized);
    }
    Ok(())
}

/// Emits a [`events::HolderCountChangedEvent`] whenever the holder count
/// crosses the zero boundary.
///
/// `old_count` and `new_count` must already reflect the change; this helper
/// is purely the emit call so the pattern is not duplicated across buy, sell,
/// buyback, burn, transfer, and vesting paths.
fn emit_holder_count_changed(env: &Env, creator: &Address, old_count: u32, new_count: u32) {
    if old_count == new_count {
        return;
    }
    env.events().publish(
        events::holder_count_changed_topics(creator),
        events::HolderCountChangedEvent {
            creator_id: creator.clone(),
            old_count,
            new_count,
            ledger: env.ledger().sequence(),
        },
    );
}

/// Reads the snapshot retention window in ledgers (`0` = no age-based pruning).
fn read_snapshot_retention_ledgers(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&constants::storage::SNAPSHOT_RETENTION_LEDGERS)
        .unwrap_or(0)
}

/// Prunes snapshots for `creator` that are older than the configured retention
/// window (based on `snapshot_ledger`). Works by walking forward from the
/// `OldestSnapshotId` until it finds a snapshot that is still within the
/// retention window (or exhausts known ids up to `current_snapshot_id`).
///
/// This is a best-effort prune: it stops on the first snapshot it cannot find
/// (already pruned or never stored) to avoid runaway loops.
fn prune_old_snapshots(env: &Env, creator: &Address, current_snapshot_id: u32) {
    let retention = read_snapshot_retention_ledgers(env);
    if retention == 0 {
        return;
    }
    let current_ledger = env.ledger().sequence();
    let cutoff = current_ledger.saturating_sub(retention);

    let oldest_key = constants::storage::oldest_snapshot_id(creator);
    let oldest_id: u32 = env.storage().persistent().get(&oldest_key).unwrap_or(0);

    let mut cursor = oldest_id;
    while cursor < current_snapshot_id {
        let meta_key = constants::storage::snapshot_meta(creator, cursor);
        let meta: Option<HolderSnapshotMeta> = env.storage().persistent().get(&meta_key);
        let Some(meta) = meta else {
            // Already pruned or never stored — advance.
            cursor += 1;
            continue;
        };
        if meta.snapshot_ledger >= cutoff {
            // Still within retention window — stop pruning.
            break;
        }

        // Prune: remove per-holder balance and staked-balance entries, then
        // remove the holders list and meta key.
        let holders_key = constants::storage::snapshot_holders(creator, cursor);
        let holders: Option<soroban_sdk::Vec<Address>> =
            env.storage().persistent().get(&holders_key);
        if let Some(holders) = holders {
            for holder in holders.iter() {
                env.storage()
                    .persistent()
                    .remove(&constants::storage::snapshot_balance(
                        creator, cursor, &holder,
                    ));
                env.storage()
                    .persistent()
                    .remove(&constants::storage::snapshot_staked_balance(
                        creator, cursor, &holder,
                    ));
            }
        }
        env.storage().persistent().remove(&holders_key);
        env.storage().persistent().remove(&meta_key);

        env.events().publish(
            events::snapshot_pruned_topics(creator),
            events::SnapshotPrunedEvent {
                creator_id: creator.clone(),
                snapshot_id: cursor,
                ledger: current_ledger,
            },
        );

        cursor += 1;
    }

    // Advance the oldest pointer to avoid re-scanning pruned ids.
    if cursor != oldest_id {
        env.storage().persistent().set(&oldest_key, &cursor);
        extend_key_ttl_to_full_window(env, &oldest_key);
    }
}

/// Maximum bid-ask spread in basis points (50%).
///
/// Caps the on-chain spread setting so the sell price is never forced below
/// 50% of the buy price.
pub const MAX_SPREAD_BPS: u32 = 5_000;

/// Aggregated analytics returned by `get_analytics`.
///
/// All fields are read-only accumulators updated on every trade.
/// Fields are append-only — do not reorder.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AnalyticsView {
    /// Creator address.
    pub creator: Address,
    /// Total number of buy and sell operations (all time).
    pub trade_count: u64,
    /// Number of unique wallets that have ever traded this creator's keys.
    pub unique_traders: u64,
    /// Cumulative trade volume in XLM stroops.
    pub total_volume: i128,
}

/// Identifies which on-chain action produced a reputation change.
///
/// Append-only: new variants must be added at the end so the serialized
/// discriminant stays stable for indexers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum ReputationReason {
    /// A key was launched successfully.
    KeyLaunch = 0,
    /// A configured supply milestone was crossed upward.
    Milestone = 1,
    /// The creator participated in a governance vote.
    GovernanceParticipation = 2,
    /// A trade executed against the creator's key.
    Trade = 3,
    /// The creator deprecated their own key.
    KeyDeprecation = 4,
    /// A governance violation was recorded against the creator.
    GovernanceViolation = 5,
}

/// Per-reason contribution breakdown backing a creator's reputation score.
///
/// Each points field accumulates the signed points contributed by that reason
/// since registration, and each count field records how many times that reason
/// fired. `score` is the floor-bounded sum of every points field, so the
/// breakdown always reconciles with the headline score.
///
/// Fields are append-only — do not reorder.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[contracttype]
pub struct ReputationBreakdown {
    /// Points from successful key launches.
    pub key_launch_points: i128,
    /// Points from supply milestones crossed.
    pub milestone_points: i128,
    /// Points from governance participation.
    pub governance_points: i128,
    /// Points from trading activity.
    pub trade_points: i128,
    /// Penalty points from key deprecation (negative).
    pub deprecation_penalty: i128,
    /// Penalty points from governance violations (negative).
    pub violation_penalty: i128,
    /// Number of key launches credited.
    pub key_launches: u32,
    /// Number of milestones credited.
    pub milestones_reached: u32,
    /// Number of governance votes participated in.
    pub governance_participations: u32,
    /// Number of trades credited.
    pub trades: u32,
    /// Number of governance violations recorded.
    pub governance_violations: u32,
    /// Number of times the score changed.
    pub update_count: u32,
    /// Ledger of the most recent score change.
    pub last_updated_ledger: u32,
}

/// Full reputation state for a creator, returned by `get_reputation`.
///
/// Fields are append-only — do not reorder.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReputationView {
    /// Creator address.
    pub creator: Address,
    /// Current floor-bounded reputation score.
    pub score: i128,
    /// Signed per-reason contribution breakdown.
    pub breakdown: ReputationBreakdown,
    /// Number of key launches credited.
    pub key_launches: u32,
    /// Number of milestones credited.
    pub milestones_reached: u32,
    /// Number of governance votes participated in.
    pub governance_participations: u32,
    /// Number of governance violations recorded.
    pub governance_violations: u32,
    /// `true` when the creator's key has been deprecated.
    pub deprecated: bool,
}

/// Protocol-wide poll quorum-escalation configuration.
///
/// `threshold_bps` is measured against the proposal's own quorum requirement,
/// so a creator-configured `quorum_bps` and this value compose cleanly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EscalationConfig {
    /// Fraction of the quorum requirement that must already be met to qualify
    /// for an extension, in basis points.
    pub threshold_bps: u32,
    /// Ledgers added to the proposal deadline per extension.
    pub extension_ledgers: u32,
    /// Maximum number of extensions a single proposal may consume.
    pub max_extensions: u32,
}

/// Read-only view of a proposal's quorum-escalation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EscalationView {
    /// Proposal id, scoped to the creator.
    pub poll_id: u32,
    /// Current deadline ledger.
    pub expires_at: u32,
    /// Extensions already consumed.
    pub extensions_used: u32,
    /// Maximum extensions allowed by the active config.
    pub max_extensions: u32,
    /// Number of ledgers remaining before the deadline.
    pub ledgers_remaining: u32,
    /// Current participation in basis points of circulating supply.
    pub participation_bps: u32,
    /// Quorum requirement in basis points of circulating supply.
    pub quorum_bps: u32,
    /// `true` when participation has reached the escalation threshold.
    pub eligible: bool,
    /// `true` when no further extension is possible.
    pub exhausted: bool,
}

/// Live market-state view for a creator's key, aggregating the individual
/// read-only getters into a single call.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct KeyMarketStatsView {
    /// Creator address.
    pub creator: Address,
    /// Keys currently in circulation.
    pub supply: u32,
    /// Current buy price from the bonding curve, before fees (XLM stroops).
    pub buy_price: i128,
    /// Current sell price after the configured bid-ask spread (XLM stroops).
    pub sell_price: i128,
    /// Distinct wallets holding at least one key.
    pub holder_count: u32,
    /// Cumulative trade volume (XLM stroops).
    pub volume: i128,
}

/// Applies the configured bid-ask spread to a buy price to produce the sell price.
///
/// `sell_price = buy_price - floor(buy_price * spread_bps / 10_000)`
/// When spread is zero the sell price equals the buy price.
/// Returns `None` on overflow (should never happen with sensible inputs).
fn apply_spread(env: &Env, creator: &Address, buy_price: i128) -> Result<i128, ContractError> {
    let spread_bps: u32 = env
        .storage()
        .persistent()
        .get(&constants::storage::spread_bps(creator))
        .unwrap_or(0);

    if spread_bps == 0 {
        return Ok(buy_price);
    }

    let spread_amount =
        fee::apply_percentage_fee(buy_price, spread_bps).ok_or(ContractError::Overflow)?;
    buy_price
        .checked_sub(spread_amount)
        .ok_or(ContractError::Overflow)
}

/// Increments the per-creator trade count and, on first trade from a wallet,
/// increments the unique trader count. Also accumulates volume.
fn accrue_trade_analytics(
    env: &Env,
    creator: &Address,
    trader: &Address,
    price: i128,
) -> Result<(), ContractError> {
    // Increment trade count
    let trade_key = constants::storage::trade_count(creator);
    let current_trades: u64 = env.storage().persistent().get(&trade_key).unwrap_or(0);
    let new_trades = current_trades
        .checked_add(1)
        .ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&trade_key, &new_trades);

    // Increment unique trader count on first trade from this wallet
    let has_traded_key = constants::storage::has_traded(creator, trader);
    let already_traded: bool = env
        .storage()
        .persistent()
        .get(&has_traded_key)
        .unwrap_or(false);
    if !already_traded {
        env.storage().persistent().set(&has_traded_key, &true);
        let ut_key = constants::storage::unique_trader_count(creator);
        let current_ut: u64 = env.storage().persistent().get(&ut_key).unwrap_or(0);
        let new_ut = current_ut.checked_add(1).ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&ut_key, &new_ut);
    }

    // Accumulate volume
    if price > 0 {
        let vol_key = constants::storage::creator_volume(creator);
        let current_vol: i128 = env.storage().persistent().get(&vol_key).unwrap_or(0);
        let new_vol = current_vol
            .checked_add(price)
            .ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&vol_key, &new_vol);
    }

    // Trading activity is a positive reputation signal, awarded on the same
    // path for buys and sells so both flows feed the score identically.
    accrue_reputation_on_trade(env, creator)?;

    Ok(())
}

/// Applies a signed reputation delta to a creator's score and breakdown, then
/// emits [`events::ReputationUpdatedEvent`].
///
/// The score is floored at [`REPUTATION_MIN_SCORE`]: a decrement that would take
/// it below zero saturates at zero rather than wrapping into a negative or
/// panicking. `delta` is recorded verbatim in the event so consumers can see
/// the applied change, while `new_score` reflects the clamped result.
///
/// A `delta` of `0` is a no-op and emits nothing, which keeps callers free to
/// compute a conditional contribution without a separate guard.
fn apply_reputation_delta(
    env: &Env,
    creator: &Address,
    delta: i128,
    reason: ReputationReason,
) -> Result<(), ReputationError> {
    if delta == 0 {
        return Ok(());
    }

    let score_key = constants::storage::reputation_score(creator);
    let old_score: i128 = env.storage().persistent().get(&score_key).unwrap_or(0);

    // Floor the score at zero instead of letting decrements go negative.
    let new_score = if delta < 0 && old_score + delta < REPUTATION_MIN_SCORE {
        REPUTATION_MIN_SCORE
    } else {
        old_score
            .checked_add(delta)
            .ok_or(ReputationError::Overflow)?
    };

    let breakdown_key = constants::storage::reputation_breakdown(creator);
    let mut breakdown: ReputationBreakdown = env
        .storage()
        .persistent()
        .get(&breakdown_key)
        .unwrap_or_default();

    match reason {
        ReputationReason::KeyLaunch => {
            breakdown.key_launch_points = breakdown
                .key_launch_points
                .checked_add(delta)
                .ok_or(ReputationError::Overflow)?;
            breakdown.key_launches = breakdown
                .key_launches
                .checked_add(1)
                .ok_or(ReputationError::Overflow)?;
        }
        ReputationReason::Milestone => {
            breakdown.milestone_points = breakdown
                .milestone_points
                .checked_add(delta)
                .ok_or(ReputationError::Overflow)?;
            breakdown.milestones_reached = breakdown
                .milestones_reached
                .checked_add(1)
                .ok_or(ReputationError::Overflow)?;
        }
        ReputationReason::GovernanceParticipation => {
            breakdown.governance_points = breakdown
                .governance_points
                .checked_add(delta)
                .ok_or(ReputationError::Overflow)?;
            breakdown.governance_participations = breakdown
                .governance_participations
                .checked_add(1)
                .ok_or(ReputationError::Overflow)?;
        }
        ReputationReason::Trade => {
            breakdown.trade_points = breakdown
                .trade_points
                .checked_add(delta)
                .ok_or(ReputationError::Overflow)?;
            breakdown.trades = breakdown
                .trades
                .checked_add(1)
                .ok_or(ReputationError::Overflow)?;
        }
        ReputationReason::KeyDeprecation => {
            breakdown.deprecation_penalty = breakdown
                .deprecation_penalty
                .checked_add(delta)
                .ok_or(ReputationError::Overflow)?;
        }
        ReputationReason::GovernanceViolation => {
            breakdown.violation_penalty = breakdown
                .violation_penalty
                .checked_add(delta)
                .ok_or(ReputationError::Overflow)?;
            breakdown.governance_violations = breakdown
                .governance_violations
                .checked_add(1)
                .ok_or(ReputationError::Overflow)?;
        }
    }

    breakdown.update_count = breakdown
        .update_count
        .checked_add(1)
        .ok_or(ReputationError::Overflow)?;
    breakdown.last_updated_ledger = env.ledger().sequence();

    env.storage().persistent().set(&score_key, &new_score);
    extend_key_ttl_to_full_window(env, &score_key);
    env.storage().persistent().set(&breakdown_key, &breakdown);
    extend_key_ttl_to_full_window(env, &breakdown_key);

    env.events().publish(
        events::reputation_updated_topics(creator),
        events::ReputationUpdatedEvent {
            creator: creator.clone(),
            old_score,
            new_score,
            delta,
            reason,
            ledger: breakdown.last_updated_ledger,
        },
    );

    Ok(())
}

/// Reads a creator's stored reputation score, defaulting to zero.
fn read_reputation_score(env: &Env, creator: &Address) -> i128 {
    let key = constants::storage::reputation_score(creator);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Reads a creator's stored reputation breakdown, defaulting to all zeros.
fn read_reputation_breakdown(env: &Env, creator: &Address) -> ReputationBreakdown {
    let key = constants::storage::reputation_breakdown(creator);
    env.storage().persistent().get(&key).unwrap_or_default()
}

/// Awards the positive reputation for a milestone crossed upward.
///
/// Called from the milestone emitter so a configured supply milestone feeds the
/// creator's standing exactly once per crossing.
fn accrue_reputation_on_milestone(env: &Env, creator: &Address) -> Result<(), ContractError> {
    apply_reputation_delta(
        env,
        creator,
        REPUTATION_MILESTONE_POINTS,
        ReputationReason::Milestone,
    )
    .map_err(|_| ContractError::Overflow)
}

/// Awards the positive reputation for a completed trade.
///
/// Called from the trade-analytics accumulator so buy and sell share one path.
fn accrue_reputation_on_trade(env: &Env, creator: &Address) -> Result<(), ContractError> {
    apply_reputation_delta(
        env,
        creator,
        REPUTATION_TRADE_POINTS,
        ReputationReason::Trade,
    )
    .map_err(|_| ContractError::Overflow)
}

/// Awards the positive reputation for a governance vote cast on a creator's key.
///
/// Called from `cast_vote` so only votes that actually settle are credited;
/// re-voting before expiry still counts as a fresh participation.
pub(crate) fn accrue_reputation_on_governance_participation(
    env: &Env,
    creator: &Address,
) -> Result<(), ContractError> {
    apply_reputation_delta(
        env,
        creator,
        REPUTATION_GOVERNANCE_PARTICIPATION_POINTS,
        ReputationReason::GovernanceParticipation,
    )
    .map_err(|_| ContractError::Overflow)
}

/// Converts a poll's total vote weight into basis points of circulating supply.
///
/// Returns `0` when the creator has no circulating supply, which keeps a
/// zero-supply poll from reporting full participation.
fn escalation_participation_bps(
    total_weight: u32,
    circulating_supply: u32,
) -> Result<u32, EscalationError> {
    if circulating_supply == 0 {
        return Ok(0);
    }
    let bps = (total_weight as u128)
        .checked_mul(10_000)
        .ok_or(EscalationError::Overflow)?
        / circulating_supply as u128;
    // Participation is clamped to 100% so a poll whose voters collectively hold
    // more than the tracked supply cannot outrank a full-quorum threshold.
    Ok(bps.min(10_000) as u32)
}

/// Decides whether a proposal qualifies for a quorum-escalation extension.
///
/// The proposal must have consumed fewer than `max_extensions` extensions, must
/// carry a non-zero quorum requirement, must not have reached that requirement
/// yet, and its participation must already have reached `threshold_bps` of it.
///
/// The two disqualifying quorum cases are deliberate:
///
/// - `quorum_bps == 0` means [`events::close_poll`] imposes no quorum check at
///   all, so there is no participation shortfall for escalation to rescue.
/// - `participation_bps >= quorum_bps` means the proposal can already close; an
///   extension would only consume one of the creator's budgeted extensions.
fn is_escalation_eligible(
    participation_bps: u32,
    quorum_bps: u32,
    threshold_bps: u32,
    max_extensions: u32,
    extensions_used: u32,
) -> bool {
    if max_extensions == 0 || extensions_used >= max_extensions {
        return false;
    }
    if quorum_bps == 0 {
        return false;
    }
    if participation_bps >= quorum_bps {
        return false;
    }
    // "Close to quorum": participation has reached `threshold_bps` of the
    // proposal's own requirement. `threshold_bps` is validated to be non-zero by
    // `set_escalation_config`, so a zero-participation poll never qualifies.
    let required_bps = (quorum_bps as u128).saturating_mul(threshold_bps as u128) / 10_000;
    (participation_bps as u128) >= required_bps
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

    /// Purchases multiple keys in a single transaction, subject to the creator's
    /// per-transaction max buy quantity limit if configured.
    ///
    /// Each key is priced individually along the bonding curve (or at the auction
    /// price during an auction phase), so the total cost increases with quantity.
    /// The `max_price` parameter acts as a slippage guard on the total cost.
    pub fn buy_keys(
        env: Env,
        creator: Address,
        buyer: Address,
        quantity: u32,
        payment: i128,
        max_price: Option<i128>,
    ) -> Result<u32, ContractError> {
        Self::buy_keys_with_referrer(env, creator, buyer, quantity, payment, max_price, None)
    }

    pub fn buy_keys_with_referrer(
        env: Env,
        creator: Address,
        buyer: Address,
        quantity: u32,
        payment: i128,
        max_price: Option<i128>,
        referrer: Option<Address>,
    ) -> Result<u32, ContractError> {
        buyer.require_auth();
        assert_global_trading_not_halted(&env)?;
        assert_key_trading_not_paused(&env, &creator)?;
        assert_not_paused(&env)?;
        assert_not_blacklisted(&env, &buyer)?;
        assert_before_global_deadline(&env)?;
        assert_position_not_frozen(&env, &creator, &buyer)?;

        // Reject buys on deprecated keys immediately — before any price or fee math.
        if env
            .storage()
            .persistent()
            .has(&constants::storage::deprecated_key(&creator))
        {
            return Err(ContractError::KeyDeprecated);
        }

        if quantity == 0 {
            return Err(ContractError::NotPositiveAmount);
        }

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

        // Enforce max buy quantity limit if configured.
        if let Some(max_qty) = env
            .storage()
            .persistent()
            .get::<DataKey, u32>(&constants::storage::max_buy_quantity(&creator))
        {
            if quantity > max_qty {
                return Err(ContractError::QuantityExceedsLimit);
            }
        }

        let balance_key = constants::storage::holder_balance_key(&creator, &buyer);
        // Missing balance entries are treated as zero to keep storage sparse.
        let mut current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        let mut total_price = 0i128;
        let mut keys_bought = 0u32;

        // Process each key individually so the bonding curve price is applied
        // incrementally and all per-key guards (caps, cooldown, flash-loan) fire
        // for every key in the batch.
        for _ in 0..quantity {
            let key_price = if in_auction {
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
                let post_price =
                    compute_bonding_curve_price(&env, &creator, base_price, post_supply)?;
                let threshold_pct: u32 = env
                    .storage()
                    .persistent()
                    .get(&constants::storage::CIRCUIT_BREAKER_THRESHOLD)
                    .unwrap_or(30);
                if pre_price > 0 && post_price > pre_price {
                    let price_change = (post_price - pre_price) as u128;
                    let pre_price_u128 = pre_price as u128;
                    let threshold_pct_u128 = threshold_pct as u128;
                    if price_change
                        .checked_mul(100)
                        .ok_or(ContractError::Overflow)?
                        >= pre_price_u128
                            .checked_mul(threshold_pct_u128)
                            .ok_or(ContractError::Overflow)?
                    {
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

            // Check max supply cap if set
            if let Some(max_supply) = env
                .storage()
                .persistent()
                .get::<DataKey, u32>(&constants::storage::max_supply(&creator))
            {
                if profile.supply >= max_supply {
                    break;
                }
            }

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
                    break;
                }
            }

            // Check the per-creator percentage holding cap.
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
                    let max_allowed = ((i128::from(post_buy_supply) * i128::from(cap_bps))
                        / i128::from(fee::BPS_MAX)) as u32;
                    if post_buy_balance > max_allowed {
                        break;
                    }
                }
            }

            if total_price
                .checked_add(key_price)
                .is_none_or(|t| t > payment)
            {
                break;
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

            // Emit HolderCountChanged when a new holder enters (first buy).
            if current_balance == 0 {
                emit_holder_count_changed(
                    &env,
                    &creator,
                    profile.holder_count - 1,
                    profile.holder_count,
                );
            }

            profile.supply = profile
                .supply
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;

            // Supply and holder_count must always move together with buyer balance writes.
            write_creator_supply(&env, &creator, profile.supply);
            emit_milestone_crossings(&env, &creator, profile.supply - 1, profile.supply)?;

            // Record the key creation ledger on the first buy for launch penalty tracking.
            if profile.supply == 1 {
                let created_key = constants::storage::created_at_ledger(&creator);
                env.storage()
                    .persistent()
                    .set(&created_key, &env.ledger().sequence());
                extend_key_ttl_to_full_window(&env, &created_key);
            }

            let new_balance = current_balance
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
            // Balance key is scoped by (creator, holder) so creator positions cannot collide.
            env.storage().persistent().set(&balance_key, &new_balance);
            // Grant the balance entry the full TTL window so long-held positions
            // survive the same horizon as creator state between trades.
            extend_key_ttl_to_full_window(&env, &balance_key);

            // Flash-loan guard (issue #781): record this buy's ledger so sell_key can
            // reject a same-ledger sell of the position just bought.
            let last_buy_ledger_key = constants::storage::last_buy_ledger(&creator, &buyer);
            env.storage()
                .persistent()
                .set(&last_buy_ledger_key, &env.ledger().sequence());
            extend_key_ttl_to_full_window(&env, &last_buy_ledger_key);

            // Record the purchase time on the holder's entry so sells can enforce
            // the anti-flash-trade lockup window.
            let last_buy_key = constants::storage::last_buy_timestamp(&creator, &buyer);
            env.storage()
                .persistent()
                .set(&last_buy_key, &env.ledger().timestamp());
            extend_key_ttl_to_full_window(&env, &last_buy_key);

            total_price = total_price
                .checked_add(key_price)
                .ok_or(ContractError::Overflow)?;
            current_balance = new_balance;
            keys_bought = keys_bought.checked_add(1).ok_or(ContractError::Overflow)?;
        }

        if keys_bought == 0 {
            return Err(ContractError::InsufficientPayment);
        }

        // Enforce per-wallet buy cooldown after all keys have been processed.
        let cooldown_ledgers: u32 = env
            .storage()
            .persistent()
            .get(&constants::storage::buy_cooldown(&creator))
            .unwrap_or(0);
        if cooldown_ledgers > 0 {
            let last_buy_ledger_key = constants::storage::last_buy_ledger(&creator, &buyer);
            if let Some(last_ledger) = env
                .storage()
                .persistent()
                .get::<DataKey, u32>(&last_buy_ledger_key)
            {
                let current_ledger = env.ledger().sequence();
                let elapsed = current_ledger.saturating_sub(last_ledger);
                if elapsed < cooldown_ledgers {
                    let ledgers_remaining = cooldown_ledgers - elapsed;
                    env.events().publish(
                        events::cooldown_blocked_topics(&creator, &buyer),
                        events::CooldownBlockedEvent {
                            wallet: buyer.clone(),
                            creator_id: creator.clone(),
                            ledgers_remaining,
                        },
                    );
                    // Roll back the supply and balance changes.
                    return Err(ContractError::CooldownActive);
                }
            }
        }

        assert_buy_price_slippage(&env, &creator, total_price, max_price)?;

        // Deduct the protocol trade fee before computing the creator payout so
        // the fee collector is paid ahead of every other participant. A share
        // of the fee is routed into the creator's staking rewards pool.
        let net_amount = collect_protocol_trade_fee(&env, &creator, total_price)?;

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
            let royalty_amount = fee::apply_percentage_fee(total_price, royalty.buy_fee_bps)
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
                .checked_add(keys_bought)
                .ok_or(ContractError::Overflow)?;
            env.storage().persistent().set(&auction_config_key, &config);

            env.events().publish(
                events::auction_purchase_topics(&creator, &buyer),
                events::AuctionPurchaseEvent {
                    buyer: buyer.clone(),
                    creator_id: creator.clone(),
                    quantity: keys_bought,
                    price_paid: total_price,
                    new_supply: profile.supply,
                    auction_sold: config.auction_sold,
                    ledger: env.ledger().sequence(),
                },
            );
        } else {
            let buy_event_data = events::KeysBoughtEvent {
                buyer: buyer.clone(),
                creator_id: creator.clone(),
                quantity: keys_bought,
                price_paid: total_price,
                new_supply: profile.supply,
                ledger: env.ledger().sequence(),
            };

            env.events()
                .publish(events::buy_event_topics(&creator, &buyer), buy_event_data);
        }

        record_trade_price_snapshot(&env, &creator);

        // Extend TTL for creator storage after successful buy
        extend_creator_ttl(&env, &creator);

        Ok(profile.supply)
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
        assert_global_trading_not_halted(&env)?;
        assert_key_trading_not_paused(&env, &creator)?;
        assert_not_paused(&env)?;
        assert_not_blacklisted(&env, &buyer)?;
        assert_before_global_deadline(&env)?;
        assert_position_not_frozen(&env, &creator, &buyer)?;

        // Reject buys on deprecated keys immediately — before any price or fee math.
        if env
            .storage()
            .persistent()
            .has(&constants::storage::deprecated_key(&creator))
        {
            return Err(ContractError::KeyDeprecated);
        }

        if payment <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        if let Some(ref referrer_addr) = referrer {
            if *referrer_addr == buyer || *referrer_addr == creator {
                return Err(ContractError::InvalidReferrer);
            }
        }

        let (referrer, from_registration) = resolve_registered_referrer(&env, &buyer, referrer);

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
            if pre_price > 0 && post_price > pre_price {
                let price_change = (post_price - pre_price) as u128;
                let pre_price_u128 = pre_price as u128;
                let threshold_pct_u128 = threshold_pct as u128;
                if price_change
                    .checked_mul(100)
                    .ok_or(ContractError::Overflow)?
                    >= pre_price_u128
                        .checked_mul(threshold_pct_u128)
                        .ok_or(ContractError::Overflow)?
                {
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

        assert_buy_price_slippage(&env, &creator, price, max_price)?;

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
                    return Err(ContractError::MaxHoldingExceeded);
                }
            }
        }

        // Enforce the per-wallet buy cooldown: once a cooldown is configured
        // by the creator via `set_buy_cooldown`, the same wallet cannot buy
        // again until `cooldown_ledgers` have elapsed since their last buy.
        let cooldown_ledgers: u32 = env
            .storage()
            .persistent()
            .get(&constants::storage::buy_cooldown(&creator))
            .unwrap_or(0);
        if cooldown_ledgers > 0 {
            let last_buy_ledger_key = constants::storage::last_buy_ledger(&creator, &buyer);
            if let Some(last_ledger) = env
                .storage()
                .persistent()
                .get::<DataKey, u32>(&last_buy_ledger_key)
            {
                let current_ledger = env.ledger().sequence();
                let elapsed = current_ledger.saturating_sub(last_ledger);
                if elapsed < cooldown_ledgers {
                    let ledgers_remaining = cooldown_ledgers - elapsed;
                    env.events().publish(
                        events::cooldown_blocked_topics(&creator, &buyer),
                        events::CooldownBlockedEvent {
                            wallet: buyer.clone(),
                            creator_id: creator.clone(),
                            ledgers_remaining,
                        },
                    );
                    return Err(ContractError::CooldownActive);
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

        // Emit HolderCountChanged when a new holder enters (first buy).
        if current_balance == 0 {
            emit_holder_count_changed(
                &env,
                &creator,
                profile.holder_count - 1,
                profile.holder_count,
            );
        }

        profile.supply = profile
            .supply
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;

        // Supply and holder_count must always move together with buyer balance writes.
        write_creator_supply(&env, &creator, profile.supply);
        emit_milestone_crossings(&env, &creator, profile.supply - 1, profile.supply)?;

        // Record the key creation ledger on the first buy for launch penalty tracking.
        if profile.supply == 1 {
            let created_key = constants::storage::created_at_ledger(&creator);
            env.storage()
                .persistent()
                .set(&created_key, &env.ledger().sequence());
            extend_key_ttl_to_full_window(&env, &created_key);
        }

        let new_balance = current_balance
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;
        // Balance key is scoped by (creator, holder) so creator positions cannot collide.
        env.storage().persistent().set(&balance_key, &new_balance);
        // Grant the balance entry the full TTL window so long-held positions
        // survive the same horizon as creator state between trades.
        extend_key_ttl_to_full_window(&env, &balance_key);

        // Flash-loan guard (issue #781): record this buy's ledger so sell_key can
        // reject a same-ledger sell of the position just bought.
        // Also used by the per-wallet cooldown guard to track the most recent
        // successful buy ledger for each (creator, holder) pair.
        let last_buy_ledger_key = constants::storage::last_buy_ledger(&creator, &buyer);
        env.storage()
            .persistent()
            .set(&last_buy_ledger_key, &env.ledger().sequence());
        extend_key_ttl_to_full_window(&env, &last_buy_ledger_key);

        // Record the purchase time on the holder's entry so sells can enforce
        // the anti-flash-trade lockup window.
        let last_buy_key = constants::storage::last_buy_timestamp(&creator, &buyer);
        env.storage()
            .persistent()
            .set(&last_buy_key, &env.ledger().timestamp());
        extend_key_ttl_to_full_window(&env, &last_buy_key);

        // Deduct the protocol trade fee before computing the creator payout so
        // the fee collector is paid ahead of every other participant. A share
        // of the fee is routed into the creator's staking rewards pool.
        let net_amount = collect_protocol_trade_fee(&env, &creator, price)?;

        if let Some(config) = read_protocol_fee_config(&env) {
            let (creator_fee, protocol_fee) =
                fee::checked_compute_fee_split(net_amount, config.creator_bps, config.protocol_bps)
                    .ok_or(ContractError::Overflow)?;

            credit_creator_fee(&env, &creator, creator_fee)?;
            credit_staking_rewards_pool(&env, &creator, protocol_fee)?;

            // Split protocol fee between treasury and referrer only when a referrer is provided
            if let Some(referrer_addr) = referrer {
                // Registered referrals pay the admin-configured share of the
                // protocol fee (default 50%); explicit referrers keep the flat split.
                let referral_amount = if from_registration {
                    let bps: u32 = env
                        .storage()
                        .persistent()
                        .get(&constants::storage::referral_fee_bps())
                        .unwrap_or(5_000);
                    protocol_fee
                        .checked_mul(i128::from(bps))
                        .ok_or(ContractError::Overflow)?
                        / i128::from(fee::BPS_MAX)
                } else {
                    protocol_fee / 2
                };
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

                    if from_registration {
                        env.events().publish(
                            (events::referral_reward_allocated_topics(),),
                            events::ReferralRewardAllocatedEvent {
                                referee: buyer.clone(),
                                referrer: referrer_addr.clone(),
                                amount: referral_amount,
                            },
                        );
                    }
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

        record_trade_price_snapshot(&env, &creator);

        // Update analytics accumulators atomically with the trade.
        accrue_trade_analytics(&env, &creator, &buyer, price)?;

        // Extend TTL for creator storage after successful buy
        extend_creator_ttl(&env, &creator);

        Ok(profile.supply)
    }

    /// Purchase multiple keys in a single transaction.
    ///
    /// The buyer pays `payment` (total across all keys) and receives `quantity`
    /// keys. Each key is priced along the bonding curve (or at the auction
    /// price when in auction phase), and all per-key side-effects (fees,
    /// dividends, TTL extension, events) are applied per key.
    ///
    /// # Limits
    ///
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
        assert_key_trading_not_paused(&env, &creator)?;
        assert_not_paused(&env)?;
        assert_not_blacklisted(&env, &seller)?;
        assert_position_not_frozen(&env, &creator, &seller)?;

        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

        let balance_key = constants::storage::holder_balance_key(&creator, &seller);
        // Missing balance entries are interpreted as zero and rejected consistently.
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if current_balance == 0 {
            return Err(ContractError::InsufficientBalance);
        }

        // Flash-loan guard (issue #781): reject a sell in the same ledger as the
        // seller's most recent buy, closing the risk-free buy-then-sell vector
        // within a single transaction/ledger.
        let last_buy_ledger_key = constants::storage::last_buy_ledger(&creator, &seller);
        let last_buy_ledger: Option<u32> = env.storage().persistent().get(&last_buy_ledger_key);
        let current_ledger = env.ledger().sequence();
        if last_buy_ledger == Some(current_ledger) {
            env.events().publish(
                events::flash_loan_blocked_topics(&seller, &creator),
                events::FlashLoanBlockedEvent {
                    wallet: seller.clone(),
                    key_id: creator.clone(),
                    ledger: current_ledger,
                },
            );
            return Err(ContractError::FlashLoanDetected);
        }

        // Check liquid balance (total balance - staked balance)
        let staked_balance_key = constants::storage::staked_balance(&creator, &seller);
        let staked_balance: u32 = env
            .storage()
            .persistent()
            .get(&staked_balance_key)
            .unwrap_or(0);
        let liquid_balance = current_balance
            .saturating_sub(staked_balance)
            .saturating_sub(read_self_frozen_balance(&env, &creator, &seller));

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
                    return Err(ContractError::AllocationLocked);
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
        let curve_price = compute_bonding_curve_price(&env, &creator, base_price, sell_supply)?;
        // Apply bid-ask spread after bonding curve, before slippage check.
        let price = apply_spread(&env, &creator, curve_price)?;

        // Settle dividends before balance changes so earnings are captured at old balance.
        settle_holder_dividends(&env, &creator, &seller, current_balance)?;

        assert_sell_proceeds_slippage(&env, &creator, price, min_proceeds)?;

        let new_balance = current_balance
            .checked_sub(1)
            .ok_or(ContractError::SellUnderflow)?;
        profile.supply = profile
            .supply
            .checked_sub(1)
            .ok_or(ContractError::SellUnderflow)?;
        emit_milestone_crossings(&env, &creator, profile.supply + 1, profile.supply)?;

        if new_balance == 0 {
            profile.holder_count = profile
                .holder_count
                .checked_sub(1)
                .ok_or(ContractError::SellUnderflow)?;
        }

        // Persist holder_count before write_creator_supply reads the profile.
        let key = constants::storage::creator(&creator);
        env.storage().persistent().set(&key, &profile);

        // Emit HolderCountChanged when a holder fully exits (balance hits zero).
        if new_balance == 0 {
            emit_holder_count_changed(
                &env,
                &creator,
                profile.holder_count + 1,
                profile.holder_count,
            );
        }

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

        // Launch penalty: if the sell occurs within the launch window
        // (7 days / 120,960 ledgers of the key's creation), deduct a
        // configurable penalty from the proceeds and credit it to the
        // creator fee balance.
        let gross_proceeds = compute_sell_proceeds(&env, price).unwrap_or(0);

        // Sell tax: deduct the creator's configured tax from their proceeds and
        // forward it to the buyback pool. The pool credit and the proceeds
        // reduction happen in the same call, so a sell either does both or
        // neither. `proceeds` below is what the seller actually receives.
        let (proceeds, tax_amount, pool_balance_after) =
            Self::collect_sell_tax(&env, &creator, gross_proceeds)?;
        if tax_amount > 0 {
            // Until an admin assigns a pool the collected tax is held against
            // the zero address rather than a fabricated recipient, so the
            // unassigned balance is always visible in the event.
            let pool: Address = env
                .storage()
                .persistent()
                .get(&constants::storage::BUYBACK_POOL_ADDRESS)
                .unwrap_or_else(|| zero_address(&env));
            env.events().publish(
                events::sell_tax_collected_topics(&creator, &seller),
                events::SellTaxCollectedEvent {
                    creator: creator.clone(),
                    seller: seller.clone(),
                    amount: tax_amount,
                    pool,
                    tax_bps: Self::get_sell_tax_bps(env.clone(), creator.clone()),
                    gross_proceeds,
                    net_proceeds: proceeds,
                    pool_balance: pool_balance_after,
                    ledger: env.ledger().sequence(),
                },
            );
        }

        if let Some(created_at) = env
            .storage()
            .persistent()
            .get::<DataKey, u32>(&constants::storage::created_at_ledger(&creator))
        {
            let current_ledger = env.ledger().sequence();
            if current_ledger.checked_sub(created_at).unwrap_or(u32::MAX)
                < crate::LAUNCH_PENALTY_WINDOW_LEDGERS
            {
                let penalty_bps: u32 = env
                    .storage()
                    .persistent()
                    .get::<DataKey, u32>(&constants::storage::launch_penalty_bps(&creator))
                    .unwrap_or(crate::DEFAULT_LAUNCH_PENALTY_BPS);
                let capped_bps = penalty_bps.min(crate::MAX_LAUNCH_PENALTY_BPS);
                if capped_bps > 0 {
                    let penalty_amount =
                        crate::fee::apply_percentage_fee(proceeds, capped_bps).unwrap_or(0);
                    if penalty_amount > 0 {
                        credit_creator_fee_balance(&env, &creator, penalty_amount)?;
                        env.events().publish(
                            events::launch_penalty_applied_topics(&creator, &seller),
                            events::LaunchPenaltyAppliedEvent {
                                creator_id: creator.clone(),
                                seller: seller.clone(),
                                penalty_bps: capped_bps,
                                penalty_amount,
                                ledger: env.ledger().sequence(),
                            },
                        );
                    }
                }
            }
        }

        let sell_event_data = events::KeysSoldEvent {
            seller: seller.clone(),
            creator_id: creator.clone(),
            quantity: 1,
            proceeds,
            new_supply: profile.supply,
            ledger: env.ledger().sequence(),
        };

        env.events().publish(
            (events::SELL_EVENT_NAME, creator.clone(), seller.clone()),
            sell_event_data,
        );

        record_trade_price_snapshot(&env, &creator);

        // Update analytics accumulators atomically with the trade.
        accrue_trade_analytics(&env, &creator, &seller, price)?;

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
        emit_milestone_crossings(&env, &creator, profile.supply + amount, profile.supply)?;

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

    // =========================================================================
    // #834 — Key deprecation and holder buybacks
    // =========================================================================

    /// Deprecates a creator key, disabling new buys and initiating an orderly
    /// shutdown via a fixed-price holder buyback.
    ///
    /// The creator must escrow `circulating_supply * buyback_price_per_key` XLM
    /// (`escrow_payment`) at the time of calling. Holders can then call
    /// [`CreatorKeysContract::redeem`] to exchange their keys for the fixed
    /// buyback price.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `caller != creator`.
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    /// - [`ContractError::NotPositiveAmount`] if `buyback_price_per_key <= 0`.
    /// - [`ContractError::KeyDeprecated`] if the key is already deprecated.
    /// - [`ContractError::InsufficientEscrow`] if `escrow_payment` is less than
    ///   `circulating_supply * buyback_price_per_key`.
    /// - [`ContractError::ProtocolPaused`] if the contract is paused.
    pub fn deprecate_key(
        env: Env,
        creator: Address,
        caller: Address,
        buyback_price_per_key: i128,
        escrow_payment: i128,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        assert_not_paused(&env)?;

        if caller != creator {
            return Err(ContractError::Unauthorized);
        }
        if buyback_price_per_key <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let profile = read_registered_creator_profile(&env, &creator)?;

        // Reject if already deprecated.
        if env
            .storage()
            .persistent()
            .has(&constants::storage::deprecated_key(&creator))
        {
            return Err(ContractError::KeyDeprecated);
        }

        // Compute the required escrow: circulating_supply * buyback_price_per_key.
        let circulating_supply = profile.supply;
        let required_escrow = (circulating_supply as i128)
            .checked_mul(buyback_price_per_key)
            .ok_or(ContractError::Overflow)?;

        if escrow_payment < required_escrow {
            return Err(ContractError::InsufficientEscrow);
        }

        // Persist the deprecation marker (stores the fixed buyback price).
        let dep_key = constants::storage::deprecated_key(&creator);
        env.storage()
            .persistent()
            .set(&dep_key, &buyback_price_per_key);
        extend_key_ttl_to_full_window(&env, &dep_key);

        // Persist the escrow balance (capped at required_escrow; any overpayment
        // is treated as excess and not credited to the escrow pool).
        let escrow_key = constants::storage::deprecation_escrow(&creator);
        env.storage()
            .persistent()
            .set(&escrow_key, &required_escrow);
        extend_key_ttl_to_full_window(&env, &escrow_key);

        env.events().publish(
            events::key_deprecated_topics(&creator),
            events::KeyDeprecatedEvent {
                creator: creator.clone(),
                buyback_price_per_key,
                circulating_supply,
                total_escrow: required_escrow,
                ledger: env.ledger().sequence(),
            },
        );

        // Deprecating a key is a negative reputation signal for the creator.
        apply_reputation_delta(
            &env,
            &creator,
            -REPUTATION_DEPRECATION_PENALTY,
            ReputationReason::KeyDeprecation,
        )
        .map_err(|_| ContractError::Overflow)?;

        Ok(())
    }

    /// Redeems all keys held by `holder` for a deprecated creator key.
    ///
    /// Transfers `holder_balance * buyback_price_per_key` XLM from the escrow
    /// pool to the holder, burns the holder's keys, and decrements the supply.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    /// - [`ContractError::KeyNotDeprecated`] if the creator is registered but
    ///   has not called [`CreatorKeysContract::deprecate_key`].
    /// - [`ContractError::InsufficientBalance`] if the holder has no keys.
    /// - [`ContractError::InsufficientEscrow`] if the escrow pool is unexpectedly
    ///   short (should not happen under normal conditions).
    /// - [`ContractError::ProtocolPaused`] if the contract is paused.
    pub fn redeem(env: Env, creator: Address, holder: Address) -> Result<i128, ContractError> {
        holder.require_auth();
        assert_not_paused(&env)?;

        // Resolve the profile first so an unknown creator reports NotRegistered
        // rather than KeyNotDeprecated.
        let mut profile = read_registered_creator_profile(&env, &creator)?;

        // The key must be deprecated before holders can redeem.
        let dep_key = constants::storage::deprecated_key(&creator);
        let buyback_price_per_key: i128 = env
            .storage()
            .persistent()
            .get(&dep_key)
            .ok_or(ContractError::KeyNotDeprecated)?;

        let balance_key = constants::storage::holder_balance_key(&creator, &holder);
        let holder_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        if holder_balance == 0 {
            return Err(ContractError::InsufficientBalance);
        }

        // Compute payout.
        let payout = (holder_balance as i128)
            .checked_mul(buyback_price_per_key)
            .ok_or(ContractError::Overflow)?;

        // Deduct from escrow.
        let escrow_key = constants::storage::deprecation_escrow(&creator);
        let current_escrow: i128 = env.storage().persistent().get(&escrow_key).unwrap_or(0);

        if current_escrow < payout {
            return Err(ContractError::InsufficientEscrow);
        }

        let new_escrow = current_escrow
            .checked_sub(payout)
            .ok_or(ContractError::Overflow)?;

        // Burn holder's keys and update supply / holder count.
        profile.supply = profile
            .supply
            .checked_sub(holder_balance)
            .ok_or(ContractError::SellUnderflow)?;
        profile.holder_count = profile
            .holder_count
            .checked_sub(1)
            .ok_or(ContractError::SellUnderflow)?;

        // Persist updated state.
        let creator_key = constants::storage::creator(&creator);
        env.storage().persistent().set(&creator_key, &profile);
        env.storage().persistent().remove(&balance_key);

        if new_escrow == 0 {
            env.storage().persistent().remove(&escrow_key);
        } else {
            env.storage().persistent().set(&escrow_key, &new_escrow);
            extend_key_ttl_to_full_window(&env, &escrow_key);
        }

        env.events().publish(
            events::keys_redeemed_topics(&creator, &holder),
            events::KeysRedeemedEvent {
                creator: creator.clone(),
                holder: holder.clone(),
                quantity: holder_balance,
                payout,
                new_supply: profile.supply,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(payout)
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
        env.events()
            .publish((events::PAUSE_EVENT_NAME, admin.clone()), ());
        env.events().publish(
            events::pause_state_changed_topics(),
            events::PauseStateChangedEvent {
                paused: true,
                caller: admin,
            },
        );
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
            .publish((events::UNPAUSE_EVENT_NAME, admin.clone()), ());
        env.events().publish(
            events::pause_state_changed_topics(),
            events::PauseStateChangedEvent {
                paused: false,
                caller: admin,
            },
        );
        Ok(())
    }

    /// Read-only view: returns whether the protocol is currently paused.
    pub fn get_is_paused(env: Env) -> bool {
        is_paused(&env)
    }

    /// Sets the supply milestones that emit `MilestoneCrossed` events (#887).
    ///
    /// Only the protocol admin may call this. Thresholds must be positive and
    /// strictly ascending; the 1-based position of a threshold is its tier.
    pub fn set_supply_milestones(
        env: Env,
        admin: Address,
        milestones: Vec<u32>,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        let mut previous = 0u32;
        for milestone in milestones.iter() {
            if milestone <= previous {
                return Err(ContractError::NotPositiveAmount);
            }
            previous = milestone;
        }
        env.storage()
            .persistent()
            .set(&constants::storage::SUPPLY_MILESTONES, &milestones);
        extend_key_ttl_to_full_window(&env, &constants::storage::SUPPLY_MILESTONES);
        Ok(())
    }

    /// Read-only view: returns the configured supply milestones (empty if unset).
    pub fn get_supply_milestones(env: Env) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&constants::storage::SUPPLY_MILESTONES)
            .unwrap_or(Vec::new(&env))
    }

    /// Upgrades the contract WASM to `new_wasm_hash` and increments the version.
    ///
    /// Only the protocol admin may call this. Emits an `UpgradeExecuted` event
    /// carrying the old and new version.
    pub fn upgrade(
        env: Env,
        admin: Address,
        new_wasm_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        let old_version = Self::get_version(env.clone());
        let new_version = old_version.checked_add(1).ok_or(ContractError::Overflow)?;
        env.storage()
            .persistent()
            .set(&constants::storage::CONTRACT_VERSION, &new_version);
        extend_key_ttl_to_full_window(&env, &constants::storage::CONTRACT_VERSION);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        env.events().publish(
            events::upgrade_executed_topics(&admin),
            events::UpgradeExecutedEvent {
                old_version,
                new_version,
            },
        );
        Ok(())
    }

    /// Read-only view: returns the current contract upgrade version (starts at 1).
    pub fn get_version(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&constants::storage::CONTRACT_VERSION)
            .unwrap_or(1)
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

    /// Voluntarily removes `quantity` keys from the caller's transferable balance.
    pub fn self_freeze(
        env: Env,
        key_id: Address,
        wallet: Address,
        quantity: u32,
    ) -> Result<(), ContractError> {
        wallet.require_auth();
        assert_not_paused(&env)?;
        if quantity == 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        let available = available_holder_balance(&env, &key_id, &wallet);
        if available < quantity {
            return Err(ContractError::FreezeQuantityExceedsBalance);
        }
        let key = constants::storage::self_frozen_balance(&key_id, &wallet);
        let frozen = read_self_frozen_balance(&env, &key_id, &wallet);
        env.storage().persistent().set(
            &key,
            &frozen
                .checked_add(quantity)
                .ok_or(ContractError::Overflow)?,
        );
        extend_key_ttl_to_full_window(&env, &key);
        env.events().publish(
            (
                events::SELF_FREEZE_APPLIED_EVENT_NAME,
                key_id.clone(),
                wallet.clone(),
            ),
            events::SelfFreezeEvent {
                key_id,
                wallet,
                quantity,
            },
        );
        Ok(())
    }

    /// Releases previously self-frozen keys for the caller.
    pub fn self_unfreeze(
        env: Env,
        key_id: Address,
        wallet: Address,
        quantity: u32,
    ) -> Result<(), ContractError> {
        wallet.require_auth();
        if quantity == 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        let key = constants::storage::self_frozen_balance(&key_id, &wallet);
        let frozen = read_self_frozen_balance(&env, &key_id, &wallet);
        if frozen < quantity {
            return Err(ContractError::InsufficientBalance);
        }
        let remaining = frozen - quantity;
        if remaining == 0 {
            env.storage().persistent().remove(&key);
        } else {
            env.storage().persistent().set(&key, &remaining);
            extend_key_ttl_to_full_window(&env, &key);
        }
        env.events().publish(
            (
                events::SELF_FREEZE_LIFTED_EVENT_NAME,
                key_id.clone(),
                wallet.clone(),
            ),
            events::SelfFreezeEvent {
                key_id,
                wallet,
                quantity,
            },
        );
        Ok(())
    }

    pub fn get_self_frozen_balance(env: Env, key_id: Address, wallet: Address) -> u32 {
        read_self_frozen_balance(&env, &key_id, &wallet)
    }

    /// Freezes the caller's whole position in `key_id`: `buy_key`, `sell_key`
    /// and `transfer_keys` (and their batch variants) are rejected with
    /// [`ContractError::FrozenPosition`] until [`Self::unfreeze_position`].
    ///
    /// Only the position owner (`wallet`) may freeze their own position. This
    /// boolean lock is independent of the quantity-based [`Self::self_freeze`].
    ///
    /// # Errors
    /// - [`ContractError::NotRegistered`] if `key_id` has no creator profile.
    /// - [`ContractError::InsufficientBalance`] if `wallet` holds no keys.
    pub fn freeze_position(
        env: Env,
        key_id: Address,
        wallet: Address,
    ) -> Result<(), ContractError> {
        wallet.require_auth();
        assert_not_paused(&env)?;
        read_registered_creator_profile(&env, &key_id)?;
        if Self::get_key_balance(env.clone(), key_id.clone(), wallet.clone()) == 0 {
            return Err(ContractError::InsufficientBalance);
        }
        let key = constants::storage::position_frozen(&key_id, &wallet);
        env.storage().persistent().set(&key, &true);
        extend_key_ttl_to_full_window(&env, &key);
        Ok(())
    }

    /// Clears the freeze flag set by [`Self::freeze_position`]. Only the
    /// position owner may call this; it is a no-op when not frozen.
    pub fn unfreeze_position(
        env: Env,
        key_id: Address,
        wallet: Address,
    ) -> Result<(), ContractError> {
        wallet.require_auth();
        env.storage()
            .persistent()
            .remove(&constants::storage::position_frozen(&key_id, &wallet));
        Ok(())
    }

    /// Read-only view: whether `wallet`'s position in `key_id` is frozen.
    pub fn get_position_frozen(env: Env, key_id: Address, wallet: Address) -> bool {
        is_position_frozen(&env, &key_id, &wallet)
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

    /// Records a snapshot of holder balances at the current ledger for offline
    /// processing (dividend distributions, governance) (issue #778).
    ///
    /// Soroban contract storage cannot be enumerated on-chain, so `holders` is
    /// supplied by the caller (e.g. sourced from an indexer) rather than read
    /// from an on-chain registry — see [`MAX_SNAPSHOT_HOLDERS`] for why this
    /// deviates from the issue's literal "iterate the holder registry"
    /// wording, and how a holder set larger than the cap should be handled.
    ///
    /// Only callable by the protocol admin.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    /// - [`ContractError::NotRegistered`] if `creator` has no profile.
    /// - [`ContractError::SnapshotHolderLimitExceeded`] if `holders` exceeds
    ///   [`MAX_SNAPSHOT_HOLDERS`] entries.
    /// - [`ContractError::SnapshotAlreadyExists`] if `snapshot_id` was already
    ///   used for this creator.
    pub fn take_snapshot(
        env: Env,
        admin: Address,
        creator: Address,
        snapshot_id: u32,
        holders: Vec<Address>,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        read_registered_creator_profile(&env, &creator)?;

        if holders.len() > MAX_SNAPSHOT_HOLDERS {
            return Err(ContractError::SnapshotHolderLimitExceeded);
        }

        let meta_key = constants::storage::snapshot_meta(&creator, snapshot_id);
        if env.storage().persistent().has(&meta_key) {
            return Err(ContractError::SnapshotAlreadyExists);
        }

        let snapshot_ledger = env.ledger().sequence();
        let mut total_holders: u32 = 0;
        for holder in holders.iter() {
            let balance_key = constants::storage::holder_balance_key(&creator, &holder);
            let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
            let staked_key = constants::storage::staked_balance(&creator, &holder);
            let staked_balance: u32 = env.storage().persistent().get(&staked_key).unwrap_or(0);

            let snap_key = constants::storage::snapshot_balance(&creator, snapshot_id, &holder);
            env.storage().persistent().set(&snap_key, &balance);
            extend_key_ttl_to_full_window(&env, &snap_key);

            let snap_staked_key =
                constants::storage::snapshot_staked_balance(&creator, snapshot_id, &holder);
            env.storage()
                .persistent()
                .set(&snap_staked_key, &staked_balance);
            extend_key_ttl_to_full_window(&env, &snap_staked_key);

            total_holders = total_holders
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
        }

        let meta = HolderSnapshotMeta {
            snapshot_ledger,
            total_holders,
        };
        env.storage().persistent().set(&meta_key, &meta);
        extend_key_ttl_to_full_window(&env, &meta_key);

        let holders_key = constants::storage::snapshot_holders(&creator, snapshot_id);
        env.storage().persistent().set(&holders_key, &holders);
        extend_key_ttl_to_full_window(&env, &holders_key);

        env.events().publish(
            events::snapshot_taken_topics(&creator, snapshot_id),
            events::SnapshotTakenEvent {
                creator_id: creator,
                snapshot_id,
                snapshot_ledger,
                total_holders,
            },
        );

        Ok(())
    }

    /// Distributes the treasury balance pro-rata to the staked balances captured
    /// by a holder snapshot. Payouts are added to each holder's claimable
    /// dividend balance; floor-division dust remains in the treasury.
    pub fn distribute_protocol_revenue(
        env: Env,
        admin: Address,
        creator: Address,
        snapshot_id: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        let meta_key = constants::storage::snapshot_meta(&creator, snapshot_id);
        let _meta: HolderSnapshotMeta = env
            .storage()
            .persistent()
            .get(&meta_key)
            .ok_or(ContractError::SnapshotNotFound)?;
        let holders_key = constants::storage::snapshot_holders(&creator, snapshot_id);
        let holders: soroban_sdk::Vec<Address> = env
            .storage()
            .persistent()
            .get(&holders_key)
            .ok_or(ContractError::SnapshotNotFound)?;
        let current_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        let treasury_balance = read_treasury_balance(&env);

        let mut total_weight: i128 = 0;
        let mut weights = soroban_sdk::Vec::new(&env);
        let mut staker_count: u32 = 0;
        for holder in holders.iter() {
            let staked_key =
                constants::storage::snapshot_staked_balance(&creator, snapshot_id, &holder);
            let staked_quantity: u32 = env.storage().persistent().get(&staked_key).unwrap_or(0);
            let weight = i128::from(staked_quantity)
                .checked_mul(current_price)
                .ok_or(ContractError::Overflow)?;
            if weight > 0 {
                total_weight = total_weight
                    .checked_add(weight)
                    .ok_or(ContractError::Overflow)?;
                staker_count = staker_count.checked_add(1).ok_or(ContractError::Overflow)?;
            }
            weights.push_back((holder, weight));
        }

        let mut total_distributed: i128 = 0;
        if total_weight > 0 && treasury_balance > 0 {
            for (holder, weight) in weights.iter() {
                if weight == 0 {
                    continue;
                }
                let payout = treasury_balance
                    .checked_mul(weight)
                    .ok_or(ContractError::Overflow)?
                    / total_weight;
                if payout == 0 {
                    continue;
                }
                let pending_key = constants::storage::holder_dividend_pending(&creator, &holder);
                let pending: i128 = env.storage().persistent().get(&pending_key).unwrap_or(0);
                let updated_pending = pending.checked_add(payout).ok_or(ContractError::Overflow)?;
                env.storage()
                    .persistent()
                    .set(&pending_key, &updated_pending);
                extend_key_ttl_to_full_window(&env, &pending_key);
                total_distributed = total_distributed
                    .checked_add(payout)
                    .ok_or(ContractError::Overflow)?;
            }
        }

        let remaining = treasury_balance
            .checked_sub(total_distributed)
            .ok_or(ContractError::Overflow)?;
        env.storage()
            .persistent()
            .set(&constants::storage::TREASURY_BALANCE, &remaining);
        extend_key_ttl_to_full_window(&env, &constants::storage::TREASURY_BALANCE);

        env.events().publish(
            events::protocol_revenue_distributed_topics(&creator, snapshot_id),
            events::ProtocolRevenueDistributedEvent {
                total_distributed,
                staker_count,
                snapshot_id,
            },
        );

        Ok(())
    }

    /// Adds `amount` of trading fees to `creator`'s revenue distribution pool.
    pub fn accumulate_fees(
        env: Env,
        admin: Address,
        creator: Address,
        amount: i128,
    ) -> Result<i128, ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        if amount <= 0 {
            return Err(ContractError::ZeroDistributionAmount);
        }
        let key = RevenueKey::Pool(creator);
        let pool: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let updated = pool.checked_add(amount).ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&key, &updated);
        extend_key_ttl_to_full_window(&env, &key);
        Ok(updated)
    }

    /// Sets the minimum number of ledgers between revenue distribution cycles.
    pub fn set_distribution_cycle_length(
        env: Env,
        admin: Address,
        length_ledgers: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        if length_ledgers == 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        let key = RevenueKey::CycleLength;
        env.storage().persistent().set(&key, &length_ledgers);
        extend_key_ttl_to_full_window(&env, &key);
        Ok(())
    }

    /// Read-only view: undistributed revenue pool balance for `creator`.
    pub fn get_revenue_pool(env: Env, creator: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&RevenueKey::Pool(creator))
            .unwrap_or(0)
    }

    /// Snapshots `creator`'s revenue pool as a new cycle and allocates it to
    /// `holders` in proportion to their current staked balance. Floor-division
    /// dust stays in the pool for the next cycle. Returns the new cycle id.
    pub fn distribute_cycle(
        env: Env,
        admin: Address,
        creator: Address,
        holders: soroban_sdk::Vec<Address>,
    ) -> Result<u32, ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        let pool_key = RevenueKey::Pool(creator.clone());
        let pool: i128 = env.storage().persistent().get(&pool_key).unwrap_or(0);
        if pool <= 0 {
            return Err(ContractError::ZeroDistributionAmount);
        }

        let last_key = RevenueKey::LastDistribution(creator.clone());
        let length: u32 = env
            .storage()
            .persistent()
            .get(&RevenueKey::CycleLength)
            .unwrap_or(0);
        if let Some(last) = env.storage().persistent().get::<_, u32>(&last_key) {
            let next_allowed = last.checked_add(length).ok_or(ContractError::Overflow)?;
            if env.ledger().sequence() < next_allowed {
                return Err(ContractError::CooldownActive);
            }
        }

        let mut unique = soroban_sdk::Vec::new(&env);
        let mut total_weight: i128 = 0;
        for holder in holders.iter() {
            if unique.contains(&holder) {
                continue;
            }
            let staked: u32 = env
                .storage()
                .persistent()
                .get(&constants::storage::staked_balance(&creator, &holder))
                .unwrap_or(0);
            total_weight = total_weight
                .checked_add(i128::from(staked))
                .ok_or(ContractError::Overflow)?;
            unique.push_back(holder);
        }
        if total_weight == 0 {
            return Err(ContractError::NoKeyHolders);
        }

        let count_key = RevenueKey::CycleCount(creator.clone());
        let cycle: u32 = env
            .storage()
            .persistent()
            .get::<_, u32>(&count_key)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;

        let mut allocated: i128 = 0;
        for holder in unique.iter() {
            let staked: u32 = env
                .storage()
                .persistent()
                .get(&constants::storage::staked_balance(&creator, &holder))
                .unwrap_or(0);
            let share = pool
                .checked_mul(i128::from(staked))
                .ok_or(ContractError::Overflow)?
                / total_weight;
            if share == 0 {
                continue;
            }
            let share_key = RevenueKey::CycleShare(creator.clone(), cycle, holder);
            env.storage().persistent().set(&share_key, &share);
            extend_key_ttl_to_full_window(&env, &share_key);
            allocated = allocated
                .checked_add(share)
                .ok_or(ContractError::Overflow)?;
        }

        let cycle_pool_key = RevenueKey::CyclePool(creator.clone(), cycle);
        env.storage().persistent().set(&cycle_pool_key, &pool);
        extend_key_ttl_to_full_window(&env, &cycle_pool_key);
        env.storage()
            .persistent()
            .set(&pool_key, &(pool - allocated));
        extend_key_ttl_to_full_window(&env, &pool_key);
        env.storage().persistent().set(&count_key, &cycle);
        extend_key_ttl_to_full_window(&env, &count_key);
        env.storage()
            .persistent()
            .set(&last_key, &env.ledger().sequence());
        extend_key_ttl_to_full_window(&env, &last_key);

        Ok(cycle)
    }

    /// Claims `holder`'s allocated share for a distribution `cycle` and returns
    /// the amount. Errors with `AlreadyClaimed` on a repeat claim and
    /// `NoDividendClaimable` when the holder has no share in that cycle.
    pub fn claim_cycle(
        env: Env,
        creator: Address,
        holder: Address,
        cycle: u32,
    ) -> Result<i128, ContractError> {
        holder.require_auth();
        assert_not_paused(&env)?;

        let claimed_key = RevenueKey::CycleClaimed(creator.clone(), cycle, holder.clone());
        if env.storage().persistent().has(&claimed_key) {
            return Err(ContractError::AlreadyClaimed);
        }
        let share: i128 = env
            .storage()
            .persistent()
            .get(&RevenueKey::CycleShare(creator, cycle, holder))
            .unwrap_or(0);
        if share == 0 {
            return Err(ContractError::NoDividendClaimable);
        }
        env.storage().persistent().set(&claimed_key, &true);
        extend_key_ttl_to_full_window(&env, &claimed_key);
        Ok(share)
    }

    /// Read-only view: returns a holder's snapshotted balance, or `0` if the
    /// holder was not included in the snapshot's `holders` list.
    pub fn get_snapshot_balance(
        env: Env,
        creator: Address,
        snapshot_id: u32,
        holder: Address,
    ) -> u32 {
        let key = constants::storage::snapshot_balance(&creator, snapshot_id, &holder);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Read-only view: returns a snapshot's metadata, or `None` if `snapshot_id`
    /// has not been taken for `creator`.
    pub fn get_snapshot_meta(
        env: Env,
        creator: Address,
        snapshot_id: u32,
    ) -> Option<HolderSnapshotMeta> {
        let key = constants::storage::snapshot_meta(&creator, snapshot_id);
        env.storage().persistent().get(&key)
    }

    /// Read-only view: returns the optional immutable co-creator config.
    ///
    /// Returns `None` when the creator was registered without a co-creator split.
    pub fn get_co_creator(env: Env, creator: Address) -> Option<CoCreatorConfig> {
        read_co_creator_config(&env, &creator)
    }

    /// Designates (or updates) the creator's co-creator revenue split (issue #782).
    ///
    /// Unlike the immutable split optionally set at [`Self::register_creator`], this
    /// entrypoint lets a registered creator set or change their co-creator split at any
    /// time. Once set, every subsequent buy and sell routes `share_bps` of the creator's
    /// fee to `co_creator` via the same [`credit_creator_fee`] path the registration-time
    /// config already uses — this call only writes the config; splitting on trades is
    /// unconditional on it existing, no extra plumbing needed.
    ///
    /// # Errors
    /// - [`ContractError::NotRegistered`] if `creator` has no profile.
    /// - [`ContractError::ZeroAddress`] if `co_creator` is the zero address.
    /// - [`ContractError::SplitTooHigh`] if `split_bps` is `0` or exceeds `9000` (90%).
    pub fn set_co_creator(
        env: Env,
        creator: Address,
        co_creator: Address,
        split_bps: u32,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        read_registered_creator_profile(&env, &creator)?;
        validate_non_zero_address(&env, &co_creator)?;
        if split_bps == 0 || split_bps > 9000 {
            return Err(ContractError::SplitTooHigh);
        }

        let config = CoCreatorConfig {
            address: co_creator.clone(),
            share_bps: split_bps,
        };
        let key = constants::storage::co_creator(&creator);
        env.storage().persistent().set(&key, &config);
        extend_key_ttl_to_full_window(&env, &key);

        env.events().publish(
            events::co_creator_set_topics(&creator, &co_creator),
            events::CoCreatorSetEvent {
                creator_id: creator,
                co_creator,
                split_bps,
            },
        );

        Ok(())
    }

    /// Removes the co-creator split for a registered creator (issue #791).
    ///
    /// After removal the full creator fee goes to `creator` on future trades.
    /// The co-creator's existing fee balance is preserved (they can still
    /// withdraw it).
    ///
    /// # Errors
    /// - [`FeatureError::Unauthorized`] if `caller` is not `creator`.
    /// - [`FeatureError::NoCoCreatorSet`] if no co-creator is configured.
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
        let config: Option<CoCreatorConfig> = env.storage().persistent().get(&key);
        let Some(config) = config else {
            return Err(FeatureError::NoCoCreatorSet);
        };
        let removed_address = config.address.clone();
        env.storage().persistent().remove(&key);
        env.events().publish(
            events::co_creator_removed_topics(&creator, &removed_address),
            events::CoCreatorRemovedEvent {
                creator_id: creator,
                co_creator: removed_address,
            },
        );
        Ok(())
    }

    /// Configures a pre-launch fixed-price auction for a creator's key (issue #787).
    ///
    /// Must be called before any key has been sold (supply == 0).
    /// During the auction phase `buy_key` will settle at `auction_price` instead of
    /// the bonding-curve price until `auction_supply` keys have been sold.
    ///
    /// # Errors
    /// - [`FeatureError::Unauthorized`] if `caller != creator`.
    /// - [`FeatureError::NotRegistered`] if the creator has no profile.
    /// - [`FeatureError::AuctionAlreadyStarted`] if supply > 0.
    /// - [`FeatureError::NotPositiveAmount`] if `auction_price <= 0`.
    /// - [`FeatureError::InvalidAuctionConfig`] if `auction_supply` is 0 or > 10 000.
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
        let profile: CreatorProfile = env
            .storage()
            .persistent()
            .get(&constants::storage::creator(&creator))
            .ok_or(FeatureError::NotRegistered)?;
        if profile.supply > 0 {
            return Err(FeatureError::AuctionAlreadyStarted);
        }
        if auction_price <= 0 {
            return Err(FeatureError::NotPositiveAmount);
        }
        if auction_supply == 0 || auction_supply > 10_000 {
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
                ledger: env.ledger().sequence(),
            },
        );
        Ok(())
    }

    /// Cancels a pre-launch auction before any key has been purchased (issue #790).
    ///
    /// # Errors
    /// - [`FeatureError::Unauthorized`] if `caller != creator`.
    /// - [`FeatureError::NoAuctionConfigured`] if no auction exists.
    /// - [`FeatureError::AuctionAlreadyStarted`] if at least one key has been sold.
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
            },
        );
        Ok(())
    }

    /// Read-only view: returns the current auction configuration for a creator, if any.
    pub fn get_auction_config(env: Env, creator: Address) -> Option<AuctionConfig> {
        env.storage()
            .persistent()
            .get(&constants::storage::auction_config(&creator))
    }

    /// Stores on-chain identity metadata (name, bio, avatar URI) for a
    /// registered creator's key (issue #779).
    ///
    /// Callable only by the creator themselves, once. To change metadata
    /// afterwards, see the immutability note on [`ContractError::KeyAlreadyInitialised`] —
    /// this contract has no `update_key_metadata` entrypoint; adding one is a
    /// natural follow-up but out of scope for this issue.
    ///
    /// # Errors
    /// - [`ContractError::NotRegistered`] if `creator` has no profile.
    /// - [`ContractError::DisplayNameEmpty`] if `name` or `bio` is empty.
    /// - [`ContractError::NameTooLong`] if `name` exceeds 64 bytes.
    /// - [`ContractError::BioTooLong`] if `bio` exceeds 256 bytes.
    /// - [`ContractError::KeyAlreadyInitialised`] if metadata already exists
    ///   for `creator`.
    pub fn initialise_key(
        env: Env,
        creator: Address,
        metadata: KeyMetadata,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        read_registered_creator_profile(&env, &creator)?;

        validate_key_metadata(&metadata)?;

        if read_creator_metadata(&env, &creator).is_some() {
            return Err(ContractError::KeyAlreadyInitialised);
        }

        write_creator_metadata(&env, &creator, &metadata);

        env.events().publish(
            events::key_initialised_topics(&creator),
            events::KeyInitialisedEvent {
                creator_id: creator,
                name: metadata.name,
                bio: metadata.bio,
                avatar_uri: metadata.avatar_uri,
            },
        );

        Ok(())
    }

    /// Read-only view: returns a creator's on-chain key metadata, or `None`
    /// if `initialise_key` has not been called for them.
    pub fn get_key_metadata(env: Env, creator: Address) -> Option<KeyMetadata> {
        read_creator_metadata(&env, &creator)
    }

    /// Registers a creator key on their behalf with its full initial config:
    /// zero supply, curve preset, buy cooldown, metadata and an optional
    /// `auction_pending` flag. Emits [`events::KeyRegisteredEvent`].
    ///
    /// The issue text names an authorised factory contract, but this
    /// repository has none, so the caller is gated on the protocol admin.
    /// `auction_mode` only records the `auction_pending` flag; the auction
    /// price and supply are still set by the creator via `configure_auction`.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    /// - [`ContractError::AlreadyRegistered`] if `creator` already has a profile.
    /// - [`ContractError::InvalidCooldown`] if `cooldown_ledgers` exceeds
    ///   [`MAX_BUY_COOLDOWN_LEDGERS`].
    /// - Handle and metadata validation errors as in `register_creator` and
    ///   `initialise_key`.
    #[allow(clippy::too_many_arguments)]
    pub fn register_key(
        env: Env,
        admin: Address,
        creator: Address,
        handle: String,
        metadata: KeyMetadata,
        curve_preset: CurvePreset,
        cooldown_ledgers: u32,
        auction_mode: bool,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        assert_not_paused(&env)?;
        assert_not_blacklisted(&env, &creator)?;
        validate_creator_handle(&handle)?;
        validate_key_metadata(&metadata)?;
        if cooldown_ledgers > MAX_BUY_COOLDOWN_LEDGERS {
            return Err(ContractError::InvalidCooldown);
        }

        let key = constants::storage::creator(&creator);
        if env.storage().persistent().has(&key) {
            return Err(ContractError::AlreadyRegistered);
        }

        let current_ledger = env.ledger().sequence();
        let profile = CreatorProfile {
            creator: creator.clone(),
            handle,
            supply: 0,
            holder_count: 0,
            fee_recipient: creator.clone(),
            registered_at: current_ledger,
        };
        env.storage().persistent().set(&key, &profile);
        extend_key_ttl_to_full_window(&env, &key);

        let preset_key = constants::storage::curve_preset(&creator);
        env.storage().persistent().set(&preset_key, &curve_preset);
        extend_key_ttl_to_full_window(&env, &preset_key);

        let cooldown_key = constants::storage::buy_cooldown(&creator);
        env.storage()
            .persistent()
            .set(&cooldown_key, &cooldown_ledgers);
        extend_key_ttl_to_full_window(&env, &cooldown_key);

        let auction_key = constants::storage::auction_pending(&creator);
        env.storage().persistent().set(&auction_key, &auction_mode);
        extend_key_ttl_to_full_window(&env, &auction_key);

        write_creator_metadata(&env, &creator, &metadata);

        let live_until_key = constants::storage::creator_ttl_live_until(&creator);
        env.storage()
            .persistent()
            .set(&live_until_key, &(current_ledger + CREATOR_TTL_LEDGERS));
        extend_key_ttl_to_full_window(&env, &live_until_key);

        env.events().publish(
            events::key_registered_topics(&creator),
            events::KeyRegisteredEvent {
                key_id: creator.clone(),
                creator: creator.clone(),
                auction_pending: auction_mode,
                registered_at_ledger: current_ledger,
            },
        );

        // A successful key launch is a positive reputation signal.
        apply_reputation_delta(
            &env,
            &creator,
            REPUTATION_KEY_LAUNCH_POINTS,
            ReputationReason::KeyLaunch,
        )
        .map_err(|_| ContractError::Overflow)?;

        Ok(())
    }

    /// Read-only view: `true` when the key was registered via `register_key`
    /// with `auction_mode` set.
    pub fn is_auction_pending(env: Env, creator: Address) -> bool {
        env.storage()
            .persistent()
            .get(&constants::storage::auction_pending(&creator))
            .unwrap_or(false)
    }

    /// Updates a creator's key metadata. Only fields wrapped in `Some` are
    /// changed; `None` fields remain untouched. Emits `MetadataUpdated`.
    pub fn update_metadata(
        env: Env,
        creator: Address,
        name: Option<String>,
        bio: Option<String>,
        avatar_uri: Option<String>,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        let mut metadata =
            read_creator_metadata(&env, &creator).ok_or(ContractError::NotRegistered)?;

        let mut changed = false;
        let mut updated_name = String::from_str(&env, "");
        let mut updated_bio = String::from_str(&env, "");
        let mut updated_avatar = String::from_str(&env, "");

        if let Some(n) = name {
            if n.len() > METADATA_NAME_MAX_LEN {
                return Err(ContractError::HandleTooLong);
            }
            updated_name = n.clone();
            metadata.name = n;
            changed = true;
        }
        if let Some(b) = bio {
            if b.len() > METADATA_BIO_MAX_LEN {
                return Err(ContractError::HandleTooLong);
            }
            updated_bio = b.clone();
            metadata.bio = b;
            changed = true;
        }
        if let Some(u) = avatar_uri {
            if u.len() > METADATA_AVATAR_URI_MAX_LEN {
                return Err(ContractError::HandleTooLong);
            }
            updated_avatar = u.clone();
            metadata.avatar_uri = u;
            changed = true;
        }

        if !changed {
            return Ok(());
        }

        write_creator_metadata(&env, &creator, &metadata);

        env.events().publish(
            events::metadata_updated_topics(&creator),
            events::MetadataUpdatedEvent {
                creator_id: creator.clone(),
                name: updated_name,
                bio: updated_bio,
                avatar_uri: updated_avatar,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    // =========================================================================
    // Feature: update_config — admin-only bonding curve / fee config update
    // =========================================================================

    /// Updates protocol configuration parameters in a single admin call.
    ///
    /// Validates all values before applying any change so the update is atomic.
    /// Emits a [`events::ConfigUpdatedEvent`] on success.
    ///
    /// # Validation rules
    ///
    /// - `params.creator_bps + params.protocol_bps` must be `> 0` and `<= 10000`.
    /// - `params.protocol_bps` must be `<= fee::PROTOCOL_BPS_MAX` (10000).
    /// - `params.curve_slope` must be `>= 0`.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] — caller is not the protocol admin.
    /// - [`ContractError::InvalidFeeConfig`] — fee bps are out of range.
    /// - [`ContractError::ProtocolFeeExceedsCap`] — `protocol_bps > PROTOCOL_BPS_MAX`.
    /// - [`ContractError::NotPositiveAmount`] — `curve_slope < 0`.
    pub fn update_config(
        env: Env,
        admin: Address,
        params: ConfigUpdateParams,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        // Validate fee bps using the shared helper.
        fee::assert_valid_fee_bps(params.creator_bps, params.protocol_bps)?;

        if params.curve_slope < 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        // Apply: update fee config.
        let new_fee_config = fee::FeeConfig {
            creator_bps: params.creator_bps,
            protocol_bps: params.protocol_bps,
        };
        env.storage()
            .persistent()
            .set(&constants::storage::FEE_CONFIG, &new_fee_config);
        extend_key_ttl_to_full_window(&env, &constants::storage::FEE_CONFIG);

        // Apply: update curve slope.
        env.storage()
            .persistent()
            .set(&constants::storage::CURVE_SLOPE, &params.curve_slope);
        extend_key_ttl_to_full_window(&env, &constants::storage::CURVE_SLOPE);

        // Increment protocol state version on config update.
        let current_version: u32 = env
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

        env.events().publish(
            events::config_updated_topics(&admin),
            events::ConfigUpdatedEvent {
                admin: admin.clone(),
                creator_bps: params.creator_bps,
                protocol_bps: params.protocol_bps,
                curve_slope: params.curve_slope,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    // =========================================================================
    // Feature: holder_count — get_holder_count() view alias
    // =========================================================================

    // =========================================================================
    // Feature: snapshot governance auth + pruning
    // =========================================================================

    /// Sets the governance contract address that is authorised to call
    /// [`Self::take_snapshot_governance`].
    ///
    /// Only callable by the protocol admin.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    pub fn set_governance_address(
        env: Env,
        admin: Address,
        governance: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        env.storage()
            .persistent()
            .set(&constants::storage::GOVERNANCE_ADDRESS, &governance);
        extend_key_ttl_to_full_window(&env, &constants::storage::GOVERNANCE_ADDRESS);

        env.events()
            .publish(events::governance_address_set_topics(&admin), governance);

        Ok(())
    }

    /// Read-only view: returns the configured governance contract address, if any.
    pub fn get_governance_address(env: Env) -> Option<Address> {
        read_governance_address(&env)
    }

    /// Sets the retention window (in ledgers) after which snapshots are pruned.
    ///
    /// A value of `0` disables age-based pruning. Only callable by the protocol
    /// admin.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    pub fn set_snapshot_retention(
        env: Env,
        admin: Address,
        retention_ledgers: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        env.storage().persistent().set(
            &constants::storage::SNAPSHOT_RETENTION_LEDGERS,
            &retention_ledgers,
        );
        extend_key_ttl_to_full_window(&env, &constants::storage::SNAPSHOT_RETENTION_LEDGERS);
        Ok(())
    }

    /// Read-only view: returns the configured snapshot retention window in ledgers.
    pub fn get_snapshot_retention(env: Env) -> u32 {
        read_snapshot_retention_ledgers(&env)
    }

    /// Records a snapshot of holder balances, callable **only** by the registered
    /// governance contract (set via [`Self::set_governance_address`]).
    ///
    /// This is the governance-gated variant of the existing admin-only
    /// [`Self::take_snapshot`].  It also triggers age-based pruning of old
    /// snapshots for the same creator after storing the new one.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] — caller is not the registered governance contract.
    /// - [`ContractError::NotRegistered`] — `creator` has no profile.
    /// - [`ContractError::SnapshotHolderLimitExceeded`] — `holders` exceeds `MAX_SNAPSHOT_HOLDERS`.
    /// - [`ContractError::SnapshotAlreadyExists`] — `snapshot_id` already used for `creator`.
    pub fn take_snapshot_governance(
        env: Env,
        caller: Address,
        creator: Address,
        snapshot_id: u32,
        holders: soroban_sdk::Vec<Address>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        assert_is_governance(&env, &caller)?;
        read_registered_creator_profile(&env, &creator)?;

        if holders.len() > MAX_SNAPSHOT_HOLDERS {
            return Err(ContractError::SnapshotHolderLimitExceeded);
        }

        let meta_key = constants::storage::snapshot_meta(&creator, snapshot_id);
        if env.storage().persistent().has(&meta_key) {
            return Err(ContractError::SnapshotAlreadyExists);
        }

        let snapshot_ledger = env.ledger().sequence();
        let mut total_holders: u32 = 0;

        for holder in holders.iter() {
            let balance_key = constants::storage::holder_balance_key(&creator, &holder);
            let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
            let staked_key = constants::storage::staked_balance(&creator, &holder);
            let staked_balance: u32 = env.storage().persistent().get(&staked_key).unwrap_or(0);

            let snap_key = constants::storage::snapshot_balance(&creator, snapshot_id, &holder);
            env.storage().persistent().set(&snap_key, &balance);
            extend_key_ttl_to_full_window(&env, &snap_key);

            let snap_staked_key =
                constants::storage::snapshot_staked_balance(&creator, snapshot_id, &holder);
            env.storage()
                .persistent()
                .set(&snap_staked_key, &staked_balance);
            extend_key_ttl_to_full_window(&env, &snap_staked_key);

            total_holders = total_holders
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
        }

        let meta = HolderSnapshotMeta {
            snapshot_ledger,
            total_holders,
        };
        env.storage().persistent().set(&meta_key, &meta);
        extend_key_ttl_to_full_window(&env, &meta_key);

        let holders_key = constants::storage::snapshot_holders(&creator, snapshot_id);
        env.storage().persistent().set(&holders_key, &holders);
        extend_key_ttl_to_full_window(&env, &holders_key);

        // Track oldest snapshot id for pruning.
        let oldest_key = constants::storage::oldest_snapshot_id(&creator);
        if !env.storage().persistent().has(&oldest_key) {
            env.storage().persistent().set(&oldest_key, &snapshot_id);
            extend_key_ttl_to_full_window(&env, &oldest_key);
        }

        // Prune snapshots that have aged out.
        prune_old_snapshots(&env, &creator, snapshot_id);

        env.events().publish(
            events::snapshot_taken_topics(&creator, snapshot_id),
            events::SnapshotTakenEvent {
                creator_id: creator,
                snapshot_id,
                snapshot_ledger,
                total_holders,
            },
        );

        Ok(())
    }

    // =========================================================================
    // Feature: enhanced batch_buy with per-key max_price slippage + FeeCollected
    // =========================================================================

    /// Executes multiple key purchases across different creators in a single
    /// transaction, with per-key slippage protection and `FeeCollected` events.
    ///
    /// Each order is a `(creator_address, quantity, max_price_per_order)` tuple
    /// where `max_price_per_order` is the caller's slippage ceiling for the
    /// **total** cost of that order (`quantity` keys). Pass `None` to skip the
    /// slippage check for a given order.
    ///
    /// The batch size is capped at [`MAX_BATCH_BUY_SIZE`] orders; exceeding it
    /// returns [`ContractError::BatchClaimExceedsLimit`].
    ///
    /// All orders execute or none do (Soroban transaction atomicity). A
    /// [`events::FeeCollectedEvent`] is emitted for every order that incurs a
    /// protocol trade fee, and a summary [`events::BatchBuyCompletedEvent`] is
    /// emitted at the end.
    ///
    /// # Errors
    ///
    /// - [`ContractError::BatchClaimExceedsLimit`] — batch is empty or too large.
    /// - [`ContractError::NotPositiveAmount`] — an order has `quantity == 0`.
    /// - [`ContractError::SlippageExceeded`] — total order cost > `max_price`.
    /// - Any error that `buy_key` would return for an individual order.
    pub fn batch_buy_v2(
        env: Env,
        buyer: Address,
        orders: soroban_sdk::Vec<(Address, u32, Option<i128>)>,
    ) -> Result<soroban_sdk::Vec<BatchBuyOrderResult>, ContractError> {
        buyer.require_auth();
        assert_not_paused(&env)?;
        assert_not_blacklisted(&env, &buyer)?;
        assert_before_global_deadline(&env)?;
        assert_global_trading_not_halted(&env)?;

        if orders.is_empty() || orders.len() > MAX_BATCH_BUY_SIZE as u32 {
            return Err(ContractError::BatchClaimExceedsLimit);
        }

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        bump_persistent_ttl(&env, &constants::storage::KEY_PRICE);

        let mut results = soroban_sdk::Vec::new(&env);
        let mut total_price_paid: i128 = 0;

        for order in orders.iter() {
            let (creator, quantity, max_price) = order;

            if quantity == 0 {
                return Err(ContractError::NotPositiveAmount);
            }

            // Reject deprecated keys.
            if env
                .storage()
                .persistent()
                .has(&constants::storage::deprecated_key(&creator))
            {
                return Err(ContractError::KeyDeprecated);
            }

            assert_position_not_frozen(&env, &buyer, &creator)?;
            let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;
            assert_whitelist_allows_buy(&env, &profile, &buyer)?;

            let mut order_price: i128 = 0;

            let mut i = 0u32;
            while i < quantity {
                let price =
                    compute_bonding_curve_price(&env, &creator, base_price, profile.supply)?;

                let balance_key = constants::storage::holder_balance_key(&creator, &buyer);
                let current_balance: u32 =
                    env.storage().persistent().get(&balance_key).unwrap_or(0);

                // Settle dividends before balance changes.
                settle_holder_dividends(&env, &creator, &buyer, current_balance)?;

                let old_holder_count = profile.holder_count;
                if current_balance == 0 {
                    profile.holder_count = profile
                        .holder_count
                        .checked_add(1)
                        .ok_or(ContractError::Overflow)?;
                }

                let creator_profile_key = constants::storage::creator(&creator);
                env.storage()
                    .persistent()
                    .set(&creator_profile_key, &profile);

                // Emit HolderCountChanged on first buy.
                if current_balance == 0 {
                    emit_holder_count_changed(
                        &env,
                        &creator,
                        old_holder_count,
                        profile.holder_count,
                    );
                }

                profile.supply = profile
                    .supply
                    .checked_add(1)
                    .ok_or(ContractError::Overflow)?;

                write_creator_supply(&env, &creator, profile.supply);

                let new_balance = current_balance
                    .checked_add(1)
                    .ok_or(ContractError::Overflow)?;
                env.storage().persistent().set(&balance_key, &new_balance);
                extend_key_ttl_to_full_window(&env, &balance_key);

                // Collect protocol trade fee and emit FeeCollected per key.
                let trade_fee = compute_trade_fee(&env, price)?;
                if trade_fee > 0 {
                    if let Some((_, treasury)) = read_trade_fee_config(&env) {
                        credit_treasury_balance(&env, trade_fee)?;
                        credit_staking_rewards_pool(&env, &creator, trade_fee)?;
                        env.events().publish(
                            events::fee_collected_topics(&treasury),
                            events::FeeCollectedEvent {
                                treasury: treasury.clone(),
                                amount: trade_fee,
                                ledger: env.ledger().sequence(),
                            },
                        );
                    }
                }

                // Collect creator / protocol fee split.
                let net_amount = price
                    .checked_sub(trade_fee)
                    .ok_or(ContractError::Overflow)?;
                if let Some(config) = read_protocol_fee_config(&env) {
                    let (creator_fee, protocol_fee) = fee::checked_compute_fee_split(
                        net_amount,
                        config.creator_bps,
                        config.protocol_bps,
                    )
                    .ok_or(ContractError::Overflow)?;
                    credit_creator_fee(&env, &creator, creator_fee)?;
                    credit_treasury_balance(&env, protocol_fee)?;
                    credit_protocol_fee_recipient_balance(&env, protocol_fee)?;
                }

                if let Some(royalty) = read_royalty_config(&env, &creator) {
                    let royalty_amount = fee::apply_percentage_fee(price, royalty.buy_fee_bps)
                        .ok_or(ContractError::Overflow)?;
                    if royalty_amount > 0 {
                        credit_creator_fee_recipient_balance(&env, &creator, royalty_amount)?;
                    }
                }

                order_price = order_price
                    .checked_add(price)
                    .ok_or(ContractError::Overflow)?;

                i += 1;
            }

            // Per-order slippage check: total cost for this order vs max_price.
            if let Some(max) = max_price {
                if order_price > max {
                    return Err(ContractError::SlippageExceeded);
                }
                env.events().publish(
                    events::slippage_check_passed_topics(&creator),
                    events::SlippageCheckPassedEvent {
                        creator_id: creator.clone(),
                        actual_amount: order_price,
                        bound: max,
                        ledger: env.ledger().sequence(),
                    },
                );
            }

            // Emit BatchBuyFeeCollected summary per order.
            let order_trade_fee = compute_trade_fee(&env, order_price).unwrap_or(0);
            env.events().publish(
                events::batch_buy_fee_collected_topics(&creator, &buyer),
                events::BatchBuyFeeCollectedEvent {
                    creator_id: creator.clone(),
                    buyer: buyer.clone(),
                    quantity,
                    total_price: order_price,
                    fee_amount: order_trade_fee,
                    ledger: env.ledger().sequence(),
                },
            );

            env.events().publish(
                events::buy_event_topics(&creator, &buyer),
                events::KeysBoughtEvent {
                    buyer: buyer.clone(),
                    creator_id: creator.clone(),
                    quantity,
                    price_paid: order_price,
                    new_supply: profile.supply,
                    ledger: env.ledger().sequence(),
                },
            );

            total_price_paid = total_price_paid
                .checked_add(order_price)
                .ok_or(ContractError::Overflow)?;

            extend_creator_ttl(&env, &creator);

            results.push_back(BatchBuyOrderResult {
                creator,
                quantity,
                price_paid: order_price,
            });
        }

        env.events().publish(
            events::batch_buy_completed_topics(&buyer),
            events::BatchBuyCompletedEvent {
                buyer: buyer.clone(),
                total_price_paid,
                order_count: results.len(),
                ledger: env.ledger().sequence(),
            },
        );

        Ok(results)
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
    pub fn get_price_at_supply(
        env: Env,
        creator: Address,
        supply: u64,
    ) -> Result<i128, ContractError> {
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
        // Apply spread after bonding curve, before fee calculation.
        let spread_adjusted = apply_spread(&env, &creator, curve_price)?;
        let Some(price) = normalize_quote_amount(spread_adjusted)? else {
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

    /// Distributes `total_amount` to the supplied holders using the claim-based
    /// model (issue #857).
    ///
    /// Unlike `distribute_dividend`, this does **not** move any balance at
    /// distribution time: each holder's pro-rata share is written to
    /// `DataKey::UnclaimedDividend` and the holder pulls it later via
    /// `claim_dividend_claimable`.
    ///
    /// Authorization: creator-only. The open-caller `distribute_dividend`
    /// is left unchanged.
    ///
    /// `holders` is caller-supplied (matching `take_snapshot`) because Soroban
    /// storage cannot be enumerated on-chain.
    pub fn distribute_dividend_claimable(
        env: Env,
        creator: Address,
        total_amount: i128,
        holders: Vec<Address>,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        assert_not_paused(&env)?;

        if total_amount <= 0 {
            return Err(ContractError::ZeroDistributionAmount);
        }

        let profile = read_registered_creator_profile(&env, &creator)?;
        if profile.supply == 0 || holders.is_empty() {
            return Err(ContractError::NoKeyHolders);
        }

        let config = read_required_protocol_fee_config(&env)?;
        let (net_amount, protocol_amount) =
            fee::compute_fee_split(total_amount, config.creator_bps, config.protocol_bps);

        credit_protocol_fee_recipient_balance(&env, protocol_amount)?;

        let per_key_net = net_amount / profile.supply as i128;

        for holder in holders.iter() {
            let balance_key = constants::storage::holder_balance_key(&creator, &holder);
            let holder_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);

            if holder_balance == 0 {
                continue;
            }

            let share = per_key_net
                .checked_mul(holder_balance as i128)
                .ok_or(ContractError::Overflow)?;

            let unclaimed_key = constants::storage::unclaimed_dividend(&creator, &holder);
            let prior: i128 = env.storage().persistent().get(&unclaimed_key).unwrap_or(0);
            let new_balance = fee::checked_accumulate(prior, share)?;
            env.storage().persistent().set(&unclaimed_key, &new_balance);
            extend_key_ttl_to_full_window(&env, &unclaimed_key);

            env.events().publish(
                events::dividend_credited_topics(&creator, &holder),
                events::DividendCreditedEvent {
                    creator: creator.clone(),
                    holder: holder.clone(),
                    amount: share,
                },
            );
        }

        env.events().publish(
            events::dividend_distributed_topics(&creator),
            events::DividendDistributedEvent {
                creator: creator.clone(),
                total_amount,
                snapshot_supply: profile.supply,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    /// Claims the caller's unclaimed claim-based dividend balance for `creator`
    /// (issue #857).
    ///
    /// Credits the caller's `HolderDividendPending` balance (the internal-ledger
    /// equivalent of a wallet payout in this contract, which has no SEP-41
    /// transfer), zeros the `UnclaimedDividend` entry, and emits
    /// `DividendClaimedEvent`.
    pub fn claim_dividend_claimable(
        env: Env,
        creator: Address,
        holder: Address,
    ) -> Result<i128, ContractError> {
        holder.require_auth();
        assert_not_paused(&env)?;

        let unclaimed_key = constants::storage::unclaimed_dividend(&creator, &holder);
        let amount: i128 = env.storage().persistent().get(&unclaimed_key).unwrap_or(0);

        if amount == 0 {
            return Err(ContractError::NoDividendClaimable);
        }

        let pending_key = constants::storage::holder_dividend_pending(&creator, &holder);
        let prior_pending: i128 = env.storage().persistent().get(&pending_key).unwrap_or(0);
        let new_pending = fee::checked_accumulate(prior_pending, amount)?;
        env.storage().persistent().set(&pending_key, &new_pending);
        extend_key_ttl_to_full_window(&env, &pending_key);

        env.storage().persistent().set(&unclaimed_key, &0i128);
        extend_key_ttl_to_full_window(&env, &unclaimed_key);

        env.events().publish(
            events::dividend_claimed_topics(&creator, &holder),
            events::DividendClaimedEvent {
                creator: creator.clone(),
                claimant: holder.clone(),
                amount,
            },
        );

        Ok(amount)
    }

    /// Read-only view of a holder's unclaimed claim-based dividend balance.
    pub fn get_unclaimed_dividend(env: Env, creator: Address, holder: Address) -> i128 {
        let key = constants::storage::unclaimed_dividend(&creator, &holder);
        env.storage().persistent().get(&key).unwrap_or(0)
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
    /// [`ContractError::WalletCapExceeded`]. The creator's own wallet is
    /// exempt from the cap.
    pub fn set_holder_cap(
        env: Env,
        creator: Address,
        cap_bps: Option<u32>,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        let resolved_bps = cap_bps.unwrap_or(DEFAULT_HOLDER_CAP_BPS);
        if !(HOLDER_CAP_MIN_BPS..=HOLDER_CAP_MAX_BPS).contains(&resolved_bps) {
            return Err(ContractError::InvalidHolderCap);
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

    /// Sets the per-wallet buy cooldown for a creator's keys.
    ///
    /// Only the key creator may call this. `cooldown_ledgers` must be in
    /// the range `0..=720` (≈ 1 hour at 5 s/ledger); values above 720 return
    /// [`CooldownError::CooldownTooLong`]. A value of `0` disables the
    /// cooldown (the default when no cooldown has been configured).
    ///
    /// Once configured, `buy_key` rejects consecutive purchases by the same
    /// wallet within the cooldown window with [`ContractError::CooldownActive`]
    /// and emits a [`events::COOLDOWN_BLOCKED_EVENT_NAME`] event.
    pub fn set_buy_cooldown(
        env: Env,
        creator: Address,
        cooldown_ledgers: u32,
    ) -> Result<(), CooldownError> {
        creator.require_auth();
        read_registered_creator_profile(&env, &creator)
            .map_err(|_| CooldownError::NotRegistered)?;
        if cooldown_ledgers > MAX_BUY_COOLDOWN_LEDGERS {
            return Err(CooldownError::CooldownTooLong);
        }
        let key = constants::storage::buy_cooldown(&creator);
        env.storage().persistent().set(&key, &cooldown_ledgers);
        extend_key_ttl_to_full_window(&env, &key);
        Ok(())
    }

    /// Read-only view: returns the configured buy cooldown in ledgers for a creator.
    ///
    /// Returns `0` (no cooldown) when none has been configured.
    pub fn get_buy_cooldown(env: Env, creator: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&constants::storage::buy_cooldown(&creator))
            .unwrap_or(0)
    }

    /// Sets the maximum number of keys a single buy transaction may purchase
    /// for this creator's keys.
    ///
    /// Only callable by the key creator. `max_qty` must be in 1..=10 000.
    /// A value of 0 disables the limit (no per-tx cap).
    pub fn set_max_buy_quantity(
        env: Env,
        creator: Address,
        max_qty: u32,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        if max_qty > MAX_BUY_QUANTITY_LIMIT {
            return Err(ContractError::LimitTooHigh);
        }
        let key = constants::storage::max_buy_quantity(&creator);
        env.storage().persistent().set(&key, &max_qty);
        extend_key_ttl_to_full_window(&env, &key);
        env.events().publish(
            events::max_buy_quantity_updated_topics(&creator),
            events::MaxBuyQuantityUpdatedEvent {
                creator_id: creator.clone(),
                max_qty,
                ledger: env.ledger().sequence(),
            },
        );
        Ok(())
    }

    /// Read-only view: returns the max buy quantity per transaction for a creator.
    ///
    /// Returns `None` while no limit is configured, meaning buys are not
    /// quantity-limited.
    pub fn get_max_buy_quantity(env: Env, creator: Address) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&constants::storage::max_buy_quantity(&creator))
    }

    /// Sets the launch penalty basis points for a creator's keys.
    ///
    /// Only callable by the key creator. `penalty_bps` must be in 0..=2000.
    /// A value of 0 disables the penalty (default behaviour).
    pub fn set_launch_penalty(env: Env, creator: Address, penalty_bps: u32) {
        creator.require_auth();
        if penalty_bps > crate::MAX_LAUNCH_PENALTY_BPS {
            panic!("PenaltyTooHigh: penalty_bps must be 0..=2000");
        }
        let key = constants::storage::launch_penalty_bps(&creator);
        env.storage().persistent().set(&key, &penalty_bps);
        extend_key_ttl_to_full_window(&env, &key);
        env.events().publish(
            events::launch_penalty_set_topics(&creator),
            events::LaunchPenaltySetEvent {
                creator_id: creator,
                penalty_bps,
                ledger: env.ledger().sequence(),
            },
        );
    }

    /// Returns the custom launch penalty basis points for a creator,
    /// or `None` if the default (500 bps) should be used.
    pub fn get_launch_penalty_bps(env: Env, creator: Address) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&constants::storage::launch_penalty_bps(&creator))
    }

    /// Returns the ledger sequence at which the first key was bought.
    pub fn get_created_at_ledger(env: Env, creator: Address) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&constants::storage::created_at_ledger(&creator))
    }

    /// Configures the sell lockup duration enforced on every sell.
    ///
    /// Only the protocol admin may call this. A duration of 0 returns
    /// [`ContractError::NotPositiveAmount`]; use [`DEFAULT_LOCKUP_DURATION_SECS`]
    /// (24 hours) as the canonical starting value. Once configured, `sell_key`
    /// rejects sales made less than `duration_secs` after the seller's most
    /// recent buy of that creator's keys with
    /// [`ContractError::AllocationLocked`] and emits a
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
    /// - [`ContractError::ZeroAddress`] if the recipient is the zero address.
    /// - [`ContractError::CooldownActive`] if the sender is inside the creator's buy cooldown.
    /// - [`ContractError::FrozenPosition`] if the sender's frozen keys block the transfer.
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
        assert_position_not_frozen(&env, &creator, &from)?;

        if amount == 0 {
            return Err(ContractError::ZeroTransferAmount);
        }
        if from == to {
            return Err(ContractError::SelfTransfer);
        }
        validate_non_zero_address(&env, &to)?;

        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

        // Reject transfers while the sender is inside the creator's buy cooldown window.
        let cooldown_ledgers: u32 = env
            .storage()
            .persistent()
            .get(&constants::storage::buy_cooldown(&creator))
            .unwrap_or(0);
        if cooldown_ledgers > 0 {
            if let Some(last_ledger) = env
                .storage()
                .persistent()
                .get::<DataKey, u32>(&constants::storage::last_buy_ledger(&creator, &from))
            {
                if env.ledger().sequence().saturating_sub(last_ledger) < cooldown_ledgers {
                    return Err(ContractError::CooldownActive);
                }
            }
        }

        let from_balance_key = constants::storage::holder_balance_key(&creator, &from);
        let from_balance: u32 = env
            .storage()
            .persistent()
            .get(&from_balance_key)
            .unwrap_or(0);

        // Settle dividends for sender before balance changes.
        settle_holder_dividends(&env, &creator, &from, from_balance)?;

        if available_holder_balance(&env, &creator, &from) < amount {
            // Frozen keys are what make an otherwise sufficient balance unavailable.
            if read_self_frozen_balance(&env, &creator, &from) > 0 && from_balance >= amount {
                return Err(ContractError::FrozenPosition);
            }
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
        assert_within_holding_cap(&env, &creator, new_to_balance)?;
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

    /// Transfers keys from the caller to multiple recipients in a single
    /// atomic transaction.
    ///
    /// Accepts up to [`MAX_BATCH_TRANSFER_SIZE`] `(recipient, quantity)` pairs.
    /// All transfers are processed atomically: if any single step fails, the
    /// entire batch reverts and no state changes are persisted.
    ///
    /// Dividend checkpoints are settled for the sender and for each recipient
    /// before any balance is modified, so dividend accounting stays consistent
    /// across the whole batch.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    /// - [`ContractError::BatchTransferSizeExceeded`] if `transfers` contains
    ///   more than [`MAX_BATCH_TRANSFER_SIZE`] entries.
    /// - [`ContractError::ZeroTransferAmount`] if any entry has a zero quantity.
    /// - [`ContractError::InvalidRecipient`] if any recipient equals the sender
    ///   (self-transfer is not allowed inside a batch).
    /// - [`ContractError::InsufficientBalance`] if the sender's available
    ///   balance is less than the sum of all requested quantities.
    pub fn batch_transfer_keys(
        env: Env,
        creator: Address,
        from: Address,
        transfers: Vec<(Address, u32)>,
    ) -> Result<(), ContractError> {
        from.require_auth();
        assert_not_paused(&env)?;
        assert_position_not_frozen(&env, &creator, &from)?;

        if transfers.len() > MAX_BATCH_TRANSFER_SIZE {
            return Err(ContractError::BatchTransferSizeExceeded);
        }

        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

        // --- Pre-flight validation pass ---
        // Validate every entry and total the quantities before touching state.
        let mut total_quantity: u32 = 0;
        for (to, qty) in transfers.iter() {
            if qty == 0 {
                return Err(ContractError::ZeroTransferAmount);
            }
            if to == from {
                return Err(ContractError::InvalidRecipient);
            }
            total_quantity = total_quantity
                .checked_add(qty)
                .ok_or(ContractError::Overflow)?;
        }

        // Read sender balance and settle dividends before any mutations.
        let from_balance_key = constants::storage::holder_balance_key(&creator, &from);
        let from_balance: u32 = env
            .storage()
            .persistent()
            .get(&from_balance_key)
            .unwrap_or(0);
        settle_holder_dividends(&env, &creator, &from, from_balance)?;

        if available_holder_balance(&env, &creator, &from) < total_quantity {
            return Err(ContractError::InsufficientBalance);
        }

        // --- Apply all transfers atomically ---
        let mut remaining_from_balance = from_balance;
        for (to, qty) in transfers.iter() {
            let to_balance_key = constants::storage::holder_balance_key(&creator, &to);
            let to_balance: u32 = env.storage().persistent().get(&to_balance_key).unwrap_or(0);

            // Settle dividends for each recipient before their balance changes.
            settle_holder_dividends(&env, &creator, &to, to_balance)?;

            // Decrement the sender's running balance.
            remaining_from_balance = remaining_from_balance
                .checked_sub(qty)
                .ok_or(ContractError::InsufficientBalance)?;

            // Increment the recipient balance.
            let new_to_balance = to_balance.checked_add(qty).ok_or(ContractError::Overflow)?;
            assert_within_holding_cap(&env, &creator, new_to_balance)?;
            env.storage()
                .persistent()
                .set(&to_balance_key, &new_to_balance);
            extend_key_ttl_to_full_window(&env, &to_balance_key);

            // Increment holder count when the recipient had zero balance before.
            if to_balance == 0 {
                profile.holder_count = profile
                    .holder_count
                    .checked_add(1)
                    .ok_or(ContractError::Overflow)?;
            }
        }

        // Write the final sender balance.
        env.storage()
            .persistent()
            .set(&from_balance_key, &remaining_from_balance);
        extend_key_ttl_to_full_window(&env, &from_balance_key);

        // Decrement holder count if the sender balance reaches zero.
        if remaining_from_balance == 0 {
            profile.holder_count = profile
                .holder_count
                .checked_sub(1)
                .ok_or(ContractError::Overflow)?;
        }

        // Write the updated profile (holder_count may have changed).
        let profile_key = constants::storage::creator(&creator);
        env.storage().persistent().set(&profile_key, &profile);

        env.events().publish(
            events::batch_transfer_completed_topics(&creator, &from),
            events::BatchTransferCompletedEvent {
                creator_id: creator,
                from,
                transfers,
                total_transferred: total_quantity,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
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
    // #806 — Staking lifecycle
    // =========================================================================

    /// Returns the next sequential stake id for a `(creator, holder)` pair.
    fn next_stake_id(env: &Env, creator: &Address, holder: &Address) -> Result<u32, StakingError> {
        let id_key = constants::storage::next_stake_id(creator, holder);
        let next: u32 = env.storage().persistent().get(&id_key).unwrap_or(0);
        let new_next = next.checked_add(1).ok_or(StakingError::Overflow)?;
        env.storage().persistent().set(&id_key, &new_next);
        env.storage()
            .persistent()
            .extend_ttl(&id_key, CREATOR_TTL_LEDGERS, CREATOR_TTL_LEDGERS);
        Ok(next)
    }

    /// Locks `amount` keys into a staking position that matures
    /// `lock_ledgers` ledgers from now.
    ///
    /// The holder must authorize the call. Staked keys are removed from the
    /// holder's liquid balance and cannot be sold until the position matures.
    /// The staking rewards pool for `creator` accrues a fixed share of every
    /// protocol trade fee, so locked positions earn a pro-rata share of the
    /// pool when they mature.
    ///
    /// # Errors
    ///
    /// - [`StakingError::NotPositiveAmount`] if `amount` or `lock_ledgers` is zero
    /// - [`StakingError::InsufficientBalance`] if the holder's liquid balance is
    ///   smaller than `amount`
    pub fn stake_keys_locked(
        env: Env,
        creator: Address,
        holder: Address,
        amount: u32,
        lock_ledgers: u32,
    ) -> Result<u32, StakingError> {
        holder.require_auth();
        assert_not_paused(&env).map_err(map_staking_error)?;

        if amount == 0 || lock_ledgers == 0 {
            return Err(StakingError::NotPositiveAmount);
        }

        read_registered_creator_profile(&env, &creator).map_err(map_staking_error)?;

        let balance_key = constants::storage::key_balance(&creator, &holder);
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        let current_staked: u32 =
            Self::get_staked_balance(env.clone(), creator.clone(), holder.clone());
        let liquid_balance = current_balance.saturating_sub(current_staked);
        if liquid_balance < amount {
            return Err(StakingError::InsufficientBalance);
        }

        let stake_id = Self::next_stake_id(&env, &creator, &holder)?;
        let unlock_ledger = env
            .ledger()
            .sequence()
            .checked_add(lock_ledgers)
            .ok_or(StakingError::Overflow)?;

        let position = StakePosition {
            stake_id,
            amount,
            unlock_ledger,
        };
        let position_key = constants::storage::staking_position(&creator, &holder, stake_id);
        env.storage().persistent().set(&position_key, &position);
        extend_key_ttl_to_full_window(&env, &position_key);

        // Book the keys as staked so sell-gating and liquid-balance views stay
        // consistent with the existing `stake_keys` accounting.
        let new_staked = current_staked
            .checked_add(amount)
            .ok_or(StakingError::Overflow)?;
        let staked_balance_key = constants::storage::staked_balance(&creator, &holder);
        env.storage()
            .persistent()
            .set(&staked_balance_key, &new_staked);
        extend_key_ttl_to_full_window(&env, &staked_balance_key);

        // Track the cross-holder staked total for reward distribution.
        let pool_key = constants::storage::staking_rewards_pool(&creator);
        let mut state: StakingRewardsState =
            env.storage()
                .persistent()
                .get(&pool_key)
                .unwrap_or(StakingRewardsState {
                    pool: 0,
                    total_staked: 0,
                });
        state.total_staked = state
            .total_staked
            .checked_add(amount)
            .ok_or(StakingError::Overflow)?;
        env.storage().persistent().set(&pool_key, &state);
        extend_key_ttl_to_full_window(&env, &pool_key);

        env.events().publish(
            events::stake_topics(&creator, &holder, stake_id),
            events::StakeEvent {
                creator_id: creator,
                holder,
                stake_id,
                amount,
                unlock_ledger,
            },
        );

        Ok(stake_id)
    }

    /// Extends the lock period of `stake_id` by `additional_ledgers` ledgers.
    ///
    /// The holding period is extended from the current maturity ledger, pushing
    /// `unlock_ledger` forward. The staker continues to accrue a pro-rata share
    /// of the reward pool for the extended period.
    pub fn stake_extend(
        env: Env,
        creator: Address,
        holder: Address,
        stake_id: u32,
        additional_ledgers: u32,
    ) -> Result<u32, StakingError> {
        holder.require_auth();
        assert_not_paused(&env).map_err(map_staking_error)?;

        if additional_ledgers == 0 {
            return Err(StakingError::NotPositiveAmount);
        }

        let position_key = constants::storage::staking_position(&creator, &holder, stake_id);
        let mut position: StakePosition = env
            .storage()
            .persistent()
            .get(&position_key)
            .ok_or(StakingError::PositionNotFound)?;

        position.unlock_ledger = position
            .unlock_ledger
            .checked_add(additional_ledgers)
            .ok_or(StakingError::Overflow)?;
        env.storage().persistent().set(&position_key, &position);
        extend_key_ttl_to_full_window(&env, &position_key);

        env.events().publish(
            events::stake_extended_topics(&creator, &holder, stake_id),
            events::StakeExtendedEvent {
                creator_id: creator,
                holder,
                stake_id,
                unlock_ledger: position.unlock_ledger,
                additional_ledgers,
            },
        );

        Ok(position.unlock_ledger)
    }

    /// Sets early exit penalty basis points for `key_id` (0 to 5000 bps, i.e. 0%–50%).
    ///
    /// Callable by the key creator. Default is 2000 bps (20%).
    /// Panics with `PenaltyTooHigh` if `penalty_bps > 5000`.
    pub fn set_early_exit_penalty(env: Env, key_id: Address, penalty_bps: u32) {
        key_id.require_auth();
        if penalty_bps > 5000 {
            panic!("PenaltyTooHigh: penalty_bps must be 0..=5000");
        }
        let storage_key = constants::storage::early_exit_penalty_bps(&key_id);
        env.storage().persistent().set(&storage_key, &penalty_bps);
        extend_key_ttl_to_full_window(&env, &storage_key);
    }

    /// Read-only view: returns the early exit penalty bps configured for `key_id`.
    /// Defaults to 2000 bps (20%) if unconfigured.
    pub fn get_early_exit_penalty_bps(env: Env, key_id: Address) -> u32 {
        let storage_key = constants::storage::early_exit_penalty_bps(&key_id);
        env.storage().persistent().get(&storage_key).unwrap_or(2000)
    }

    /// Allows stakers to exit early before their lock expires by forfeiting a penalty.
    ///
    /// Callable by any wallet with an active stake for `key_id`.
    /// Panics with `NoStakeFound` if the wallet has no active stake for `key_id`.
    pub fn early_unstake(env: Env, key_id: Address, wallet: Address) {
        let (actual_key_id, actual_wallet) =
            if Self::get_staked_balance(env.clone(), key_id.clone(), wallet.clone()) > 0 {
                (key_id.clone(), wallet.clone())
            } else if Self::get_staked_balance(env.clone(), wallet.clone(), key_id.clone()) > 0 {
                (wallet.clone(), key_id.clone())
            } else {
                (key_id.clone(), wallet.clone())
            };

        actual_wallet.require_auth();
        assert_not_paused(&env).unwrap();

        let staked_quantity =
            Self::get_staked_balance(env.clone(), actual_key_id.clone(), actual_wallet.clone());
        if staked_quantity == 0 {
            panic!("NoStakeFound: wallet has no active stake for key_id");
        }

        let penalty_bps = Self::get_early_exit_penalty_bps(env.clone(), actual_key_id.clone());
        let penalty_quantity = ((staked_quantity as u64) * (penalty_bps as u64) / 10000) as u32;
        let returned_quantity = staked_quantity - penalty_quantity;

        // Clear staked balance for wallet
        let staked_balance_key = constants::storage::staked_balance(&actual_key_id, &actual_wallet);
        env.storage().persistent().remove(&staked_balance_key);
        env.storage()
            .persistent()
            .remove(&constants::storage::stake_unlock_ledger(
                &actual_key_id,
                &actual_wallet,
            ));

        // Update total staked for key_id
        let total_staked_key = constants::storage::total_staked(&actual_key_id);
        let current_total_staked = read_total_staked(&env, &actual_key_id);
        let new_total_staked = current_total_staked.saturating_sub(staked_quantity);
        if new_total_staked == 0 {
            env.storage().persistent().remove(&total_staked_key);
        } else {
            env.storage()
                .persistent()
                .set(&total_staked_key, &new_total_staked);
        }

        // Deduct penalty_quantity from holder's total key_balance
        let balance_key = constants::storage::holder_balance_key(&actual_key_id, &actual_wallet);
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        let new_balance = current_balance.saturating_sub(penalty_quantity);
        if new_balance == 0 {
            env.storage().persistent().remove(&balance_key);
        } else {
            env.storage().persistent().set(&balance_key, &new_balance);
        }

        // Add penalty_quantity to staking_rewards pool for key_id
        let pool_key = constants::storage::staking_rewards_pool(&actual_key_id);
        let mut state: StakingRewardsState =
            env.storage()
                .persistent()
                .get(&pool_key)
                .unwrap_or(StakingRewardsState {
                    pool: 0,
                    total_staked: 0,
                });
        state.pool = state.pool.saturating_add(penalty_quantity as i128);
        env.storage().persistent().set(&pool_key, &state);

        // Emit early_unstake event
        env.events().publish(
            events::early_unstake_penalty_topics(&actual_key_id, &actual_wallet),
            events::EarlyUnstakePenaltyEvent {
                wallet: actual_wallet,
                key_id: actual_key_id,
                returned_quantity,
                penalty_quantity,
            },
        );
    }

    /// Unstakes `stake_id` before its lock period elapses for a specific positional stake.
    pub fn early_unstake_position(
        env: Env,
        creator: Address,
        holder: Address,
        stake_id: u32,
    ) -> Result<StakeExit, StakingError> {
        holder.require_auth();
        assert_not_paused(&env).map_err(map_staking_error)?;

        let position_key = constants::storage::staking_position(&creator, &holder, stake_id);
        let position: StakePosition = env
            .storage()
            .persistent()
            .get(&position_key)
            .ok_or(StakingError::PositionNotFound)?;

        if env.ledger().sequence() >= position.unlock_ledger {
            return Err(StakingError::PositionNotLocked);
        }

        let pool_key = constants::storage::staking_rewards_pool(&creator);
        let mut state: StakingRewardsState =
            env.storage()
                .persistent()
                .get(&pool_key)
                .unwrap_or(StakingRewardsState {
                    pool: 0,
                    total_staked: 0,
                });

        // Pro-rata share of the current pool this position would have earned at
        // maturity. Guard division by zero.
        let reward_share = if state.total_staked > 0 {
            (i128::from(position.amount) * state.pool) / i128::from(state.total_staked)
        } else {
            0
        };
        let penalty =
            fee::apply_percentage_fee(reward_share, crate::staking::EARLY_UNSTAKE_PENALTY_BPS)
                .ok_or(StakingError::Overflow)?;

        // Remove the entitlement, then retain the penalty on behalf of the
        // remaining stakers: pool' = pool - entitlement + penalty.
        let new_pool = state
            .pool
            .checked_sub(reward_share)
            .ok_or(StakingError::Overflow)?
            .checked_add(penalty)
            .ok_or(StakingError::Overflow)?;
        state.pool = new_pool;
        state.total_staked = state
            .total_staked
            .checked_sub(position.amount)
            .ok_or(StakingError::Overflow)?;
        if state.total_staked == 0 && state.pool == 0 {
            env.storage().persistent().remove(&pool_key);
        } else {
            env.storage().persistent().set(&pool_key, &state);
            extend_key_ttl_to_full_window(&env, &pool_key);
        }

        // Release the keys back into the holder's liquid balance.
        env.storage().persistent().remove(&position_key);
        sub_staked_balance(&env, &creator, &holder, position.amount);

        let result = StakeExit {
            stake_id,
            amount: position.amount,
            forgone_reward: reward_share,
            penalty,
        };
        env.events().publish(
            events::early_unstake_topics(&creator, &holder, stake_id),
            events::EarlyUnstakeEvent {
                creator_id: creator,
                holder,
                stake_id,
                amount: result.amount,
                forgone_reward: result.forgone_reward,
                penalty: result.penalty,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(result)
    }

    /// Claims the staking reward for `stake_id` once its lock period has elapsed.
    ///
    /// Pays out the position's pro-rata share of the pool, releases the keys to
    /// the holder's liquid balance and removes the position.
    pub fn claim_stake_reward(
        env: Env,
        creator: Address,
        holder: Address,
        stake_id: u32,
    ) -> Result<StakeRewardClaim, StakingError> {
        holder.require_auth();
        assert_not_paused(&env).map_err(map_staking_error)?;

        let position_key = constants::storage::staking_position(&creator, &holder, stake_id);
        let position: StakePosition = env
            .storage()
            .persistent()
            .get(&position_key)
            .ok_or(StakingError::PositionNotFound)?;

        if env.ledger().sequence() < position.unlock_ledger {
            return Err(StakingError::PositionLocked);
        }

        let pool_key = constants::storage::staking_rewards_pool(&creator);
        let mut state: StakingRewardsState =
            env.storage()
                .persistent()
                .get(&pool_key)
                .unwrap_or(StakingRewardsState {
                    pool: 0,
                    total_staked: 0,
                });

        let reward = if state.total_staked > 0 {
            (i128::from(position.amount) * state.pool) / i128::from(state.total_staked)
        } else {
            0
        };
        state.pool = state
            .pool
            .checked_sub(reward)
            .ok_or(StakingError::Overflow)?;
        state.total_staked = state
            .total_staked
            .checked_sub(position.amount)
            .ok_or(StakingError::Overflow)?;
        if state.total_staked == 0 && state.pool == 0 {
            env.storage().persistent().remove(&pool_key);
        } else {
            env.storage().persistent().set(&pool_key, &state);
            extend_key_ttl_to_full_window(&env, &pool_key);
        }

        // Release the keys back into the holder's liquid balance.
        env.storage().persistent().remove(&position_key);
        sub_staked_balance(&env, &creator, &holder, position.amount);

        let result = StakeRewardClaim {
            stake_id,
            amount: position.amount,
            reward,
        };
        env.events().publish(
            events::stake_reward_claimed_topics(&creator, &holder, stake_id),
            events::StakeRewardClaimedEvent {
                creator_id: creator,
                holder,
                stake_id,
                amount: result.amount,
                reward: result.reward,
                unlock_ledger: position.unlock_ledger,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(result)
    }

    /// Read-only view: returns the stored staking position for
    /// `(creator, holder, stake_id)`, or `None` if no such position exists.
    pub fn get_staking_position(
        env: Env,
        creator: Address,
        holder: Address,
        stake_id: u32,
    ) -> Option<StakePosition> {
        env.storage()
            .persistent()
            .get(&constants::storage::staking_position(
                &creator, &holder, stake_id,
            ))
    }

    /// Read-only view: returns the current staking rewards pool for `creator`.
    pub fn get_staking_rewards_pool(env: Env, creator: Address) -> i128 {
        env.storage()
            .persistent()
            .get::<DataKey, StakingRewardsState>(&constants::storage::staking_rewards_pool(
                &creator,
            ))
            .map(|state| state.pool)
            .unwrap_or(0)
    }

    /// Read-only view: returns the total number of keys currently staked for
    /// `creator` across all holders.
    pub fn get_total_staked(env: Env, creator: Address) -> u32 {
        env.storage()
            .persistent()
            .get::<DataKey, StakingRewardsState>(&constants::storage::staking_rewards_pool(
                &creator,
            ))
            .map(|state| state.total_staked)
            .unwrap_or(0)
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

    /// Read-only view: returns the current live pause state for a key.
    pub fn get_pause_state(env: Env, key_id: Address) -> Option<PauseState> {
        env.storage()
            .persistent()
            .get(&constants::storage::pause_state(&key_id))
    }

    /// Sets a timed pause for a key's trading via the creator's multisig admin flow.
    ///
    /// `duration_ledgers` must be in the inclusive range `1..=17_280` or the call
    /// panics with [`ContractError::PauseTooLong`]. Only a configured admin may call.
    pub fn pause_with_expiry(
        env: Env,
        creator: Address,
        caller: Address,
        duration_ledgers: u32,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let config: MultisigAdmins = env
            .storage()
            .persistent()
            .get(&constants::storage::multisig_admins(&creator))
            .ok_or(ContractError::Unauthorized)?;

        if !config.admins.iter().any(|admin| admin == caller) {
            return Err(ContractError::Unauthorized);
        }

        if duration_ledgers == 0 || duration_ledgers > 17_280 {
            return Err(ContractError::PauseTooLong);
        }

        let pause_expires_at = env.ledger().sequence().saturating_add(duration_ledgers);
        env.storage().persistent().set(
            &constants::storage::pause_state(&creator),
            &PauseState {
                trading_paused: true,
                pause_expires_at,
            },
        );

        env.events().publish(
            events::pause_expiry_set_topics(&creator),
            events::PauseExpirySetEvent {
                key_id: creator,
                pause_expires_at,
            },
        );

        Ok(())
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
    /// subtracting already-claimed keys. Returns `AlreadyClaimed` if no
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
            return Err(ContractError::AllocationLocked);
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
            .ok_or(ContractError::AlreadyClaimed)?;

        if claimable == 0 {
            return Err(ContractError::AlreadyClaimed);
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

    /// Admin sets the upper bound a creator may choose for their per-wallet
    /// holding cap via [`Self::set_holding_cap`].
    pub fn set_holding_cap_bound(
        env: Env,
        admin: Address,
        bound: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        if bound == 0 {
            return Err(ContractError::InvalidHolderCap);
        }
        let key = constants::storage::MAX_HOLDING_BOUND;
        env.storage().persistent().set(&key, &bound);
        extend_key_ttl_to_full_window(&env, &key);
        Ok(())
    }

    /// Creator updates the maximum number of keys one wallet may hold.
    ///
    /// Reuses the per-wallet cap already enforced by `buy_key`, and now also
    /// by `transfer_keys` and `batch_transfer_keys`. `new_cap` must be greater
    /// than zero and, when the admin has set a bound, not exceed it.
    pub fn set_holding_cap(env: Env, creator: Address, new_cap: u32) -> Result<(), ContractError> {
        creator.require_auth();
        read_registered_creator_profile(&env, &creator)?;
        if new_cap == 0 {
            return Err(ContractError::InvalidHolderCap);
        }
        if let Some(bound) = env
            .storage()
            .persistent()
            .get::<DataKey, u32>(&constants::storage::MAX_HOLDING_BOUND)
        {
            if new_cap > bound {
                return Err(ContractError::InvalidHolderCap);
            }
        }
        let key = constants::storage::max_keys_per_wallet(&creator);
        let old_cap: Option<u32> = env.storage().persistent().get(&key);
        env.storage().persistent().set(&key, &new_cap);
        extend_key_ttl_to_full_window(&env, &key);

        env.events().publish(
            events::holding_cap_updated_topics(&creator),
            events::HoldingCapUpdatedEvent {
                creator,
                old_cap,
                new_cap,
            },
        );
        Ok(())
    }

    /// Read-only view: the per-wallet holding cap, or `None` when uncapped.
    pub fn get_holding_cap(env: Env, creator: Address) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&constants::storage::max_keys_per_wallet(&creator))
    }

    /// Creator or admin toggles early-access mode. While on, only whitelisted
    /// wallets may buy; turning it off opens trading to everyone. Shares the
    /// flag used by `enable_whitelist` / `disable_whitelist`.
    pub fn set_early_access_mode(
        env: Env,
        caller: Address,
        creator: Address,
        enabled: bool,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        assert_creator_or_admin(&env, &caller, &creator)?;
        let mode_key = constants::storage::whitelist_mode(&creator);
        env.storage().persistent().set(&mode_key, &enabled);
        extend_key_ttl_to_full_window(&env, &mode_key);
        Ok(())
    }

    /// Creator or admin adds (`allowed = true`) or removes a wallet from the
    /// early-access whitelist and emits `WhitelistUpdatedEvent`.
    pub fn update_whitelist(
        env: Env,
        caller: Address,
        creator: Address,
        wallet: Address,
        allowed: bool,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        assert_creator_or_admin(&env, &caller, &creator)?;
        let entry_key = constants::storage::whitelist_entry(&creator, &wallet);
        env.storage().persistent().set(&entry_key, &allowed);
        extend_key_ttl_to_full_window(&env, &entry_key);

        env.events().publish(
            events::whitelist_updated_topics(&creator),
            events::WhitelistUpdatedEvent {
                creator,
                wallet,
                allowed,
            },
        );
        Ok(())
    }

    /// Read-only view: whether `wallet` is on `creator`'s early-access whitelist.
    pub fn get_wallet_whitelist_status(env: Env, creator: Address, wallet: Address) -> bool {
        env.storage()
            .persistent()
            .get(&constants::storage::whitelist_entry(&creator, &wallet))
            .unwrap_or(false)
    }

    /// Admin sets the share of the protocol fee (in bps) paid to a referrer on
    /// a referee's first registered trade. Defaults to 5000 (50%) when unset.
    pub fn set_referral_fee_bps(env: Env, admin: Address, bps: u32) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        if bps > fee::BPS_MAX {
            return Err(ContractError::InvalidFeeConfig);
        }
        let key = constants::storage::referral_fee_bps();
        env.storage().persistent().set(&key, &bps);
        extend_key_ttl_to_full_window(&env, &key);
        Ok(())
    }

    /// Links `referee` to `referrer`. Allowed once per referee and only before
    /// the referee's referral reward has been paid. The next buy by the referee
    /// without an explicit referrer pays the reward; later buys pay nothing.
    pub fn register_referral(
        env: Env,
        referee: Address,
        referrer: Address,
    ) -> Result<(), ContractError> {
        referee.require_auth();
        if referee == referrer {
            return Err(ContractError::InvalidReferrer);
        }
        let key = constants::storage::referrer_of(&referee);
        if env.storage().persistent().has(&key) {
            return Err(ContractError::AlreadyRegistered);
        }
        env.storage().persistent().set(&key, &referrer);
        extend_key_ttl_to_full_window(&env, &key);

        env.events().publish(
            (events::referral_registered_topics(),),
            events::ReferralRegisteredEvent { referee, referrer },
        );
        Ok(())
    }

    /// Read-only view: the referrer registered for `referee`, if any.
    pub fn get_referrer(env: Env, referee: Address) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&constants::storage::referrer_of(&referee))
    }

    /// Referrer withdraws all accumulated referral rewards. Returns the amount
    /// claimed and resets the balance to zero.
    pub fn claim_referral_rewards(env: Env, referrer: Address) -> Result<i128, ContractError> {
        referrer.require_auth();
        let key = constants::storage::referral_earnings(&referrer);
        let amount: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if amount <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        env.storage().persistent().set(&key, &0i128);
        extend_key_ttl_to_full_window(&env, &key);

        env.events().publish(
            (events::referral_rewards_claimed_topics(),),
            events::ReferralRewardsClaimedEvent { referrer, amount },
        );
        Ok(amount)
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

        if available_holder_balance(&env, &key_id, &caller) < quantity {
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

        let delegate_key = constants::storage::delegate(&creator_id, &voter);
        if env.storage().persistent().has(&delegate_key) {
            return Err(PollError::Unauthorized);
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
    // Delegated Voting
    // =========================================================================

    /// Assigns voting power to a delegate wallet.
    pub fn delegate(env: Env, creator_id: Address, delegator: Address, delegate: Address) {
        delegator.require_auth();
        let delegate_key = constants::storage::delegate(&creator_id, &delegator);
        env.storage().persistent().set(&delegate_key, &delegate);
        extend_key_ttl_to_full_window(&env, &delegate_key);

        env.events().publish(
            events::delegation_set_topics(&creator_id, &delegator),
            events::DelegationSetEvent {
                creator: creator_id,
                delegator,
                delegate,
            },
        );
    }

    /// Revokes a previous delegation.
    pub fn revoke_delegate(env: Env, creator_id: Address, delegator: Address) {
        delegator.require_auth();
        let delegate_key = constants::storage::delegate(&creator_id, &delegator);
        env.storage().persistent().remove(&delegate_key);

        env.events().publish(
            events::delegation_revoked_topics(&creator_id, &delegator),
            events::DelegationRevokedEvent {
                creator: creator_id,
                delegator,
            },
        );
    }

    /// Read-only view: returns the current delegate for a wallet.
    pub fn get_delegate(env: Env, creator_id: Address, delegator: Address) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&constants::storage::delegate(&creator_id, &delegator))
    }

    /// Casts a vote on behalf of delegators using their delegated weight.
    pub fn cast_delegated_vote(
        env: Env,
        creator_id: Address,
        delegate: Address,
        delegators: Vec<Address>,
        poll_id: u32,
        option_index: u32,
    ) -> Result<(), crate::events::PollError> {
        use crate::events::{PollError, PollVote, POLL_VOTE_EVENT_NAME};

        delegate.require_auth();
        let mut poll = events::read_poll(&env, &creator_id, poll_id)?;

        if events::is_poll_expired(&env, &poll) {
            return Err(PollError::PollExpired);
        }
        if option_index >= poll.options.len() {
            return Err(PollError::InvalidOption);
        }

        for delegator in delegators.iter() {
            let actual_delegate: Option<Address> = env
                .storage()
                .persistent()
                .get(&constants::storage::delegate(&creator_id, &delegator));
            if actual_delegate != Some(delegate.clone()) {
                return Err(PollError::Unauthorized);
            }

            let snapshot_key =
                DataKey::VoteSnapshot(creator_id.clone(), poll_id, delegator.clone());
            let weight: u32 = if let Some(snap) = env
                .storage()
                .persistent()
                .get::<DataKey, u32>(&snapshot_key)
            {
                snap
            } else {
                let balance_key = constants::storage::holder_balance_key(&creator_id, &delegator);
                let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
                if balance == 0 {
                    continue;
                }
                env.storage().persistent().set(&snapshot_key, &balance);
                balance
            };

            if weight == 0 {
                continue;
            }

            let vote_key = events::vote_storage_key(&creator_id, poll_id, &delegator);
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

            env.storage().persistent().set(
                &vote_key,
                &PollVote {
                    option_index,
                    weight,
                },
            );
            env.events().publish(
                (
                    POLL_VOTE_EVENT_NAME,
                    creator_id.clone(),
                    poll_id,
                    delegator.clone(),
                ),
                (option_index, weight),
            );
        }

        env.storage()
            .persistent()
            .set(&events::poll_storage_key(&creator_id, poll_id), &poll);

        Ok(())
    }

    // =========================================================================
    // Governance Quorum Requirement
    // =========================================================================

    /// Sets the minimum quorum threshold in basis points (100–5000, corresponding
    /// to 1%–50% of circulating key supply) required for creator proposals/polls
    /// to pass and be closed.
    ///
    /// Callable only by the registered creator.
    pub fn set_quorum_bps(
        env: Env,
        creator: Address,
        quorum_bps: u32,
    ) -> Result<(), crate::events::PollError> {
        use crate::events::PollError;

        creator.require_auth();
        let profile = read_registered_creator_profile(&env, &creator)
            .map_err(|_| PollError::NotRegistered)?;
        if profile.creator != creator {
            return Err(PollError::Unauthorized);
        }

        if quorum_bps > 5000 {
            return Err(PollError::QuorumTooHigh);
        }
        if quorum_bps < 100 {
            return Err(PollError::QuorumTooLow);
        }

        let quorum_key = constants::storage::quorum_bps(&creator);
        env.storage().persistent().set(&quorum_key, &quorum_bps);
        extend_key_ttl_to_full_window(&env, &quorum_key);

        env.events().publish(
            events::quorum_updated_topics(&creator),
            events::QuorumUpdatedEvent {
                creator,
                quorum_bps,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    /// Read-only view: returns the configured quorum threshold in basis points
    /// for a creator, or 0 if unset.
    pub fn get_quorum_bps(env: Env, creator: Address) -> u32 {
        let quorum_key = constants::storage::quorum_bps(&creator);
        env.storage().persistent().get(&quorum_key).unwrap_or(0)
    }

    /// Re-extends all known global persistent storage keys plus the scoped
    /// entries of the specified creators to the maximum TTL window.
    pub fn refresh_ttl(
        env: Env,
        admin: Address,
        creators: Vec<Address>,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        let global_keys = [
            constants::storage::FEE_CONFIG,
            constants::storage::KEY_PRICE,
            constants::storage::TREASURY_ADDRESS,
            constants::storage::ADMIN_ADDRESS,
            constants::storage::PROTOCOL_FEE_RECIPIENT,
            constants::storage::PROTOCOL_FEE_RECIPIENT_BALANCE,
            constants::storage::PROTOCOL_STATE_VERSION,
            constants::storage::PAUSED,
            constants::storage::CURVE_SLOPE,
            constants::storage::TREASURY_BALANCE,
            constants::storage::RETENTION_POLICY,
            constants::storage::GLOBAL_DEADLINE_LEDGER,
            constants::storage::referral_fee_bps(),
            constants::storage::PROTOCOL_FEE_BPS,
            constants::storage::LOCKUP_DURATION_SECS,
        ];
        for key in global_keys.iter() {
            if env.storage().persistent().has(key) {
                extend_key_ttl_to_full_window(&env, key);
            }
        }

        for creator in creators.iter() {
            extend_creator_ttl(&env, &creator);
            let whitelist_key = constants::storage::whitelist(&creator);
            if env.storage().persistent().has(&whitelist_key) {
                extend_key_ttl_to_full_window(&env, &whitelist_key);
            }
        }

        Ok(())
    }

    /// Executes multiple key purchases across different creators in a single transaction.
    pub fn batch_buy(
        env: Env,
        buyer: Address,
        orders: Vec<(Address, u32)>,
    ) -> Result<Vec<BatchBuyOrderResult>, ContractError> {
        buyer.require_auth();
        assert_not_paused(&env)?;
        assert_not_blacklisted(&env, &buyer)?;
        assert_before_global_deadline(&env)?;

        if orders.is_empty() || orders.len() > MAX_BATCH_BUY_SIZE as u32 {
            return Err(ContractError::BatchClaimExceedsLimit);
        }

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;

        let mut results = soroban_sdk::Vec::new(&env);
        let mut total_price_paid: i128 = 0;

        for order in orders.iter() {
            let (creator, quantity) = order;
            if quantity == 0 {
                return Err(ContractError::NotPositiveAmount);
            }

            assert_position_not_frozen(&env, &creator, &buyer)?;
            let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;
            assert_whitelist_allows_buy(&env, &profile, &buyer)?;

            let mut order_price: i128 = 0;

            let mut i = 0u32;
            while i < quantity {
                let price =
                    compute_bonding_curve_price(&env, &creator, base_price, profile.supply)?;

                if let Some(config) = read_protocol_fee_config(&env) {
                    let (creator_fee, protocol_fee) = fee::checked_compute_fee_split(
                        price,
                        config.creator_bps,
                        config.protocol_bps,
                    )
                    .ok_or(ContractError::Overflow)?;
                    credit_creator_fee(&env, &creator, creator_fee)?;
                    credit_treasury_balance(&env, protocol_fee)?;
                    credit_protocol_fee_recipient_balance(&env, protocol_fee)?;
                }

                if let Some(royalty) = read_royalty_config(&env, &creator) {
                    let royalty_amount = fee::apply_percentage_fee(price, royalty.buy_fee_bps)
                        .ok_or(ContractError::Overflow)?;
                    if royalty_amount > 0 {
                        credit_creator_fee_recipient_balance(&env, &creator, royalty_amount)?;
                    }
                }

                order_price = order_price
                    .checked_add(price)
                    .ok_or(ContractError::Overflow)?;

                let balance_key = constants::storage::holder_balance_key(&creator, &buyer);
                let current_balance: u32 =
                    env.storage().persistent().get(&balance_key).unwrap_or(0);

                if current_balance == 0 {
                    profile.holder_count = profile
                        .holder_count
                        .checked_add(1)
                        .ok_or(ContractError::Overflow)?;
                }

                let key = constants::storage::creator(&creator);
                env.storage().persistent().set(&key, &profile);

                profile.supply = profile
                    .supply
                    .checked_add(1)
                    .ok_or(ContractError::Overflow)?;

                write_creator_supply(&env, &creator, profile.supply);

                let new_balance = current_balance
                    .checked_add(1)
                    .ok_or(ContractError::Overflow)?;
                env.storage().persistent().set(&balance_key, &new_balance);
                extend_key_ttl_to_full_window(&env, &balance_key);

                i += 1;
            }

            env.events().publish(
                events::buy_event_topics(&creator, &buyer),
                events::KeysBoughtEvent {
                    buyer: buyer.clone(),
                    creator_id: creator.clone(),
                    quantity,
                    price_paid: order_price,
                    new_supply: profile.supply,
                    ledger: env.ledger().sequence(),
                },
            );

            total_price_paid = total_price_paid
                .checked_add(order_price)
                .ok_or(ContractError::Overflow)?;

            results.push_back(BatchBuyOrderResult {
                creator,
                quantity,
                price_paid: order_price,
            });
        }

        env.events().publish(
            events::batch_buy_completed_topics(&buyer),
            events::BatchBuyCompletedEvent {
                buyer: buyer.clone(),
                total_price_paid,
                order_count: results.len(),
                ledger: env.ledger().sequence(),
            },
        );

        Ok(results)
    }

    /// Executes multiple key sales across different creators in a single transaction.
    ///
    /// Validates that `orders` contains between 1 and 5 entries and that the caller
    /// has sufficient liquid balance (excluding frozen and staked keys) for all orders.
    /// Each sell is processed sequentially using the bonding curve logic.
    /// If any single order fails, the entire batch reverts atomically.
    pub fn batch_sell(
        env: Env,
        seller: Address,
        orders: Vec<(Address, u32)>,
    ) -> Result<Vec<BatchSellOrderResult>, ContractError> {
        seller.require_auth();
        assert_global_trading_not_halted(&env)?;
        assert_not_paused(&env)?;
        assert_not_blacklisted(&env, &seller)?;

        if orders.is_empty() || orders.len() > MAX_BATCH_SELL_SIZE as u32 {
            return Err(ContractError::BatchSizeExceeded);
        }

        // --- Pre-flight validation pass ---
        // Validate each order and check that the caller holds sufficient liquid balance.
        for i in 0..orders.len() {
            let (creator, qty) = orders.get(i).unwrap();
            if qty == 0 {
                return Err(ContractError::NotPositiveAmount);
            }
            // Verify creator profile exists
            read_registered_creator_profile(&env, &creator)?;
            assert_position_not_frozen(&env, &creator, &seller)?;

            // Sum cumulative requested quantity across the batch for this creator
            let mut total_qty_for_creator: u32 = 0;
            for j in 0..orders.len() {
                let (other_creator, other_qty) = orders.get(j).unwrap();
                if other_creator == creator {
                    total_qty_for_creator = total_qty_for_creator
                        .checked_add(other_qty)
                        .ok_or(ContractError::Overflow)?;
                }
            }

            // Liquid balance excludes staked and frozen keys
            if available_holder_balance(&env, &creator, &seller) < total_qty_for_creator {
                return Err(ContractError::InsufficientBalance);
            }

            // Flash-loan guard: reject if sold in same ledger as last buy
            let last_buy_ledger_key = constants::storage::last_buy_ledger(&creator, &seller);
            let last_buy_ledger: Option<u32> = env.storage().persistent().get(&last_buy_ledger_key);
            let current_ledger = env.ledger().sequence();
            if last_buy_ledger == Some(current_ledger) {
                env.events().publish(
                    events::flash_loan_blocked_topics(&seller, &creator),
                    events::FlashLoanBlockedEvent {
                        wallet: seller.clone(),
                        key_id: creator.clone(),
                        ledger: current_ledger,
                    },
                );
                return Err(ContractError::FlashLoanDetected);
            }

            // Anti-flash-trade lockup check
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
                        return Err(ContractError::AllocationLocked);
                    }
                }
            }
        }

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        bump_persistent_ttl(&env, &constants::storage::KEY_PRICE);

        let mut results = soroban_sdk::Vec::new(&env);
        let mut event_order_tuples = soroban_sdk::Vec::new(&env);
        let mut total_proceeds: i128 = 0;

        for order in orders.iter() {
            let (creator, quantity) = order;
            let mut profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

            let mut order_proceeds: i128 = 0;
            let mut i = 0u32;
            while i < quantity {
                let sell_supply = profile
                    .supply
                    .checked_sub(1)
                    .ok_or(ContractError::SellUnderflow)?;
                let price = compute_bonding_curve_price(&env, &creator, base_price, sell_supply)?;

                let balance_key = constants::storage::holder_balance_key(&creator, &seller);
                let current_balance: u32 =
                    env.storage().persistent().get(&balance_key).unwrap_or(0);
                if current_balance == 0 {
                    return Err(ContractError::InsufficientBalance);
                }

                // Settle dividends before balance changes so earnings are captured at old balance.
                settle_holder_dividends(&env, &creator, &seller, current_balance)?;

                let new_balance = current_balance
                    .checked_sub(1)
                    .ok_or(ContractError::SellUnderflow)?;
                profile.supply = sell_supply;

                if new_balance == 0 {
                    profile.holder_count = profile
                        .holder_count
                        .checked_sub(1)
                        .ok_or(ContractError::SellUnderflow)?;
                }

                let key = constants::storage::creator(&creator);
                env.storage().persistent().set(&key, &profile);

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

                if let Some(created_at) = env
                    .storage()
                    .persistent()
                    .get::<DataKey, u32>(&constants::storage::created_at_ledger(&creator))
                {
                    let current_ledger = env.ledger().sequence();
                    if current_ledger.checked_sub(created_at).unwrap_or(u32::MAX)
                        < crate::LAUNCH_PENALTY_WINDOW_LEDGERS
                    {
                        let penalty_bps: u32 = env
                            .storage()
                            .persistent()
                            .get::<DataKey, u32>(&constants::storage::launch_penalty_bps(&creator))
                            .unwrap_or(crate::DEFAULT_LAUNCH_PENALTY_BPS);
                        let capped_bps = penalty_bps.min(crate::MAX_LAUNCH_PENALTY_BPS);
                        if capped_bps > 0 {
                            let penalty_amount =
                                crate::fee::apply_percentage_fee(proceeds, capped_bps).unwrap_or(0);
                            if penalty_amount > 0 {
                                credit_creator_fee_balance(&env, &creator, penalty_amount)?;
                                env.events().publish(
                                    events::launch_penalty_applied_topics(&creator, &seller),
                                    events::LaunchPenaltyAppliedEvent {
                                        creator_id: creator.clone(),
                                        seller: seller.clone(),
                                        penalty_bps: capped_bps,
                                        penalty_amount,
                                        ledger: env.ledger().sequence(),
                                    },
                                );
                            }
                        }
                    }
                }

                order_proceeds = order_proceeds
                    .checked_add(proceeds)
                    .ok_or(ContractError::Overflow)?;

                i += 1;
            }

            extend_creator_ttl(&env, &creator);

            let sell_event_data = events::KeysSoldEvent {
                seller: seller.clone(),
                creator_id: creator.clone(),
                quantity,
                proceeds: order_proceeds,
                new_supply: profile.supply,
                ledger: env.ledger().sequence(),
            };

            env.events().publish(
                (events::SELL_EVENT_NAME, creator.clone(), seller.clone()),
                sell_event_data,
            );

            total_proceeds = total_proceeds
                .checked_add(order_proceeds)
                .ok_or(ContractError::Overflow)?;

            event_order_tuples.push_back((creator.clone(), quantity, order_proceeds));

            results.push_back(BatchSellOrderResult {
                key_id: creator,
                quantity,
                proceeds: order_proceeds,
            });
        }

        env.events().publish(
            events::batch_sell_completed_topics(&seller),
            events::BatchSellCompletedEvent {
                seller: seller.clone(),
                orders: event_order_tuples,
                total_proceeds,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(results)
    }

    /// Set royalty configuration for a creator's keys.
    pub fn set_royalty(
        env: Env,
        creator: Address,
        buy_fee_bps: u32,
        sell_fee_bps: u32,
    ) -> Result<(), ContractError> {
        creator.require_auth();
        assert_not_paused(&env)?;

        if buy_fee_bps > MAX_ROYALTY_BPS || sell_fee_bps > MAX_ROYALTY_BPS {
            return Err(ContractError::ProtocolFeeExceedsCap);
        }

        let _profile: CreatorProfile = read_registered_creator_profile(&env, &creator)?;

        let config = RoyaltyConfig {
            buy_fee_bps,
            sell_fee_bps,
        };

        env.storage()
            .persistent()
            .set(&constants::storage::royalty_config(&creator), &config);

        env.events().publish(
            events::royalty_updated_topics(&creator),
            events::RoyaltyUpdatedEvent {
                creator,
                buy_fee_bps,
                sell_fee_bps,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    /// Read-only view: returns the royalty configuration for a creator.
    pub fn get_royalty_config(env: Env, creator: Address) -> Option<RoyaltyConfig> {
        read_royalty_config(&env, &creator)
    }

    /// Migrate the bonding curve exponent for a set of creators.
    pub fn migrate_curve(
        env: Env,
        admin: Address,
        new_exponent: u32,
        key_ids: Vec<Address>,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        if !(1..=5).contains(&new_exponent) {
            return Err(ContractError::InvalidFeeConfig);
        }

        if key_ids.is_empty() {
            return Err(ContractError::NotPositiveAmount);
        }

        for key_id in key_ids.iter() {
            let _profile: CreatorProfile = read_registered_creator_profile(&env, &key_id)?;

            env.storage()
                .persistent()
                .set(&constants::storage::curve_exponent(&key_id), &new_exponent);
        }

        env.events().publish(
            events::curve_migrated_topics(&admin),
            events::CurveMigratedEvent {
                admin,
                new_exponent,
                key_count: key_ids.len(),
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    /// Read-only view: returns the curve exponent for a creator, if set.
    pub fn get_curve_exponent(env: Env, creator: Address) -> Option<u32> {
        read_curve_exponent(&env, &creator)
    }

    pub fn get_stake_unlock_ledger(env: Env, creator: Address, holder: Address) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&constants::storage::stake_unlock_ledger(&creator, &holder))
    }

    pub fn reinvest_dividend(
        env: Env,
        key_id: Address,
        caller: Address,
    ) -> Result<ReinvestResult, ContractError> {
        caller.require_auth();
        assert_not_paused(&env)?;
        assert_not_blacklisted(&env, &caller)?;
        assert_before_global_deadline(&env)?;

        let claimable = compute_claimable_dividend(&env, &key_id, &caller);
        if claimable <= 0 {
            return Err(ContractError::NoDividendClaimable);
        }

        // Clear the unclaimed dividend balance, matching claim_dividend settlement
        let accumulator = read_dividend_accumulator(&env, &key_id);
        let pending_key = constants::storage::holder_dividend_pending(&key_id, &caller);
        let checkpoint_key = constants::storage::holder_dividend_checkpoint(&key_id, &caller);
        env.storage().persistent().set(&pending_key, &0i128);
        env.storage()
            .persistent()
            .set(&checkpoint_key, &accumulator);
        extend_key_ttl_to_full_window(&env, &pending_key);
        extend_key_ttl_to_full_window(&env, &checkpoint_key);

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        bump_persistent_ttl(&env, &constants::storage::KEY_PRICE);

        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &key_id)?;
        assert_whitelist_allows_buy(&env, &profile, &caller)?;

        let balance_key = constants::storage::holder_balance_key(&key_id, &caller);
        let mut current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        let mut remaining = claimable;
        let mut keys_bought = 0u32;

        loop {
            let next_price =
                compute_bonding_curve_price(&env, &key_id, base_price, profile.supply)?;
            if next_price <= 0 || remaining < next_price {
                break;
            }

            // Check max supply cap if configured
            if let Some(max_supply) = env
                .storage()
                .persistent()
                .get::<DataKey, u32>(&constants::storage::max_supply(&key_id))
            {
                if profile.supply >= max_supply {
                    break;
                }
            }

            // Check max keys per wallet cap if configured
            if let Some(cap) = env
                .storage()
                .persistent()
                .get::<DataKey, u32>(&constants::storage::max_keys_per_wallet(&key_id))
            {
                let post_buy_balance = match current_balance.checked_add(1) {
                    Some(b) => b,
                    None => break,
                };
                if post_buy_balance > cap {
                    break;
                }
            }

            // Check percentage holding cap if configured
            if caller != key_id {
                if let Some(cap_bps) = env
                    .storage()
                    .persistent()
                    .get::<DataKey, u32>(&constants::storage::holder_cap_bps(&key_id))
                {
                    let post_buy_supply = match profile.supply.checked_add(1) {
                        Some(s) => s,
                        None => break,
                    };
                    let post_buy_balance = match current_balance.checked_add(1) {
                        Some(b) => b,
                        None => break,
                    };
                    let max_allowed = ((i128::from(post_buy_supply) * i128::from(cap_bps))
                        / i128::from(fee::BPS_MAX)) as u32;
                    if post_buy_balance > max_allowed {
                        break;
                    }
                }
            }

            remaining = remaining
                .checked_sub(next_price)
                .ok_or(ContractError::Overflow)?;

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

            current_balance = current_balance
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;

            keys_bought = keys_bought.checked_add(1).ok_or(ContractError::Overflow)?;

            // Collect protocol trade fee & fee splits
            let net_amount = collect_protocol_trade_fee(&env, &key_id, next_price)?;
            if let Some(config) = read_protocol_fee_config(&env) {
                let (creator_fee, protocol_fee) = fee::checked_compute_fee_split(
                    net_amount,
                    config.creator_bps,
                    config.protocol_bps,
                )
                .ok_or(ContractError::Overflow)?;

                credit_creator_fee(&env, &key_id, creator_fee)?;
                credit_treasury_balance(&env, protocol_fee)?;
                credit_protocol_fee_recipient_balance(&env, protocol_fee)?;
            }

            if let Some(royalty) = read_royalty_config(&env, &key_id) {
                let royalty_amount = fee::apply_percentage_fee(next_price, royalty.buy_fee_bps)
                    .ok_or(ContractError::Overflow)?;
                if royalty_amount > 0 {
                    credit_creator_fee_recipient_balance(&env, &key_id, royalty_amount)?;
                }
            }

            env.events().publish(
                events::buy_event_topics(&key_id, &caller),
                events::KeysBoughtEvent {
                    buyer: caller.clone(),
                    creator_id: key_id.clone(),
                    quantity: 1,
                    price_paid: next_price,
                    new_supply: profile.supply,
                    ledger: env.ledger().sequence(),
                },
            );
        }

        if keys_bought > 0 {
            let key = constants::storage::creator(&key_id);
            env.storage().persistent().set(&key, &profile);
            write_creator_supply(&env, &key_id, profile.supply);
            env.storage()
                .persistent()
                .set(&balance_key, &current_balance);
            extend_key_ttl_to_full_window(&env, &balance_key);

            let last_buy_key = constants::storage::last_buy_timestamp(&key_id, &caller);
            env.storage()
                .persistent()
                .set(&last_buy_key, &env.ledger().timestamp());
            extend_key_ttl_to_full_window(&env, &last_buy_key);

            extend_creator_ttl(&env, &key_id);
        }

        let remainder_returned = remaining;

        env.events().publish(
            events::dividend_reinvested_topics(&key_id, &caller),
            events::DividendReinvestedEvent {
                wallet: caller,
                key_id,
                keys_bought,
                remainder_returned,
            },
        );

        Ok(ReinvestResult {
            keys_bought,
            remainder_returned,
        })
    }

    /// Read-only view: simulates a buy quote for a given creator and quantity.
    ///
    /// Returns a [`SimulateResponse`] containing the total cost, per-unit price,
    /// creator fee, and protocol fee for purchasing `quantity` keys at the current
    /// supply level.  Uses the same bonding-curve and auction pricing logic as
    /// [`CreatorKeysContract::buy_key`] without writing to any storage entry.
    ///
    /// # Panics
    /// - If `quantity` is 0.
    /// - If the creator is not registered (simulates a 404 / `KeyNotFound`).
    pub fn simulate_buy(env: Env, creator: Address, quantity: u32) -> SimulateResponse {
        if quantity == 0 {
            panic!("quantity must be > 0");
        }

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .expect("KEY_PRICE not set");

        let profile = read_registered_creator_profile(&env, &creator)
            .expect("creator not registered (KeyNotFound)");

        // Check auction-phase pricing
        let auction_config_key = constants::storage::auction_config(&creator);
        let auction_config: Option<AuctionConfig> =
            env.storage().persistent().get(&auction_config_key);
        let in_auction = auction_config
            .as_ref()
            .map(|config| profile.supply < config.auction_supply)
            .unwrap_or(false);

        let mut total_cost: i128 = 0;
        let mut total_creator_fee: i128 = 0;
        let mut total_protocol_fee: i128 = 0;
        let mut current_supply = profile.supply;

        let config = read_required_protocol_fee_config(&env).expect("fee config not set");

        for _ in 0..quantity {
            let price = if in_auction {
                let ac = auction_config.as_ref().unwrap();
                if current_supply < ac.auction_supply {
                    ac.auction_price
                } else {
                    compute_bonding_curve_price(&env, &creator, base_price, current_supply)
                        .expect("bonding curve price overflow")
                }
            } else {
                compute_bonding_curve_price(&env, &creator, base_price, current_supply)
                    .expect("bonding curve price overflow")
            };

            let (creator_fee, protocol_fee) =
                fee::compute_fee_split(price, config.creator_bps, config.protocol_bps);

            total_cost = total_cost
                .checked_add(price)
                .expect("overflow in total_cost");
            total_creator_fee = total_creator_fee
                .checked_add(creator_fee)
                .expect("overflow in creator_fee");
            total_protocol_fee = total_protocol_fee
                .checked_add(protocol_fee)
                .expect("overflow in protocol_fee");

            current_supply = current_supply.checked_add(1).expect("supply overflow");
        }

        let total_with_fees = total_cost
            .checked_add(total_creator_fee)
            .expect("overflow")
            .checked_add(total_protocol_fee)
            .expect("overflow");
        let per_unit_price = total_cost / i128::from(quantity);

        SimulateResponse {
            total_cost: total_with_fees,
            per_unit_price,
            creator_fee: total_creator_fee,
            protocol_fee: total_protocol_fee,
        }
    }

    /// Read-only view: simulates a sell quote for a given creator and quantity.
    ///
    /// Returns a [`SimulateResponse`] containing the total proceeds (after fees),
    /// per-unit price, creator fee, and protocol fee for selling `quantity` keys
    /// at the current supply level.  Uses the same bonding-curve pricing logic as
    /// [`CreatorKeysContract::sell_key`] without writing to any storage entry.
    ///
    /// # Panics
    /// - If `quantity` is 0.
    /// - If the creator is not registered (simulates a 404 / `KeyNotFound`).
    pub fn simulate_sell(env: Env, creator: Address, quantity: u32) -> SimulateResponse {
        if quantity == 0 {
            panic!("quantity must be > 0");
        }

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .expect("KEY_PRICE not set");

        let profile = read_registered_creator_profile(&env, &creator)
            .expect("creator not registered (KeyNotFound)");

        let current_supply = profile.supply;
        if current_supply == 0 {
            panic!("creator has zero supply, cannot sell");
        }

        let config = read_required_protocol_fee_config(&env).expect("fee config not set");

        let mut total_proceeds: i128 = 0;
        let mut total_creator_fee: i128 = 0;
        let mut total_protocol_fee: i128 = 0;

        for i in 0..quantity {
            // Sell prices use sell_supply = supply - 1 for the first key,
            // then supply - 2, etc. (matching sell_key logic)
            let sell_supply = current_supply
                .checked_sub(1)
                .and_then(|s| s.checked_sub(i))
                .expect("not enough supply to sell");

            let price = compute_bonding_curve_price(&env, &creator, base_price, sell_supply)
                .expect("bonding curve price overflow");

            let (creator_fee, protocol_fee) =
                fee::compute_fee_split(price, config.creator_bps, config.protocol_bps);

            let net_price = price
                .checked_sub(creator_fee)
                .expect("sell underflow")
                .checked_sub(protocol_fee)
                .expect("sell underflow");

            total_proceeds = total_proceeds
                .checked_add(net_price)
                .expect("overflow in total_proceeds");
            total_creator_fee = total_creator_fee
                .checked_add(creator_fee)
                .expect("overflow in creator_fee");
            total_protocol_fee = total_protocol_fee
                .checked_add(protocol_fee)
                .expect("overflow in protocol_fee");
        }

        let per_unit_price = total_proceeds / i128::from(quantity);

        SimulateResponse {
            total_cost: total_proceeds,
            per_unit_price,
            creator_fee: total_creator_fee,
            protocol_fee: total_protocol_fee,
        }
    }

    /// Price oracle: returns the current bonding-curve price for a creator's key.
    ///
    /// Only callers present in the admin-maintained approve allowlist may call
    /// this entrypoint; unapproved callers are rejected with
    /// [`ContractError::CallerNotApproved`]. `caller` must authorize the call
    /// (`require_auth`). The returned value is the price of the next key on the
    /// bonding curve for the creator's current supply, computed with the same
    /// [`compute_bonding_curve_price`] logic used by `buy_key` / `get_buy_quote`.
    ///
    /// Each call records the observation used for TWAP and emits a
    /// [`events::PriceQueriedEvent`] with the calling contract's address.
    ///
    /// # Errors
    ///
    /// - [`ContractError::CallerNotApproved`] if the caller is not in the allowlist.
    /// - [`ContractError::KeyPriceNotSet`] if no key price is configured.
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    pub fn get_price(env: Env, creator: Address, caller: Address) -> Result<i128, ContractError> {
        caller.require_auth();
        assert_caller_approved(&env, &caller)?;

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;

        let profile = read_registered_creator_profile(&env, &creator)?;
        let price = compute_bonding_curve_price(&env, &creator, base_price, profile.supply)?;

        record_price_observation(&env, &creator, price);
        emit_price_queried(&env, &caller, &creator, price);

        Ok(price)
    }

    /// Price oracle: returns the time-weighted average price for a creator's key
    /// over `window_ledgers` ledgers ending at the current ledger.
    ///
    /// Only callers present in the admin-maintained allowlist may call this
    /// entrypoint; unapproved callers are rejected with
    /// [`ContractError::CallerNotApproved`]. `caller` must authorize the call
    /// (`require_auth`). The TWAP is derived from the observations recorded by
    /// [`CreatorKeysContract::get_price`] and
    /// [`CreatorKeysContract::get_twap_price`]: each observed price is weighted
    /// by the number of ledgers it was in effect within the window. When no
    /// observation is available in the window, the current bonding-curve price
    /// is returned.
    ///
    /// Each call records the observation used for TWAP and emits a
    /// [`events::PriceQueriedEvent`] with the calling contract's address.
    ///
    /// # Errors
    ///
    /// - [`ContractError::CallerNotApproved`] if the caller is not in the allowlist.
    /// - [`ContractError::KeyPriceNotSet`] if no key price is configured.
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    pub fn get_twap_price(
        env: Env,
        creator: Address,
        window_ledgers: u32,
        caller: Address,
    ) -> Result<i128, ContractError> {
        caller.require_auth();
        assert_caller_approved(&env, &caller)?;

        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;

        let profile = read_registered_creator_profile(&env, &creator)?;
        let current_price =
            compute_bonding_curve_price(&env, &creator, base_price, profile.supply)?;

        record_price_observation(&env, &creator, current_price);
        let twap = compute_twap_price(&env, &creator, current_price, window_ledgers);

        emit_price_queried(&env, &caller, &creator, twap);

        Ok(twap)
    }

    /// Sets the age, in ledgers, after which price snapshots are pruned on the
    /// next trade (admin-only). `0` disables age-based pruning.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    pub fn set_price_retention(
        env: Env,
        admin: Address,
        retention_ledgers: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        env.storage().persistent().set(
            &constants::storage::PRICE_RETENTION_LEDGERS,
            &retention_ledgers,
        );
        extend_key_ttl_to_full_window(&env, &constants::storage::PRICE_RETENTION_LEDGERS);
        Ok(())
    }

    /// Read-only view: the configured price snapshot retention age in ledgers
    /// (`0` when age-based pruning is disabled).
    pub fn get_price_retention(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&constants::storage::PRICE_RETENTION_LEDGERS)
            .unwrap_or(0)
    }

    /// Read-only view: number of price snapshots currently stored for `creator`.
    pub fn get_price_snapshot_count(env: Env, creator: Address) -> u32 {
        read_price_history(&env, &creator).len()
    }

    /// Adds `caller` to the price-oracle approve allowlist (admin-only).
    ///
    /// Only the protocol admin may call this. Re-approving an already-approved
    /// caller is a no-op.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    pub fn add_approved_caller(
        env: Env,
        admin: Address,
        caller: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        let mut callers = read_approved_callers(&env);
        if callers.iter().any(|c| c == caller) {
            return Ok(());
        }
        callers.push_back(caller);
        env.storage()
            .persistent()
            .set(&constants::storage::APPROVED_CALLERS, &callers);
        Ok(())
    }

    /// Removes `caller` from the price-oracle approve allowlist (admin-only).
    ///
    /// Only the protocol admin may call this. Removing a caller that is not in
    /// the allowlist is a no-op.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    pub fn remove_approved_caller(
        env: Env,
        admin: Address,
        caller: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        let callers = read_approved_callers(&env);
        let before = callers.len();
        let mut retained = Vec::new(&env);
        for c in callers.iter() {
            if c != caller {
                retained.push_back(c);
            }
        }
        if retained.len() == before {
            return Ok(());
        }
        env.storage()
            .persistent()
            .set(&constants::storage::APPROVED_CALLERS, &retained);
        Ok(())
    }

    /// Read-only view: returns whether `caller` is in the price-oracle approve
    /// allowlist.
    pub fn is_approved_caller(env: Env, caller: Address) -> bool {
        is_caller_approved(&env, &caller)
    }

    // =========================================================================
    // #905 — External price oracle feed
    // =========================================================================

    /// Sets the address authorised to publish oracle prices (admin-only).
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    pub fn set_oracle_address(
        env: Env,
        admin: Address,
        oracle: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        env.storage()
            .persistent()
            .set(&constants::storage::ORACLE_ADDRESS, &oracle);
        extend_key_ttl_to_full_window(&env, &constants::storage::ORACLE_ADDRESS);
        Ok(())
    }

    /// Read-only view: returns the authorised oracle address, if configured.
    pub fn get_oracle_address(env: Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&constants::storage::ORACLE_ADDRESS)
    }

    /// Sets the age in seconds after which the oracle price is stale (admin-only).
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    /// - [`ContractError::NotPositiveAmount`] if `threshold_secs` is zero.
    pub fn set_oracle_staleness_threshold(
        env: Env,
        admin: Address,
        threshold_secs: u64,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        if threshold_secs == 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        env.storage()
            .persistent()
            .set(&constants::storage::ORACLE_STALENESS_SECS, &threshold_secs);
        extend_key_ttl_to_full_window(&env, &constants::storage::ORACLE_STALENESS_SECS);
        Ok(())
    }

    /// Publishes a new oracle price. Only the authorised oracle address may call this.
    ///
    /// The price is stored with the current ledger timestamp so `get_oracle_price`
    /// can flag it as stale, and an [`events::OraclePriceUpdatedEvent`] is emitted.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if no oracle is configured or `oracle`
    ///   is not the configured oracle address.
    /// - [`ContractError::NotPositiveAmount`] if `price` is not positive.
    pub fn set_oracle_price(env: Env, oracle: Address, price: i128) -> Result<(), ContractError> {
        oracle.require_auth();

        let configured: Address = env
            .storage()
            .persistent()
            .get(&constants::storage::ORACLE_ADDRESS)
            .ok_or(ContractError::Unauthorized)?;
        if oracle != configured {
            return Err(ContractError::Unauthorized);
        }
        if price <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let timestamp = env.ledger().timestamp();
        env.storage().persistent().set(
            &constants::storage::ORACLE_PRICE,
            &OraclePrice {
                price,
                updated_at: timestamp,
            },
        );
        extend_key_ttl_to_full_window(&env, &constants::storage::ORACLE_PRICE);

        env.events().publish(
            events::oracle_price_updated_topics(&oracle),
            events::OraclePriceUpdatedEvent {
                oracle: oracle.clone(),
                price,
                timestamp,
            },
        );

        Ok(())
    }

    /// Read-only view: returns the latest oracle price, its publish timestamp and
    /// whether it is older than the configured staleness threshold.
    ///
    /// # Errors
    ///
    /// - [`ContractError::OraclePriceNotSet`] if no price has been published.
    pub fn get_oracle_price(env: Env) -> Result<OraclePriceView, ContractError> {
        let stored: OraclePrice = env
            .storage()
            .persistent()
            .get(&constants::storage::ORACLE_PRICE)
            .ok_or(ContractError::OraclePriceNotSet)?;
        let threshold: u64 = env
            .storage()
            .persistent()
            .get(&constants::storage::ORACLE_STALENESS_SECS)
            .unwrap_or(DEFAULT_ORACLE_STALENESS_SECS);
        let age = env.ledger().timestamp().saturating_sub(stored.updated_at);

        Ok(OraclePriceView {
            price: stored.price,
            updated_at: stored.updated_at,
            is_stale: age > threshold,
        })
    }

    // =========================================================================
    // #904 — Time-locked admin actions
    // =========================================================================

    /// Sets the delay in seconds applied to newly proposed actions (admin-only).
    ///
    /// Actions already proposed keep the execution timestamp they were given.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    /// - [`ContractError::InvalidTimelockDelay`] if `delay_secs` is zero or above 30 days.
    pub fn set_timelock_delay(
        env: Env,
        admin: Address,
        delay_secs: u64,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        if delay_secs == 0 || delay_secs > MAX_TIMELOCK_DELAY_SECS {
            return Err(ContractError::InvalidTimelockDelay);
        }
        env.storage()
            .persistent()
            .set(&constants::storage::TIMELOCK_DELAY_SECS, &delay_secs);
        extend_key_ttl_to_full_window(&env, &constants::storage::TIMELOCK_DELAY_SECS);
        Ok(())
    }

    /// Read-only view: returns the delay in seconds applied to new actions.
    pub fn get_timelock_delay(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&constants::storage::TIMELOCK_DELAY_SECS)
            .unwrap_or(DEFAULT_TIMELOCK_DELAY_SECS)
    }

    /// Proposes an admin action that cannot execute until the timelock delay has
    /// elapsed, and returns its action id.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    /// - [`ContractError::Overflow`] on arithmetic overflow.
    pub fn propose_action(
        env: Env,
        admin: Address,
        change_type: TimelockChangeType,
        payload: soroban_sdk::Bytes,
    ) -> Result<u32, ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        let action_id: u32 = env
            .storage()
            .persistent()
            .get(&constants::storage::ACTION_NEXT_ID)
            .unwrap_or(1u32);
        let next_id = action_id.checked_add(1).ok_or(ContractError::Overflow)?;

        let proposed_at = env.ledger().timestamp();
        let execution_not_before = proposed_at
            .checked_add(Self::get_timelock_delay(env.clone()))
            .ok_or(ContractError::Overflow)?;

        let action_key = constants::storage::action_proposal(action_id);
        env.storage().persistent().set(
            &action_key,
            &TimelockAction {
                change_type,
                payload,
                proposer: admin.clone(),
                proposed_at,
                execution_not_before,
                executed: false,
                cancelled: false,
            },
        );
        env.storage()
            .persistent()
            .set(&constants::storage::ACTION_NEXT_ID, &next_id);
        extend_key_ttl_to_full_window(&env, &action_key);
        extend_key_ttl_to_full_window(&env, &constants::storage::ACTION_NEXT_ID);

        env.events().publish(
            events::action_proposed_topics(action_id),
            events::ActionProposedEvent {
                action_id,
                proposer: admin,
                change_type: change_type as u32,
                proposed_at,
                execution_not_before,
            },
        );

        Ok(action_id)
    }

    /// Executes a proposed action once its execution timestamp has been reached.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    /// - [`ContractError::ProposalNotFound`] if `action_id` does not exist.
    /// - [`ContractError::ActionNotPending`] if the action was already executed or cancelled.
    /// - [`ContractError::TimelockNotElapsed`] if the delay has not yet elapsed.
    pub fn execute_action(env: Env, admin: Address, action_id: u32) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        let action_key = constants::storage::action_proposal(action_id);
        let mut action: TimelockAction = env
            .storage()
            .persistent()
            .get(&action_key)
            .ok_or(ContractError::ProposalNotFound)?;
        if action.executed || action.cancelled {
            return Err(ContractError::ActionNotPending);
        }

        let now = env.ledger().timestamp();
        if now < action.execution_not_before {
            return Err(ContractError::TimelockNotElapsed);
        }

        action.executed = true;
        env.storage().persistent().set(&action_key, &action);
        extend_key_ttl_to_full_window(&env, &action_key);

        env.events().publish(
            events::action_executed_topics(action_id),
            events::ActionExecutedEvent {
                action_id,
                executed_at: now,
            },
        );

        Ok(())
    }

    /// Cancels a pending action so it can no longer be executed (admin-only).
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    /// - [`ContractError::ProposalNotFound`] if `action_id` does not exist.
    /// - [`ContractError::ActionNotPending`] if the action was already executed or cancelled.
    pub fn cancel_action(env: Env, admin: Address, action_id: u32) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;

        let action_key = constants::storage::action_proposal(action_id);
        let mut action: TimelockAction = env
            .storage()
            .persistent()
            .get(&action_key)
            .ok_or(ContractError::ProposalNotFound)?;
        if action.executed || action.cancelled {
            return Err(ContractError::ActionNotPending);
        }

        action.cancelled = true;
        env.storage().persistent().set(&action_key, &action);
        extend_key_ttl_to_full_window(&env, &action_key);

        env.events().publish(
            events::action_cancelled_topics(action_id),
            events::ActionCancelledEvent {
                action_id,
                cancelled_at: env.ledger().timestamp(),
            },
        );

        Ok(())
    }

    /// Read-only view: returns a timelocked action by id.
    pub fn get_action(env: Env, action_id: u32) -> Option<TimelockAction> {
        env.storage()
            .persistent()
            .get(&constants::storage::action_proposal(action_id))
    }

    // =========================================================================
    // #908 — Multi-key staking vault
    // =========================================================================

    /// Deposits keys from several creators into the holder's vault position.
    ///
    /// `creator_ids[i]` and `amounts[i]` describe one deposit. Deposited keys are
    /// booked as staked (so they cannot be sold) and credited 1:1 as vault shares,
    /// which entitle the holder to a pro-rata share of vault rewards. Emits a
    /// [`events::VaultDepositEvent`] per creator.
    ///
    /// # Errors
    ///
    /// - [`ContractError::InvalidVaultInput`] if the vectors differ in length.
    /// - [`ContractError::BatchSizeExceeded`] if empty or more than 10 entries.
    /// - [`ContractError::NotPositiveAmount`] if an amount is zero.
    /// - [`ContractError::NotRegistered`] if a creator is not registered.
    /// - [`ContractError::InsufficientBalance`] if liquid keys are below the amount.
    pub fn vault_deposit(
        env: Env,
        holder: Address,
        creator_ids: soroban_sdk::Vec<Address>,
        amounts: soroban_sdk::Vec<u32>,
    ) -> Result<(), ContractError> {
        holder.require_auth();
        assert_not_paused(&env)?;

        if creator_ids.len() != amounts.len() {
            return Err(ContractError::InvalidVaultInput);
        }
        if creator_ids.is_empty() || creator_ids.len() > VAULT_MAX_BATCH {
            return Err(ContractError::BatchSizeExceeded);
        }

        for (creator, amount) in creator_ids.iter().zip(amounts.iter()) {
            if amount == 0 {
                return Err(ContractError::NotPositiveAmount);
            }
            read_registered_creator_profile(&env, &creator)?;

            let total_balance: u32 = env
                .storage()
                .persistent()
                .get(&constants::storage::key_balance(&creator, &holder))
                .unwrap_or(0);
            let staked = Self::get_staked_balance(env.clone(), creator.clone(), holder.clone());
            if total_balance.saturating_sub(staked) < amount {
                return Err(ContractError::InsufficientBalance);
            }

            let holder_shares = read_vault_shares(&env, &creator, &holder);
            settle_vault_rewards(&env, &creator, &holder, holder_shares)?;

            let new_holder_shares = holder_shares
                .checked_add(amount)
                .ok_or(ContractError::Overflow)?;
            let new_total_shares = read_vault_total_shares(&env, &creator)
                .checked_add(amount)
                .ok_or(ContractError::Overflow)?;
            let new_staked = staked.checked_add(amount).ok_or(ContractError::Overflow)?;
            write_vault_position(
                &env,
                &creator,
                &holder,
                new_holder_shares,
                new_total_shares,
                new_staked,
            );

            env.events().publish(
                events::vault_deposit_topics(&creator, &holder),
                events::VaultDepositEvent {
                    creator_id: creator.clone(),
                    holder: holder.clone(),
                    amount,
                    holder_shares: new_holder_shares,
                    total_shares: new_total_shares,
                    ledger: env.ledger().sequence(),
                },
            );
        }

        Ok(())
    }

    /// Withdraws keys from the holder's vault position back to liquid balance.
    ///
    /// `creator_ids[i]` and `amounts[i]` describe one withdrawal, so partial and
    /// full withdrawals are both supported. Rewards earned up to now stay claimable
    /// via `claim_vault_rewards`. Emits a [`events::VaultWithdrawEvent`] per creator.
    ///
    /// # Errors
    ///
    /// - [`ContractError::InvalidVaultInput`] if the vectors differ in length.
    /// - [`ContractError::BatchSizeExceeded`] if empty or more than 10 entries.
    /// - [`ContractError::NotPositiveAmount`] if an amount is zero.
    /// - [`ContractError::InsufficientBalance`] if the amount exceeds the holder's vault shares.
    pub fn vault_withdraw(
        env: Env,
        holder: Address,
        creator_ids: soroban_sdk::Vec<Address>,
        amounts: soroban_sdk::Vec<u32>,
    ) -> Result<(), ContractError> {
        holder.require_auth();
        assert_not_paused(&env)?;

        if creator_ids.len() != amounts.len() {
            return Err(ContractError::InvalidVaultInput);
        }
        if creator_ids.is_empty() || creator_ids.len() > VAULT_MAX_BATCH {
            return Err(ContractError::BatchSizeExceeded);
        }

        for (creator, amount) in creator_ids.iter().zip(amounts.iter()) {
            if amount == 0 {
                return Err(ContractError::NotPositiveAmount);
            }

            let holder_shares = read_vault_shares(&env, &creator, &holder);
            if holder_shares < amount {
                return Err(ContractError::InsufficientBalance);
            }
            settle_vault_rewards(&env, &creator, &holder, holder_shares)?;

            let new_holder_shares = holder_shares - amount;
            let new_total_shares = read_vault_total_shares(&env, &creator).saturating_sub(amount);
            let staked = Self::get_staked_balance(env.clone(), creator.clone(), holder.clone());
            write_vault_position(
                &env,
                &creator,
                &holder,
                new_holder_shares,
                new_total_shares,
                staked.saturating_sub(amount),
            );

            env.events().publish(
                events::vault_withdraw_topics(&creator, &holder),
                events::VaultWithdrawEvent {
                    creator_id: creator.clone(),
                    holder: holder.clone(),
                    amount,
                    holder_shares: new_holder_shares,
                    total_shares: new_total_shares,
                    ledger: env.ledger().sequence(),
                },
            );
        }

        Ok(())
    }

    /// Distributes `amount` as rewards pro-rata across all vault depositors of `creator`.
    ///
    /// Like `distribute_dividend`, this records accounting only; dust from the
    /// integer division is lost.
    ///
    /// # Errors
    ///
    /// - [`ContractError::ZeroDistributionAmount`] if `amount` is not positive.
    /// - [`ContractError::NoKeyHolders`] if the vault holds no keys for `creator`.
    pub fn distribute_vault_rewards(
        env: Env,
        distributor: Address,
        creator: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        distributor.require_auth();
        assert_not_paused(&env)?;

        if amount <= 0 {
            return Err(ContractError::ZeroDistributionAmount);
        }
        let total_shares = read_vault_total_shares(&env, &creator);
        if total_shares == 0 {
            return Err(ContractError::NoKeyHolders);
        }

        let acc_key = constants::storage::vault_reward_acc(&creator);
        let acc: i128 = env.storage().persistent().get(&acc_key).unwrap_or(0);
        let per_share = amount
            .checked_mul(VAULT_REWARD_PRECISION)
            .ok_or(ContractError::Overflow)?
            / i128::from(total_shares);
        let new_acc = acc.checked_add(per_share).ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&acc_key, &new_acc);
        extend_key_ttl_to_full_window(&env, &acc_key);
        Ok(())
    }

    /// Read-only view: returns the vault rewards `holder` can currently claim.
    pub fn get_vault_pending_rewards(env: Env, creator: Address, holder: Address) -> i128 {
        let acc: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::vault_reward_acc(&creator))
            .unwrap_or(0);
        let checkpoint: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::vault_reward_checkpoint(
                &creator, &holder,
            ))
            .unwrap_or(0);
        let pending: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::vault_reward_pending(&creator, &holder))
            .unwrap_or(0);
        let shares = read_vault_shares(&env, &creator, &holder);
        pending + i128::from(shares) * (acc - checkpoint) / VAULT_REWARD_PRECISION
    }

    /// Claims all vault rewards accrued by `holder` on `creator`'s keys and
    /// returns the claimed amount.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NoDividendClaimable`] if nothing is claimable.
    pub fn claim_vault_rewards(
        env: Env,
        creator: Address,
        holder: Address,
    ) -> Result<i128, ContractError> {
        holder.require_auth();
        assert_not_paused(&env)?;

        let shares = read_vault_shares(&env, &creator, &holder);
        settle_vault_rewards(&env, &creator, &holder, shares)?;

        let pending_key = constants::storage::vault_reward_pending(&creator, &holder);
        let claimable: i128 = env.storage().persistent().get(&pending_key).unwrap_or(0);
        if claimable == 0 {
            return Err(ContractError::NoDividendClaimable);
        }
        env.storage().persistent().set(&pending_key, &0i128);
        Ok(claimable)
    }

    /// Read-only view: returns the keys `holder` has deposited in the vault for `creator`.
    pub fn get_vault_share(env: Env, creator: Address, holder: Address) -> u32 {
        read_vault_shares(&env, &creator, &holder)
    }

    /// Read-only view: returns the total keys deposited in the vault for `creator`.
    pub fn get_vault_total_shares(env: Env, creator: Address) -> u32 {
        read_vault_total_shares(&env, &creator)
    }

    /// Read-only aggregate view: returns all key-level stats for a registered creator
    /// in a single call, reducing the number of RPC round trips needed by server sync
    /// and admin snapshot endpoints.
    ///
    /// # Behaviour
    ///
    /// - Bumps the TTL of every persistent entry it reads so active creator state
    ///   never expires while it is being queried.
    /// - Never panics regardless of which optional fields are unset.
    /// - Returns `Err(ContractError::NotRegistered)` for unknown `key_id` values
    ///   (the 404-equivalent for view callers).
    ///
    /// # Auction fields
    ///
    /// `auction_price`, `auction_supply`, and `auction_sold` are non-zero only when a
    /// pre-launch auction is active. `has_auction` signals to callers whether to
    /// surface those fields.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotRegistered`] if `key_id` has not been registered.
    pub fn get_key_stats(env: Env, key_id: Address) -> Result<KeyStatsView, ContractError> {
        // Require registration — this is the KeyNotFound guard.
        let profile = read_registered_creator_profile(&env, &key_id)?;

        // ── Creator profile key ────────────────────────────────────────────
        let creator_key = constants::storage::creator(&key_id);
        bump_persistent_ttl(&env, &creator_key);

        // ── Current bonding-curve price ────────────────────────────────────
        // Read the global base price; fall back to 0 when unset so we never panic.
        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .unwrap_or(0);
        bump_persistent_ttl(&env, &constants::storage::KEY_PRICE);

        let current_price = if base_price > 0 {
            // Try auction price first; only fall through to curve when not in auction.
            let auction_cfg: Option<AuctionConfig> = env
                .storage()
                .persistent()
                .get(&constants::storage::auction_config(&key_id));
            if let Some(ref cfg) = auction_cfg {
                if profile.supply < cfg.auction_supply {
                    cfg.auction_price
                } else {
                    compute_bonding_curve_price(&env, &key_id, base_price, profile.supply)
                        .unwrap_or(base_price)
                }
            } else {
                compute_bonding_curve_price(&env, &key_id, base_price, profile.supply)
                    .unwrap_or(base_price)
            }
        } else {
            0
        };

        // ── Supply cap ─────────────────────────────────────────────────────
        let supply_cap_key = constants::storage::max_supply(&key_id);
        let supply_cap: u32 = env.storage().persistent().get(&supply_cap_key).unwrap_or(0);
        if supply_cap > 0 {
            bump_persistent_ttl(&env, &supply_cap_key);
        }

        // ── Holder cap bps ─────────────────────────────────────────────────
        let holder_cap_key = constants::storage::holder_cap_bps(&key_id);
        let holder_cap_bps: u32 = env.storage().persistent().get(&holder_cap_key).unwrap_or(0);
        if holder_cap_bps > 0 {
            bump_persistent_ttl(&env, &holder_cap_key);
        }

        // ── Circuit-breaker threshold ──────────────────────────────────────
        // Default is 30 when the key has never been written to storage.
        // Only bump TTL when the entry actually exists to avoid a MissingValue panic.
        let circuit_breaker_threshold_bps: u32 = env
            .storage()
            .persistent()
            .get(&constants::storage::CIRCUIT_BREAKER_THRESHOLD)
            .unwrap_or(30);
        if env
            .storage()
            .persistent()
            .has(&constants::storage::CIRCUIT_BREAKER_THRESHOLD)
        {
            bump_persistent_ttl(&env, &constants::storage::CIRCUIT_BREAKER_THRESHOLD);
        }

        // ── Sell lockup duration ───────────────────────────────────────────
        let lockup_duration_seconds: u64 = env
            .storage()
            .persistent()
            .get(&constants::storage::LOCKUP_DURATION_SECS)
            .unwrap_or(0);
        if lockup_duration_seconds > 0 {
            bump_persistent_ttl(&env, &constants::storage::LOCKUP_DURATION_SECS);
        }

        // ── Launch penalty ─────────────────────────────────────────────────
        let launch_penalty_key = constants::storage::launch_penalty_bps(&key_id);
        let launch_penalty_bps: u32 = env
            .storage()
            .persistent()
            .get(&launch_penalty_key)
            .unwrap_or(0);
        if launch_penalty_bps > 0 {
            bump_persistent_ttl(&env, &launch_penalty_key);
        }

        // ── Buy cooldown ───────────────────────────────────────────────────
        let buy_cooldown_key = constants::storage::buy_cooldown(&key_id);
        let buy_cooldown_ledgers: u32 = env
            .storage()
            .persistent()
            .get(&buy_cooldown_key)
            .unwrap_or(0);
        if buy_cooldown_ledgers > 0 {
            bump_persistent_ttl(&env, &buy_cooldown_key);
        }

        // ── Max buy quantity ───────────────────────────────────────────────
        let max_buy_qty_key = constants::storage::max_buy_quantity(&key_id);
        let max_buy_quantity: u32 = env
            .storage()
            .persistent()
            .get(&max_buy_qty_key)
            .unwrap_or(0);
        if max_buy_quantity > 0 {
            bump_persistent_ttl(&env, &max_buy_qty_key);
        }

        // ── Auction config ─────────────────────────────────────────────────
        let auction_key = constants::storage::auction_config(&key_id);
        let auction_cfg: Option<AuctionConfig> = env.storage().persistent().get(&auction_key);
        let (has_auction, auction_price, auction_supply, auction_sold) =
            if let Some(ref cfg) = auction_cfg {
                bump_persistent_ttl(&env, &auction_key);
                (
                    true,
                    cfg.auction_price,
                    cfg.auction_supply,
                    cfg.auction_sold,
                )
            } else {
                (false, 0, 0, 0)
            };

        // ── Trading paused (global OR per-key) ────────────────────────────
        // Both flags default to `false` when absent; only bump TTL when the entry
        // exists so we do not panic with MissingValue on a fresh deployment.
        let trading_paused = is_paused(&env) || is_global_trading_paused(&env);
        if env.storage().persistent().has(&constants::storage::PAUSED) {
            bump_persistent_ttl(&env, &constants::storage::PAUSED);
        }
        if env
            .storage()
            .persistent()
            .has(&constants::storage::GLOBAL_TRADING_PAUSED)
        {
            bump_persistent_ttl(&env, &constants::storage::GLOBAL_TRADING_PAUSED);
        }

        Ok(KeyStatsView {
            current_price,
            circulating_supply: profile.supply,
            holder_count: profile.holder_count,
            trading_paused,
            supply_cap,
            holder_cap_bps,
            circuit_breaker_threshold_bps,
            lockup_duration_seconds,
            launch_penalty_bps,
            buy_cooldown_ledgers,
            max_buy_quantity,
            has_auction,
            auction_price,
            auction_supply,
            auction_sold,
        })
    }

    // -----------------------------------------------------------------------
    // Feature: Reward Pool Top-Up (#5)
    // -----------------------------------------------------------------------

    /// Sets the authorised fee router address.
    ///
    /// Only the protocol admin may call this. The fee router is the only address
    /// permitted to call [`topup_reward_pool`].
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    /// - [`ContractError::ZeroAddress`] if `router` is the Stellar zero address.
    pub fn set_fee_router(env: Env, admin: Address, router: Address) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        validate_non_zero_address(&env, &router)?;
        env.storage()
            .persistent()
            .set(&constants::storage::fee_router(), &router);
        Ok(())
    }

    /// Read-only view: returns the current fee router address.
    ///
    /// Returns `None` when no fee router has been configured.
    pub fn get_fee_router(env: Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&constants::storage::fee_router())
    }

    /// Adds `amount` stroops to the staker reward pool.
    ///
    /// Only the authorised fee router (set via [`set_fee_router`]) may call this.
    /// The caller must `require_auth` via Soroban's auth framework.
    ///
    /// # Errors
    /// - [`ContractError::FeeRouterNotSet`] if no fee router has been configured.
    /// - [`ContractError::Unauthorized`] if `sender` is not the configured fee router.
    /// - [`ContractError::NotPositiveAmount`] if `amount` is zero or negative.
    /// - [`ContractError::Overflow`] if adding `amount` would overflow the pool balance.
    pub fn topup_reward_pool(
        env: Env,
        sender: Address,
        amount: i128,
    ) -> Result<i128, ContractError> {
        sender.require_auth();

        let router: Address = env
            .storage()
            .persistent()
            .get(&constants::storage::fee_router())
            .ok_or(ContractError::FeeRouterNotSet)?;

        if sender != router {
            return Err(ContractError::Unauthorized);
        }

        if amount <= 0 {
            return Err(ContractError::NotPositiveAmount);
        }

        let current: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::reward_pool_balance())
            .unwrap_or(0);

        let new_balance = current.checked_add(amount).ok_or(ContractError::Overflow)?;

        env.storage()
            .persistent()
            .set(&constants::storage::reward_pool_balance(), &new_balance);

        env.events().publish(
            events::reward_pool_topup_topics(&sender),
            events::RewardPoolTopUpEvent {
                sender,
                amount,
                new_pool_balance: new_balance,
            },
        );

        Ok(new_balance)
    }

    /// Read-only view: returns the current staker reward pool balance.
    ///
    /// Returns `0` before any top-up has been made.
    pub fn get_reward_pool_balance(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&constants::storage::reward_pool_balance())
            .unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // Feature: Bid-Ask Spread (#6)
    // -----------------------------------------------------------------------

    /// Sets the bid-ask spread for a creator's bonding curve.
    ///
    /// The spread reduces the sell price relative to the buy price:
    /// `sell_price = buy_price - (buy_price * spread_bps / 10_000)`.
    /// A spread of zero means buy price equals sell price.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `admin` is not the protocol admin.
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    /// - [`ContractError::SpreadExceedsMax`] if `spread_bps > MAX_SPREAD_BPS`.
    pub fn set_spread_bps(
        env: Env,
        admin: Address,
        creator: Address,
        spread_bps: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        read_registered_creator_profile(&env, &creator)?;

        if spread_bps > MAX_SPREAD_BPS {
            return Err(ContractError::SpreadExceedsMax);
        }

        let old_spread_bps: u32 = env
            .storage()
            .persistent()
            .get(&constants::storage::spread_bps(&creator))
            .unwrap_or(0);

        env.storage()
            .persistent()
            .set(&constants::storage::spread_bps(&creator), &spread_bps);

        env.events().publish(
            events::spread_updated_topics(&creator),
            events::SpreadUpdatedEvent {
                creator,
                old_spread_bps,
                new_spread_bps: spread_bps,
            },
        );

        Ok(())
    }

    /// Read-only view: returns the configured spread in basis points for a creator.
    ///
    /// Returns `0` when no spread has been set (buy price equals sell price).
    pub fn get_spread_bps(env: Env, creator: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&constants::storage::spread_bps(&creator))
            .unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // Feature: Read-only view functions (#7)
    // -----------------------------------------------------------------------

    /// Read-only view: returns the current circulating supply for a key.
    ///
    /// # Errors
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    pub fn get_supply(env: Env, creator: Address) -> Result<u32, ContractError> {
        let profile = read_registered_creator_profile(&env, &creator)?;
        Ok(profile.supply)
    }

    /// Read-only view: returns the current buy and sell price for a creator's key,
    /// with the configured bid-ask spread applied to the sell price.
    ///
    /// `buy_price` is the raw bonding-curve price (before fees).
    /// `sell_price` is `buy_price` reduced by the configured spread.
    /// Both are expressed in XLM stroops.
    ///
    /// # Errors
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    /// - [`ContractError::KeyPriceNotSet`] if no key price has been configured.
    pub fn get_bid_ask_price(env: Env, creator: Address) -> Result<(i128, i128), ContractError> {
        let base_price: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::KEY_PRICE)
            .ok_or(ContractError::KeyPriceNotSet)?;
        let profile = read_registered_creator_profile(&env, &creator)?;
        let buy_price = compute_bonding_curve_price(&env, &creator, base_price, profile.supply)?;
        let sell_price = apply_spread(&env, &creator, buy_price)?;
        Ok((buy_price, sell_price))
    }

    /// Read-only view: returns the unique holder count for a creator's key.
    ///
    /// # Errors
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    pub fn get_holder_count(env: Env, creator: Address) -> Result<u32, ContractError> {
        let profile = read_registered_creator_profile(&env, &creator)?;
        Ok(profile.holder_count)
    }

    /// Read-only view: returns the cumulative trade volume in XLM stroops for a creator.
    ///
    /// Returns `0` when no volume has been recorded yet.
    ///
    /// # Errors
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    pub fn get_volume(env: Env, creator: Address) -> Result<i128, ContractError> {
        read_registered_creator_profile(&env, &creator)?;
        Ok(env
            .storage()
            .persistent()
            .get::<DataKey, i128>(&constants::storage::creator_volume(&creator))
            .unwrap_or(0))
    }

    /// Read-only view: aggregates the market-facing state of a creator's key in a
    /// single call — supply, holder count, spread-aware buy/sell prices and volume.
    ///
    /// Distinct from [`KeyStatsView`], which reports creator-configured limits
    /// (caps, cooldowns, auction terms) rather than live market data.
    ///
    /// # Errors
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    /// - [`ContractError::KeyPriceNotSet`] if no key price has been configured.
    pub fn get_key_market_stats(
        env: Env,
        creator: Address,
    ) -> Result<KeyMarketStatsView, ContractError> {
        let (buy_price, sell_price) = Self::get_bid_ask_price(env.clone(), creator.clone())?;
        let supply = Self::get_supply(env.clone(), creator.clone())?;
        let holder_count = Self::get_holder_count(env.clone(), creator.clone())?;
        let volume = Self::get_volume(env, creator.clone())?;
        Ok(KeyMarketStatsView {
            creator,
            supply,
            buy_price,
            sell_price,
            holder_count,
            volume,
        })
    }

    // -----------------------------------------------------------------------
    // Feature: Analytics accumulators (#8)
    // -----------------------------------------------------------------------

    /// Read-only view: returns aggregated trade analytics for a creator.
    ///
    /// Returns trade count, unique trader count, and total volume. All values
    /// are updated atomically on every buy and sell.
    ///
    /// # Errors
    /// - [`ContractError::NotRegistered`] if the creator is not registered.
    pub fn get_analytics(env: Env, creator: Address) -> Result<AnalyticsView, ContractError> {
        read_registered_creator_profile(&env, &creator)?;
        let trade_count: u64 = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&constants::storage::trade_count(&creator))
            .unwrap_or(0);
        let unique_traders: u64 = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&constants::storage::unique_trader_count(&creator))
            .unwrap_or(0);
        let total_volume: i128 = env
            .storage()
            .persistent()
            .get::<DataKey, i128>(&constants::storage::creator_volume(&creator))
            .unwrap_or(0);
        Ok(AnalyticsView {
            creator,
            trade_count,
            unique_traders,
            total_volume,
        })
    }

    // -----------------------------------------------------------------------
    // Feature: creator reputation scoring
    // -----------------------------------------------------------------------

    /// Read-only view: returns a creator's reputation score and its per-reason
    /// breakdown.
    ///
    /// The breakdown accumulates the signed points contributed by each reason
    /// since registration, so it always reconciles with the headline `score`.
    /// A creator with no recorded history returns a zeroed view rather than an
    /// error, but the creator must still be registered.
    ///
    /// # Errors
    /// - [`ReputationError::NotRegistered`] if the creator is not registered.
    pub fn get_reputation(env: Env, creator: Address) -> Result<ReputationView, ReputationError> {
        read_registered_creator_profile(&env, &creator)
            .map_err(|_| ReputationError::NotRegistered)?;

        let score = read_reputation_score(&env, &creator);
        let breakdown = read_reputation_breakdown(&env, &creator);

        let deprecated: bool = env
            .storage()
            .persistent()
            .has(&constants::storage::deprecated_key(&creator));

        Ok(ReputationView {
            creator,
            score,
            key_launches: breakdown.key_launches,
            milestones_reached: breakdown.milestones_reached,
            governance_participations: breakdown.governance_participations,
            governance_violations: breakdown.governance_violations,
            deprecated,
            breakdown,
        })
    }

    /// Records a governance violation against a creator, decrementing their
    /// reputation score.
    ///
    /// The protocol admin must authorize the call. `penalty` is a positive
    /// point deduction capped at [`MAX_GOVERNANCE_VIOLATION_PENALTY`] so a
    /// single call cannot wipe out a creator's accumulated standing. The
    /// resulting score is floored at [`REPUTATION_MIN_SCORE`].
    ///
    /// # Errors
    /// - [`ReputationError::Unauthorized`] if `admin` is not the protocol admin.
    /// - [`ReputationError::NotRegistered`] if the creator is not registered.
    /// - [`ReputationError::NotPositiveAmount`] if `penalty` is not positive.
    pub fn apply_governance_violation(
        env: Env,
        admin: Address,
        creator: Address,
        penalty: i128,
    ) -> Result<(), ReputationError> {
        admin.require_auth();
        assert_is_admin(&env, &admin).map_err(|_| ReputationError::Unauthorized)?;
        read_registered_creator_profile(&env, &creator)
            .map_err(|_| ReputationError::NotRegistered)?;
        if penalty <= 0 {
            return Err(ReputationError::NotPositiveAmount);
        }

        let bounded = penalty.min(MAX_GOVERNANCE_VIOLATION_PENALTY);
        apply_reputation_delta(
            &env,
            &creator,
            -bounded,
            ReputationReason::GovernanceViolation,
        )
    }

    // -----------------------------------------------------------------------
    // Feature: key transfer allowances (approve / transfer_from)
    // -----------------------------------------------------------------------

    /// Read-only view: returns the remaining transfer allowance `spender` may
    /// draw from `owner`'s balance of `key_id`.
    ///
    /// Returns `0` when no allowance has been granted.
    pub fn get_allowance(env: Env, owner: Address, spender: Address, key_id: Address) -> u32 {
        let key = constants::storage::key_allowance(&owner, &spender, &key_id);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Sets the transfer allowance `spender` may draw from the caller's
    /// balance of `key_id`.
    ///
    /// The allowance is per `(owner, spender, key_id)` tuple and is overwritten
    /// rather than accumulated, matching the ERC-20 `approve` semantics that
    /// marketplace and staking integrations expect. Passing `amount = 0` revokes
    /// the allowance and removes the storage entry.
    ///
    /// The owner must authorize the call. `spender` must not be the zero
    /// address, and must not be the owner themselves (a self-allowance is
    /// meaningless and would only create a second transfer path around
    /// `transfer_keys`).
    ///
    /// # Errors
    /// - [`AllowanceError::ZeroAddress`] if `spender` is the zero address.
    /// - [`AllowanceError::SelfTransfer`] if `spender` is the caller.
    /// - [`AllowanceError::NotRegistered`] if `key_id` is not registered.
    pub fn approve(
        env: Env,
        owner: Address,
        spender: Address,
        key_id: Address,
        amount: u32,
    ) -> Result<(), AllowanceError> {
        owner.require_auth();
        validate_non_zero_address(&env, &spender).map_err(|_| AllowanceError::ZeroAddress)?;
        if spender == owner {
            return Err(AllowanceError::SelfTransfer);
        }
        read_registered_creator_profile(&env, &key_id)
            .map_err(|_| AllowanceError::NotRegistered)?;

        let key = constants::storage::key_allowance(&owner, &spender, &key_id);
        if amount == 0 {
            env.storage().persistent().remove(&key);
        } else {
            env.storage().persistent().set(&key, &amount);
            extend_key_ttl_to_full_window(&env, &key);
        }

        env.events().publish(
            events::approval_topics(&owner, &spender),
            events::ApprovalEvent {
                owner,
                spender,
                amount,
                key_id,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    /// Transfers keys from `from` to `to` on behalf of `from`, drawing down the
    /// allowance `from` granted to the calling `spender` via
    /// [`CreatorKeysContract::approve`].
    ///
    /// The spender must authorize the call. Every invariant enforced by
    /// `transfer_keys` is enforced here too: the protocol pause flag, the
    /// creator's post-buy cooldown window, the frozen-position guard, the
    /// recipient's per-wallet holding cap, and dividend settlement for both
    /// sides. A holder-count-changed event is emitted on the same zero-boundary
    /// crossings as the direct transfer path. Supply and holder counts are
    /// untouched by minting or burning — ownership simply moves.
    ///
    /// The allowance is decremented in the same call that moves the keys, so a
    /// spender can never spend the same allowance twice.
    ///
    /// # Errors
    /// - [`AllowanceError::ProtocolPaused`] if the contract is paused.
    /// - [`AllowanceError::ZeroAmount`] if `amount` is zero.
    /// - [`AllowanceError::SelfTransfer`] if `from == to`.
    /// - [`AllowanceError::ZeroAddress`] if `to` is the zero address.
    /// - [`AllowanceError::NotRegistered`] if `key_id` is not registered.
    /// - [`AllowanceError::CooldownActive`] if `from` is inside the creator's
    ///   post-buy cooldown window.
    /// - [`AllowanceError::FrozenPosition`] if `from`'s keys are frozen.
    /// - [`AllowanceError::InsufficientBalance`] if `from`'s available balance
    ///   is below `amount`.
    /// - [`AllowanceError::InsufficientAllowance`] if the remaining allowance is
    ///   below `amount`.
    /// - [`AllowanceError::HoldingCapExceeded`] if `to` would exceed the cap.
    pub fn transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        key_id: Address,
        amount: u32,
    ) -> Result<(), AllowanceError> {
        spender.require_auth();
        assert_not_paused(&env).map_err(|_| AllowanceError::ProtocolPaused)?;

        if amount == 0 {
            return Err(AllowanceError::ZeroAmount);
        }
        if from == to {
            return Err(AllowanceError::SelfTransfer);
        }
        validate_non_zero_address(&env, &to).map_err(|_| AllowanceError::ZeroAddress)?;

        let mut profile: CreatorProfile = read_registered_creator_profile(&env, &key_id)
            .map_err(|_| AllowanceError::NotRegistered)?;

        // Mirrors the buy-cooldown guard in `transfer_keys`: a delegating
        // spender must not be able to route keys around the creator's cooldown.
        let cooldown_ledgers: u32 = env
            .storage()
            .persistent()
            .get(&constants::storage::buy_cooldown(&key_id))
            .unwrap_or(0);
        if cooldown_ledgers > 0 {
            if let Some(last_ledger) = env
                .storage()
                .persistent()
                .get::<DataKey, u32>(&constants::storage::last_buy_ledger(&key_id, &from))
            {
                if env.ledger().sequence().saturating_sub(last_ledger) < cooldown_ledgers {
                    return Err(AllowanceError::CooldownActive);
                }
            }
        }

        assert_position_not_frozen(&env, &key_id, &from)
            .map_err(|_| AllowanceError::FrozenPosition)?;

        // The allowance is checked before any balance is touched so a rejected
        // call cannot leave a partially applied transfer behind.
        let allowance_key = constants::storage::key_allowance(&from, &spender, &key_id);
        let allowance: u32 = env.storage().persistent().get(&allowance_key).unwrap_or(0);
        if allowance < amount {
            return Err(AllowanceError::InsufficientAllowance);
        }

        let from_balance_key = constants::storage::holder_balance_key(&key_id, &from);
        let from_balance: u32 = env
            .storage()
            .persistent()
            .get(&from_balance_key)
            .unwrap_or(0);
        if from_balance < amount {
            return Err(AllowanceError::InsufficientBalance);
        }
        if available_holder_balance(&env, &key_id, &from) < amount {
            // Frozen keys are what make an otherwise sufficient balance unavailable.
            return Err(AllowanceError::FrozenPosition);
        }

        let to_balance_key = constants::storage::holder_balance_key(&key_id, &to);
        let to_balance: u32 = env.storage().persistent().get(&to_balance_key).unwrap_or(0);

        // Settle dividends on both sides at their pre-transfer balances.
        settle_holder_dividends(&env, &key_id, &from, from_balance)
            .map_err(|_| AllowanceError::Overflow)?;
        settle_holder_dividends(&env, &key_id, &to, to_balance)
            .map_err(|_| AllowanceError::Overflow)?;

        let new_from_balance = from_balance
            .checked_sub(amount)
            .ok_or(AllowanceError::InsufficientBalance)?;
        let new_to_balance = to_balance
            .checked_add(amount)
            .ok_or(AllowanceError::Overflow)?;
        assert_within_holding_cap(&env, &key_id, new_to_balance)
            .map_err(|_| AllowanceError::HoldingCapExceeded)?;

        // The recipient's cap check above already guarantees the write cannot
        // fail, so the balance and allowance updates below are safe to apply
        // unconditionally.
        env.storage()
            .persistent()
            .set(&from_balance_key, &new_from_balance);
        env.storage()
            .persistent()
            .set(&to_balance_key, &new_to_balance);
        extend_key_ttl_to_full_window(&env, &from_balance_key);
        extend_key_ttl_to_full_window(&env, &to_balance_key);

        // The two adjustments below are mutually exclusive: `amount > 0` and
        // `from != to`, so at most one side crosses the zero boundary.
        let old_holder_count = profile.holder_count;
        if new_from_balance == 0 {
            profile.holder_count = profile
                .holder_count
                .checked_sub(1)
                .ok_or(AllowanceError::Overflow)?;
        }
        if to_balance == 0 {
            profile.holder_count = profile
                .holder_count
                .checked_add(1)
                .ok_or(AllowanceError::Overflow)?;
        }
        let new_holder_count = profile.holder_count;
        let profile_key = constants::storage::creator(&key_id);
        env.storage().persistent().set(&profile_key, &profile);
        extend_key_ttl_to_full_window(&env, &profile_key);

        // Mirrors `transfer_keys` so indexers tracking holder counts see the
        // delegated path too.
        emit_holder_count_changed(&env, &key_id, old_holder_count, new_holder_count);

        // Decrement the allowance in the same call as the transfer so it can
        // never be spent twice.
        let remaining_allowance = allowance - amount;
        if remaining_allowance == 0 {
            env.storage().persistent().remove(&allowance_key);
        } else {
            env.storage()
                .persistent()
                .set(&allowance_key, &remaining_allowance);
            extend_key_ttl_to_full_window(&env, &allowance_key);
        }

        env.events().publish(
            events::transfer_from_topics(&key_id, &spender),
            events::TransferFromEvent {
                key_id: key_id.clone(),
                spender,
                from,
                to,
                amount,
                remaining_allowance,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Feature: sell tax routed to the buyback pool
    // -----------------------------------------------------------------------

    /// Read-only view: returns a creator's configured sell tax in basis points.
    ///
    /// Returns `0` when no tax has been configured.
    pub fn get_sell_tax_bps(env: Env, creator: Address) -> u32 {
        let key = constants::storage::sell_tax_bps(&creator);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Sets the sell tax deducted from proceeds of `creator`'s key sales.
    ///
    /// Only `creator` may set their own tax. `tax_bps` must not exceed
    /// [`MAX_SELL_TAX_BPS`]; passing `0` disables the tax. The collected tax is
    /// forwarded to the buyback pool balance readable through
    /// [`CreatorKeysContract::get_buyback_pool_balance`].
    ///
    /// # Errors
    /// - [`SellTaxError::Unauthorized`] if `caller` is not the creator.
    /// - [`SellTaxError::NotRegistered`] if the creator is not registered.
    /// - [`SellTaxError::TaxExceedsMax`] if `tax_bps` exceeds the ceiling.
    pub fn set_sell_tax_bps(
        env: Env,
        caller: Address,
        creator: Address,
        tax_bps: u32,
    ) -> Result<(), SellTaxError> {
        caller.require_auth();
        if caller != creator {
            return Err(SellTaxError::Unauthorized);
        }
        read_registered_creator_profile(&env, &creator).map_err(|_| SellTaxError::NotRegistered)?;
        if tax_bps > MAX_SELL_TAX_BPS {
            return Err(SellTaxError::TaxExceedsMax);
        }

        let key = constants::storage::sell_tax_bps(&creator);
        let old_tax_bps: u32 = env.storage().persistent().get(&key).unwrap_or(0);

        if tax_bps == 0 {
            env.storage().persistent().remove(&key);
        } else {
            env.storage().persistent().set(&key, &tax_bps);
            extend_key_ttl_to_full_window(&env, &key);
        }

        env.events().publish(
            events::sell_tax_updated_topics(&creator),
            events::SellTaxUpdatedEvent {
                creator,
                old_tax_bps,
                new_tax_bps: tax_bps,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    /// Read-only view: returns the current buyback pool balance in XLM stroops
    /// alongside the address it is credited to.
    ///
    /// The balance accumulates the sell tax collected by every key, and starts
    /// at `0` with no pool address configured.
    pub fn get_buyback_pool_balance(env: Env) -> (i128, Option<Address>) {
        let balance: i128 = env
            .storage()
            .persistent()
            .get(&constants::storage::BUYBACK_POOL_BALANCE)
            .unwrap_or(0);
        let pool: Option<Address> = env
            .storage()
            .persistent()
            .get(&constants::storage::BUYBACK_POOL_ADDRESS);
        (balance, pool)
    }

    /// Sets the address credited with the buyback pool balance (admin only).
    ///
    /// # Errors
    /// - [`SellTaxError::ZeroAddress`] is not used here; the zero address is
    ///   rejected by the shared address validator surfaced as
    ///   [`ContractError::ZeroAddress`].
    pub fn set_buyback_pool_address(
        env: Env,
        admin: Address,
        pool: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        validate_non_zero_address(&env, &pool)?;
        let key = constants::storage::BUYBACK_POOL_ADDRESS;
        env.storage().persistent().set(&key, &pool);
        extend_key_ttl_to_full_window(&env, &key);
        Ok(())
    }

    /// Deducts `creator`'s configured sell tax from `gross_proceeds` and credits
    /// it to the buyback pool.
    ///
    /// Returns `(net_proceeds, tax_amount, pool_balance_after)`. A creator with
    /// no configured tax, a `0` tax, or proceeds too small for the tax to floor
    /// to a non-zero amount all return the full proceeds with no pool credit.
    ///
    /// The pool balance is written in the same call that computes the tax, so a
    /// sell either credits the pool and reduces the seller's proceeds, or leaves
    /// both untouched.
    fn collect_sell_tax(
        env: &Env,
        creator: &Address,
        gross_proceeds: i128,
    ) -> Result<(i128, i128, i128), ContractError> {
        let tax_bps: u32 = env
            .storage()
            .persistent()
            .get(&constants::storage::sell_tax_bps(creator))
            .unwrap_or(0);
        if tax_bps == 0 {
            return Ok((gross_proceeds, 0, 0));
        }

        let tax_amount = fee::apply_percentage_fee(gross_proceeds, tax_bps)
            .ok_or(ContractError::Overflow)?
            .min(gross_proceeds);
        if tax_amount <= 0 {
            return Ok((gross_proceeds, 0, 0));
        }

        let pool_key = constants::storage::BUYBACK_POOL_BALANCE;
        let current_balance: i128 = env.storage().persistent().get(&pool_key).unwrap_or(0);
        let new_balance = current_balance
            .checked_add(tax_amount)
            .ok_or(ContractError::Overflow)?;
        env.storage().persistent().set(&pool_key, &new_balance);
        extend_key_ttl_to_full_window(env, &pool_key);

        Ok((gross_proceeds - tax_amount, tax_amount, new_balance))
    }

    // -----------------------------------------------------------------------
    // Feature: governance quorum escalation
    // -----------------------------------------------------------------------

    /// Read-only view: returns the protocol-wide quorum-escalation config.
    ///
    /// `None` means escalation is disabled, which is the default until an admin
    /// calls [`CreatorKeysContract::set_escalation_config`].
    pub fn get_escalation_config(env: Env) -> Option<EscalationConfig> {
        env.storage()
            .persistent()
            .get(&constants::storage::ESCALATION_CONFIG)
    }

    /// Configures the protocol-wide quorum-escalation parameters (admin only).
    ///
    /// `threshold_bps` is the fraction of the proposal's own quorum requirement
    /// that must already be met before an extension is granted, and must be
    /// between [`MIN_ESCALATION_THRESHOLD_BPS`] and
    /// [`MAX_ESCALATION_THRESHOLD_BPS`]. `extension_ledgers` must be positive
    /// and no greater than [`MAX_ESCALATION_EXTENSION_LEDGERS`]. `max_extensions`
    /// must be no greater than [`MAX_ESCALATION_EXTENSIONS_BOUND`].
    ///
    /// Setting `max_extensions` to `0` disables escalation for every proposal
    /// while leaving the rest of the config readable.
    ///
    /// # Errors
    /// - [`EscalationError::Unauthorized`] if `admin` is not the protocol admin.
    /// - [`EscalationError::InvalidEscalationConfig`] if any bound is violated.
    pub fn set_escalation_config(
        env: Env,
        admin: Address,
        config: EscalationConfig,
    ) -> Result<(), EscalationError> {
        admin.require_auth();
        assert_is_admin(&env, &admin).map_err(|_| EscalationError::Unauthorized)?;

        if config.threshold_bps < MIN_ESCALATION_THRESHOLD_BPS
            || config.threshold_bps > MAX_ESCALATION_THRESHOLD_BPS
            || config.extension_ledgers == 0
            || config.extension_ledgers > MAX_ESCALATION_EXTENSION_LEDGERS
            || config.max_extensions > MAX_ESCALATION_EXTENSIONS_BOUND
        {
            return Err(EscalationError::InvalidEscalationConfig);
        }

        let old_config: Option<EscalationConfig> = env
            .storage()
            .persistent()
            .get(&constants::storage::ESCALATION_CONFIG);

        let key = constants::storage::ESCALATION_CONFIG;
        env.storage().persistent().set(&key, &config);
        extend_key_ttl_to_full_window(&env, &key);

        env.events().publish(
            events::escalation_config_updated_topics(&admin),
            events::EscalationConfigUpdatedEvent {
                admin,
                had_previous_config: old_config.is_some(),
                old_threshold_bps: old_config.map(|c| c.threshold_bps).unwrap_or(0),
                old_extension_ledgers: old_config.map(|c| c.extension_ledgers).unwrap_or(0),
                old_max_extensions: old_config.map(|c| c.max_extensions).unwrap_or(0),
                new_threshold_bps: config.threshold_bps,
                new_extension_ledgers: config.extension_ledgers,
                new_max_extensions: config.max_extensions,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(())
    }

    /// Read-only view: returns a proposal's quorum-escalation state.
    ///
    /// Reports the deadline, how many extensions have been consumed, current
    /// participation against the proposal's quorum requirement, and whether the
    /// proposal is eligible for or has exhausted further extension.
    pub fn get_escalation_status(
        env: Env,
        creator_id: Address,
        poll_id: u32,
    ) -> Result<EscalationView, EscalationError> {
        let poll = events::read_poll(&env, &creator_id, poll_id)
            .map_err(|_| EscalationError::PollNotFound)?;

        let extensions_used = events::read_poll_extension_count(&env, &creator_id, poll_id);
        let config = Self::get_escalation_config(env.clone());
        let max_extensions = config.map(|c| c.max_extensions).unwrap_or(0);
        let threshold_bps = config.map(|c| c.threshold_bps).unwrap_or(0);

        let now = env.ledger().sequence();
        let ledgers_remaining = poll.expires_at.saturating_sub(now);

        let circulating_supply = read_creator_supply(&env, &creator_id);
        let quorum_bps: u32 = env
            .storage()
            .persistent()
            .get(&constants::storage::quorum_bps(&creator_id))
            .unwrap_or(0);

        let participation_bps = if circulating_supply == 0 {
            0
        } else {
            ((poll.total_weight as u128 * 10_000) / circulating_supply as u128) as u32
        };

        let eligible = is_escalation_eligible(
            participation_bps,
            quorum_bps,
            threshold_bps,
            max_extensions,
            extensions_used,
        );

        // Matches `events::poll_extensions_exhausted`: a zero budget means
        // escalation is off, which is not the same as a spent budget.
        let exhausted = max_extensions > 0 && extensions_used >= max_extensions;

        Ok(EscalationView {
            poll_id,
            expires_at: poll.expires_at,
            extensions_used,
            max_extensions,
            ledgers_remaining,
            participation_bps,
            quorum_bps,
            eligible,
            exhausted,
        })
    }

    /// Extends a proposal's voting period when it is close to quorum but has
    /// not reached it.
    ///
    /// Soroban has no scheduled execution, so this evaluation is permissionless
    /// and deterministic: anyone may call it, but the outcome depends only on
    /// stored state. A proposal is extended when all of the following hold:
    ///
    /// 1. Quorum escalation is configured (otherwise
    ///    [`EscalationError::EscalationDisabled`]).
    /// 2. The proposal exists and is not already closed.
    /// 3. The current ledger is within
    ///    [`ESCALATION_EVALUATION_WINDOW_LEDGERS`] of the deadline, so a
    ///    proposal cannot be extended long before it would close.
    /// 4. Participation has reached `threshold_bps` of the proposal's own
    ///    quorum requirement (`quorum_bps` of circulating supply).
    /// 5. The proposal has not consumed `max_extensions` extensions.
    ///
    /// Extending adds `extension_ledgers` to the deadline and increments the
    /// consumed-extension count, so a proposal can be postponed at most
    /// `max_extensions` times. Once exhausted, the proposal closes on its
    /// existing deadline whether or not it reached quorum.
    ///
    /// # Errors
    /// - [`EscalationError::EscalationDisabled`] if no config is set.
    /// - [`EscalationError::PollNotFound`] if the proposal does not exist.
    /// - [`EscalationError::AlreadyClosed`] if the proposal is closed.
    /// - [`EscalationError::TooEarlyToEscalate`] if the deadline is too far away.
    /// - [`EscalationError::BelowEscalationThreshold`] if participation is too low.
    /// - [`EscalationError::MaxExtensionsReached`] if extensions are exhausted.
    pub fn evaluate_poll_escalation(
        env: Env,
        creator_id: Address,
        poll_id: u32,
    ) -> Result<u32, EscalationError> {
        let config =
            Self::get_escalation_config(env.clone()).ok_or(EscalationError::EscalationDisabled)?;
        if config.max_extensions == 0 {
            return Err(EscalationError::EscalationDisabled);
        }

        let mut poll = events::read_poll(&env, &creator_id, poll_id)
            .map_err(|_| EscalationError::PollNotFound)?;
        if poll.closed {
            return Err(EscalationError::AlreadyClosed);
        }

        let now = env.ledger().sequence();
        if now.saturating_add(ESCALATION_EVALUATION_WINDOW_LEDGERS) < poll.expires_at {
            return Err(EscalationError::TooEarlyToEscalate);
        }

        let extensions_used = events::read_poll_extension_count(&env, &creator_id, poll_id);
        if extensions_used >= config.max_extensions {
            return Err(EscalationError::MaxExtensionsReached);
        }

        let circulating_supply = read_creator_supply(&env, &creator_id);
        let quorum_bps: u32 = env
            .storage()
            .persistent()
            .get(&constants::storage::quorum_bps(&creator_id))
            .unwrap_or(0);
        let participation_bps =
            escalation_participation_bps(poll.total_weight, circulating_supply)?;

        if !is_escalation_eligible(
            participation_bps,
            quorum_bps,
            config.threshold_bps,
            config.max_extensions,
            extensions_used,
        ) {
            return Err(EscalationError::BelowEscalationThreshold);
        }

        let old_expires_at = poll.expires_at;
        let new_expires_at = old_expires_at
            .checked_add(config.extension_ledgers)
            .ok_or(EscalationError::Overflow)?;
        poll.expires_at = new_expires_at;
        events::write_poll(&env, &creator_id, poll_id, &poll);

        let extensions_used = extensions_used
            .checked_add(1)
            .ok_or(EscalationError::Overflow)?;
        events::write_poll_extension_count(&env, &creator_id, poll_id, extensions_used);

        env.events().publish(
            events::proposal_extended_topics(&creator_id, poll_id),
            events::ProposalExtendedEvent {
                creator_id,
                poll_id,
                old_expires_at,
                new_expires_at,
                extensions_used,
                max_extensions: config.max_extensions,
                ledger: env.ledger().sequence(),
            },
        );

        Ok(new_expires_at)
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

#[cfg(test)]
mod test;

#[cfg(test)]
mod test_issues_778_779_781_782;

#[cfg(test)]
mod test_issues_884_885_887_889;

#[cfg(test)]
mod test_staking_lifecycle;

#[cfg(test)]
mod test_issues_904_905_906_908;
