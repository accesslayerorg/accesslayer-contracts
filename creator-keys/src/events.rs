//! Centralized event names and helpers for consistent event emission.
//!
//! This module provides a single source of truth for event names used throughout
//! the contract, reducing string duplication and ensuring consistency across
//! event emission paths.
//!
//! ### Event Schema Stability

use crate::{
    constants, extend_key_ttl_to_full_window, read_creator_supply, read_registered_creator_profile,
    CreatorKeysContract, CreatorKeysContractArgs, CreatorKeysContractClient, SwapDirection,
    VaultAllocation,
};
use soroban_sdk::{
    contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, String, Symbol,
    Vec,
};

/// Event name for protocol trade fee collected on a buy or sell.
pub const FEE_COLLECTED_EVENT_NAME: Symbol = symbol_short!("fee_coll");

/// Event name for a per-key dynamic trade fee bracket change.
pub const FEE_ADJUSTED_EVENT_NAME: Symbol = symbol_short!("fee_adj");
pub const FEE_ADJUSTED_DATA_FIELDS: [&str; 5] = [
    "key_id",
    "previous_fee_bps",
    "new_fee_bps",
    "rolling_volume",
    "timestamp",
];

/// Stable payload for a detected change in a key's dynamic trade fee.
///
/// Event shape:
/// - topics: `(FEE_ADJUSTED_EVENT_NAME, key_id)`
/// - data: `FeeAdjustedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct FeeAdjustedEvent {
    pub key_id: Address,
    pub previous_fee_bps: u32,
    pub new_fee_bps: u32,
    pub rolling_volume: i128,
    pub timestamp: u64,
}

/// Event name for a key moving between rolling-volume fee tiers.
pub const KEY_FEE_TIER_CHANGED_EVENT_NAME: Symbol = symbol_short!("key_tier");
pub const KEY_FEE_TIER_CHANGED_DATA_FIELDS: [&str; 6] = [
    "key_id",
    "new_tier_index",
    "previous_fee_bps",
    "new_fee_bps",
    "rolling_volume",
    "timestamp",
];

/// Stable payload emitted alongside [`FeeAdjustedEvent`] when a key's active
/// fee tier changes, either after a trade or when 24-hour volume has decayed.
///
/// Event shape:
/// - topics: `(KEY_FEE_TIER_CHANGED_EVENT_NAME, key_id)`
/// - data: `KeyFeeTierChangedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyFeeTierChangedEvent {
    pub key_id: Address,
    pub new_tier_index: u32,
    pub previous_fee_bps: u32,
    pub new_fee_bps: u32,
    pub rolling_volume: i128,
    pub timestamp: u64,
}

/// Stable fee collection event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(FEE_COLLECTED_EVENT_NAME, treasury, trade_id)`
/// - data: `FeeCollectedEvent`
///
/// Emitted on every buy and sell once the protocol trade fee is configured,
/// carrying the trade identifier, the gross amount, the deducted fee, and the
/// treasury address that received it.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct FeeCollectedEvent {
    /// Treasury address that received the fee.
    pub treasury: Address,
    /// Monotonic trade counter for the fee collection event.
    pub trade_id: u64,
    /// Gross trade amount before the protocol deduction.
    pub amount: i128,
    /// Fee amount deducted from the trade.
    pub fee: i128,
    /// Ledger sequence number at the time of the trade.
    pub ledger: u32,
}

/// Shared fee collected event topics tuple including the trade identifier.
pub fn fee_collected_topics_with_trade_id(
    treasury: &Address,
    trade_id: u64,
) -> (Symbol, Address, u64) {
    (FEE_COLLECTED_EVENT_NAME, treasury.clone(), trade_id)
}

/// Event name for LP allocation sent to liquidity pool.
pub const LP_ALLOCATION_SENT_EVENT_NAME: Symbol = symbol_short!("lp_alloc");

/// Stable LP allocation sent event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(LP_ALLOCATION_SENT_EVENT_NAME, lp_contract)`
/// - data: `LpAllocationSentEvent`
///
/// Emitted on each buy when LP routing is configured, carrying the amount
/// forwarded to the LP contract and the LP contract address.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LpAllocationSentEvent {
    /// LP contract address that received the allocation.
    pub lp_contract: Address,
    /// Amount forwarded to the LP contract.
    pub amount: i128,
    /// Ledger sequence number at the time of the allocation.
    pub ledger: u32,
}

/// Shared LP allocation sent event topics tuple.
pub fn lp_allocation_sent_topics(lp_contract: &Address) -> (Symbol, Address) {
    (LP_ALLOCATION_SENT_EVENT_NAME, lp_contract.clone())
}

/// Event name for a sell rejected by the anti-flash-trade lockup window.
pub const LOCKUP_BLOCKED_EVENT_NAME: Symbol = symbol_short!("lck_blk");

/// Stable lockup-blocked event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(LOCKUP_BLOCKED_EVENT_NAME, creator_id, seller)`
/// - data: `LockupBlockedEvent`
///
/// Emitted when a sell is rejected because the seller's most recent buy for
/// this creator falls inside the configured lockup window.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LockupBlockedEvent {
    /// Creator whose keys the seller attempted to sell.
    pub creator_id: Address,
    /// Seller whose sale was rejected.
    pub seller: Address,
    /// Ledger timestamp of the seller's most recent buy.
    pub last_buy_timestamp: u64,
    /// Timestamp at which the lockup expires (exclusive).
    pub unlock_at: u64,
    /// Ledger timestamp at rejection.
    pub current_timestamp: u64,
}

/// Shared lockup blocked event topics tuple.
pub fn lockup_blocked_topics(creator: &Address, seller: &Address) -> (Symbol, Address, Address) {
    (LOCKUP_BLOCKED_EVENT_NAME, creator.clone(), seller.clone())
}

/// Event name for creating an investor-specific lock-up.
pub const LOCKUP_CREATED_EVENT_NAME: Symbol = symbol_short!("lck_crt");

/// Investor lock-up creation payload.
///
/// Event shape:
/// - topics: `(LOCKUP_CREATED_EVENT_NAME, wallet, key_id)`
/// - data: `LockupCreatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LockupCreatedEvent {
    pub wallet: Address,
    pub key_id: Address,
    pub unlock_timestamp: u64,
}

/// Shared investor lock-up creation event topics.
pub fn lockup_created_topics(wallet: &Address, key_id: &Address) -> (Symbol, Address, Address) {
    (LOCKUP_CREATED_EVENT_NAME, wallet.clone(), key_id.clone())
}

/// Event name for releasing an investor-specific lock-up.
pub const LOCKUP_RELEASED_EVENT_NAME: Symbol = symbol_short!("lck_rel");

/// Investor lock-up release payload.
///
/// Event shape:
/// - topics: `(LOCKUP_RELEASED_EVENT_NAME, wallet, key_id)`
/// - data: `LockupReleasedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LockupReleasedEvent {
    pub wallet: Address,
    pub key_id: Address,
    pub unlock_timestamp: u64,
    pub released_at: u64,
}

/// Shared investor lock-up release event topics.
pub fn lockup_released_topics(wallet: &Address, key_id: &Address) -> (Symbol, Address, Address) {
    (LOCKUP_RELEASED_EVENT_NAME, wallet.clone(), key_id.clone())
}

/// Event name for quorum threshold update.
pub const QUORUM_UPDATED_EVENT_NAME: Symbol = symbol_short!("qrm_upd");

/// Event name for proposal/poll closed.
pub const POLL_CLOSED_EVENT_NAME: Symbol = symbol_short!("poll_cls");

/// Event name for protocol pause.
pub const PAUSE_EVENT_NAME: Symbol = symbol_short!("pause");

/// Event name for protocol unpause.
pub const UNPAUSE_EVENT_NAME: Symbol = symbol_short!("unpause");

/// Event name for a wallet being added to the admin blacklist.
pub const BLACKLIST_ADDED_EVENT_NAME: Symbol = symbol_short!("blk_add");

/// Event name for a wallet being removed from the admin blacklist.
pub const BLACKLIST_REMOVED_EVENT_NAME: Symbol = symbol_short!("blk_rem");

/// Event name for the protocol-wide buy deadline ledger being set or cleared.
pub const GLOBAL_DEADLINE_SET_EVENT_NAME: Symbol = symbol_short!("dl_set");

/// Event name for a forwarded buy submitted by the trusted forwarder.
pub const FORWARDED_BUY_EVENT_NAME: Symbol = symbol_short!("fwd_buy");

/// Event name for creator registration.
pub const REGISTER_EVENT_NAME: Symbol = symbol_short!("register");

/// Event name for key purchase.
pub const BUY_EVENT_NAME: Symbol = symbol_short!("buy");

/// Event name for key sale.
pub const SELL_EVENT_NAME: Symbol = symbol_short!("sell");

/// Event name for peer-to-peer key transfer.
pub const TRANSFER_EVENT_NAME: Symbol = symbol_short!("transfer");

/// Event name for creator key buyback.
pub const BUYBACK_EVENT_NAME: Symbol = symbol_short!("buyback");

/// Event name for referral fee earned.
pub const REFERRAL_FEE_EARNED_EVENT_NAME: Symbol = symbol_short!("referral");

/// Event name for governance poll creation.
pub const POLL_CREATED_EVENT_NAME: Symbol = symbol_short!("poll_new");

/// Event name for governance poll votes.
pub const POLL_VOTE_EVENT_NAME: Symbol = symbol_short!("poll_vote");

/// Event name for delegation set.
pub const DELEGATION_SET_EVENT_NAME: Symbol = symbol_short!("dlg_set");

/// Event name for delegation revoked.
pub const DELEGATION_REVOKED_EVENT_NAME: Symbol = symbol_short!("dlg_rev");

/// Event name for a performance bond being forfeited to the protocol treasury.
pub const BOND_FORFEITED_EVENT_NAME: Symbol = symbol_short!("bond_forf");
/// Topic index for the event name in common event topic tuples.
pub const TOPIC_EVENT_NAME_INDEX: u32 = 0;

/// Topic index for the creator address in common event topic tuples.
pub const TOPIC_CREATOR_INDEX: u32 = 1;

/// Topic index for the buyer/seller/actor address in common event topic tuples.
pub const TOPIC_BUYER_INDEX: u32 = 2;

/// Stable field order for bond forfeited event payloads.
pub const BOND_FORFEITED_DATA_FIELDS: [&str; 4] = ["key_id", "reason", "amount", "ledger"];

/// Stable bond forfeited event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(BOND_FORFEITED_EVENT_NAME, key_id)`
/// - data: `BondForfeitedEvent`
///
/// Emitted when an admin forfeits a creator's performance bond, routing the
/// deposited amount to the protocol treasury.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BondForfeitedEvent {
    /// Creator key whose bond was forfeited.
    pub key_id: Address,
    /// Human-readable reason supplied by the admin.
    pub reason: String,
    /// Bond amount routed to the protocol treasury.
    pub amount: i128,
    /// Ledger sequence number at the time of forfeiture.
    pub ledger: u32,
}

/// Shared bond forfeited event topics tuple.
pub fn bond_forfeited_topics(key_id: &Address) -> (Symbol, Address) {
    (BOND_FORFEITED_EVENT_NAME, key_id.clone())
}

/// Stable field order for registration event payloads.
pub const REGISTER_EVENT_DATA_FIELDS: [&str; 8] = [
    "creator",
    "handle",
    "supply",
    "holder_count",
    "creator_bps",
    "protocol_bps",
    "fee_recipient",
    "registered_at_ledger",
];

/// Stable field order for buy event payloads.
pub const BUY_EVENT_DATA_FIELDS: [&str; 6] = [
    "buyer",
    "creator_id",
    "quantity",
    "price_paid",
    "new_supply",
    "ledger",
];

/// Stable field order for sell event payloads.
pub const SELL_EVENT_DATA_FIELDS: [&str; 5] =
    ["seller", "creator_id", "quantity", "proceeds", "ledger"];

/// Stable field order for buyback event payloads.
pub const BUYBACK_EVENT_DATA_FIELDS: [&str; 5] =
    ["creator", "amount", "price_paid", "new_supply", "ledger"];

const MIN_POLL_OPTIONS: u32 = 2;
const MAX_POLL_OPTIONS: u32 = 4;
const MAX_QUESTION_CHARS: u32 = 280;
const MAX_OPTION_CHARS: u32 = 100;

/// Stable registration event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(REGISTER_EVENT_NAME, creator)`
/// - data: `CreatorRegisteredEvent`
///
/// This keeps the creator address indexed in event topics while preserving
/// a predictable payload for off-chain consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CreatorRegisteredEvent {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
    pub holder_count: u32,
    pub creator_bps: u32,
    pub protocol_bps: u32,
    /// Address that receives creator fee payouts for this creator.
    pub fee_recipient: Address,
    /// Ledger sequence number at the time of registration.
    pub registered_at_ledger: u32,
}

/// Shared registration event topics tuple.
pub fn register_event_topics(creator: &Address) -> (Symbol, Address) {
    (REGISTER_EVENT_NAME, creator.clone())
}

/// Stable buyback event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(BUYBACK_EVENT_NAME, creator)`
/// - data: `KeysBoughtBackEvent`
///
/// # Creator Fee Waiver
/// On buybacks, the creator fee is explicitly waived because the creator cannot pay
/// themselves a fee. The protocol fee still applies.
///
/// # Indexer Note
/// This event represents a creator burning keys from their own held balance,
/// which is distinct from a regular buy event. Indexers should process this
/// event separately from `BUY_EVENT_NAME` events to correctly track supply
/// changes and fee accounting.
/// Stable buy event payload for downstream indexers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysBoughtEvent {
    /// Address of the buyer performing the purchase.
    pub buyer: Address,
    /// Address of the creator whose keys are being purchased.
    pub creator_id: Address,
    /// Number of keys being bought.
    pub quantity: u32,
    /// Price paid for the keys (before fees).
    pub price_paid: i128,
    /// Total supply of keys for this creator after the purchase.
    pub new_supply: u32,
    /// Ledger sequence number at the time of the purchase.
    pub ledger: u32,
}

/// Stable forwarded-buy event payload for downstream indexers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ForwardedBuyEvent {
    /// Address of the trusted forwarder that submitted the transaction.
    pub forwarder: Address,
    /// Address of the buyer on whose behalf the purchase was made.
    pub buyer: Address,
    /// Address of the creator whose keys were purchased.
    pub creator_id: Address,
    /// Number of keys purchased.
    pub quantity: u32,
    /// Ledger sequence number at the time of the forwarded purchase.
    pub ledger: u32,
}

/// Stable sell event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(SELL_EVENT_NAME, creator, seller)`
/// - data: `KeysSoldEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysSoldEvent {
    /// Address of the seller performing the sale.
    pub seller: Address,
    /// Address of the creator whose keys are being sold.
    pub creator_id: Address,
    /// Number of keys sold in this transaction.
    pub quantity: u32,
    /// Net proceeds received by the seller after fees.
    pub proceeds: i128,
    /// Total supply of keys for this creator after the sale.
    pub new_supply: u32,
    /// Ledger sequence number at the time of the sale.
    pub ledger: u32,
}

/// Event shape:
/// - topics: `(BUYBACK_EVENT_NAME, creator)`
/// - data: `KeysBoughtBackEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysBoughtBackEvent {
    /// Address of the creator performing the buyback.
    pub creator: Address,
    /// Number of keys being bought back and burned.
    pub amount: u32,
    /// Total amount paid by the creator, including protocol fee (but not creator fee).
    pub price_paid: i128,
    /// New total supply of keys for the creator after the buyback.
    pub new_supply: u32,
    /// Ledger sequence number at the time of the buyback.
    pub ledger: u32,
}

/// Shared buy event topics tuple.
pub fn buy_event_topics(creator: &Address, buyer: &Address) -> (Symbol, Address, Address) {
    (BUY_EVENT_NAME, creator.clone(), buyer.clone())
}

/// Shared peer-to-peer transfer event topics tuple.
pub fn transfer_event_topics(creator: &Address, from: &Address) -> (Symbol, Address, Address) {
    (TRANSFER_EVENT_NAME, creator.clone(), from.clone())
}

/// Shared buyback event topics tuple.
pub fn buyback_event_topics(creator: &Address) -> (Symbol, Address) {
    (BUYBACK_EVENT_NAME, creator.clone())
}

/// Event name for dividend distribution.
pub const DIVIDEND_DISTRIBUTED_EVENT_NAME: Symbol = symbol_short!("div_dist");

/// Event name for dividend claim.
pub const DIVIDEND_CLAIMED_EVENT_NAME: Symbol = symbol_short!("div_claim");

/// Event name for allocation locked.
pub const ALLOCATION_LOCKED_EVENT_NAME: Symbol = symbol_short!("alloc_lck");

/// Event name for allocation claimed.
pub const ALLOCATION_CLAIMED_EVENT_NAME: Symbol = symbol_short!("alloc_clm");

/// Event name for protocol fee recipient updated.
pub const PROTOCOL_FEE_RECIPIENT_UPDATED_EVENT_NAME: Symbol = symbol_short!("p_fee_upd");

/// Event name for creator fee recipient updated.
pub const CREATOR_FEE_RECIPIENT_UPDATED_EVENT_NAME: Symbol = symbol_short!("c_fee_upd");

/// Event name for co-creator fee accrual.
pub const CO_CREATOR_FEE_EARNED_EVENT_NAME: Symbol = symbol_short!("co_fee");

/// Event name for a creator (re)designating their co-creator split (issue #782).
pub const CO_CREATOR_SET_EVENT_NAME: Symbol = symbol_short!("co_set");

/// Stable field order for dividend distributed event payloads.
pub const DIVIDEND_DISTRIBUTED_DATA_FIELDS: [&str; 4] =
    ["creator", "total_amount", "snapshot_supply", "ledger"];

/// Stable field order for dividend claimed event payloads.
pub const DIVIDEND_CLAIMED_DATA_FIELDS: [&str; 3] = ["creator", "claimant", "amount"];

/// Stable field order for co-creator fee earned event payloads.
pub const CO_CREATOR_FEE_EARNED_DATA_FIELDS: [&str; 4] =
    ["creator_id", "co_creator", "amount", "ledger"];

/// Event shape:
/// - topics: `(DIVIDEND_DISTRIBUTED_EVENT_NAME, creator)`
/// - data: `DividendDistributedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendDistributedEvent {
    pub creator: Address,
    pub total_amount: i128,
    pub snapshot_supply: u32,
    pub ledger: u32,
}

/// Event shape:
/// - topics: `(DIVIDEND_CLAIMED_EVENT_NAME, creator, claimant)`
/// - data: `DividendClaimedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendClaimedEvent {
    pub creator: Address,
    pub claimant: Address,
    pub amount: i128,
}

pub fn dividend_distributed_topics(creator: &Address) -> (Symbol, Address) {
    (DIVIDEND_DISTRIBUTED_EVENT_NAME, creator.clone())
}

/// Event shape:
/// - topics: `(DELEGATION_SET_EVENT_NAME, creator, delegator)`
/// - data: `DelegationSetEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DelegationSetEvent {
    pub creator: Address,
    pub delegator: Address,
    pub delegate: Address,
}

/// Event shape:
/// - topics: `(DELEGATION_REVOKED_EVENT_NAME, creator, delegator)`
/// - data: `DelegationRevokedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DelegationRevokedEvent {
    pub creator: Address,
    pub delegator: Address,
}

pub fn delegation_set_topics(creator: &Address, delegator: &Address) -> (Symbol, Address, Address) {
    (
        DELEGATION_SET_EVENT_NAME,
        creator.clone(),
        delegator.clone(),
    )
}

pub fn delegation_revoked_topics(
    creator: &Address,
    delegator: &Address,
) -> (Symbol, Address, Address) {
    (
        DELEGATION_REVOKED_EVENT_NAME,
        creator.clone(),
        delegator.clone(),
    )
}

pub fn dividend_claimed_topics(
    creator: &Address,
    claimant: &Address,
) -> (Symbol, Address, Address) {
    (
        DIVIDEND_CLAIMED_EVENT_NAME,
        creator.clone(),
        claimant.clone(),
    )
}

/// Event shape:
/// - topics: `(ALLOCATION_LOCKED_EVENT_NAME, creator)`
/// - data: `AllocationLockedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AllocationLockedEvent {
    pub creator_id: Address,
    pub amount: u32,
    pub unlock_ledger: u32,
}

/// Event shape:
/// - topics: `(ALLOCATION_CLAIMED_EVENT_NAME, creator)`
/// - data: `AllocationClaimedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AllocationClaimedEvent {
    pub creator_id: Address,
    pub amount: u32,
    pub ledger: u32,
}

/// Stable field order for protocol fee recipient updated event payloads (#573).
pub const PROTOCOL_FEE_RECIPIENT_UPDATED_DATA_FIELDS: [&str; 3] =
    ["old_recipient", "new_recipient", "updated_at_ledger"];

/// Event shape:
/// - topics: `(PROTOCOL_FEE_RECIPIENT_UPDATED_EVENT_NAME, admin)`
/// - data: `ProtocolFeeRecipientUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ProtocolFeeRecipientUpdatedEvent {
    pub old_recipient: Address,
    pub new_recipient: Address,
    pub updated_at_ledger: u32,
}

/// Event shape:
/// - topics: `(CREATOR_FEE_RECIPIENT_UPDATED_EVENT_NAME, creator)`
/// - data: `CreatorFeeRecipientUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CreatorFeeRecipientUpdatedEvent {
    pub creator_id: Address,
    pub old_recipient: Address,
    pub new_recipient: Address,
}

/// Event name for contract initialization (first fee config set).
pub const CONTRACT_INITIALIZED_EVENT_NAME: Symbol = symbol_short!("init");

/// Stable contract initialization event payload for downstream indexers.
///
/// Emitted exactly once on the first successful `set_fee_config` call.
/// Re-initialization attempts revert before reaching event emission.
///
/// Event shape:
/// - topics: `(CONTRACT_INITIALIZED_EVENT_NAME, admin)`
/// - data: `ContractInitializedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ContractInitializedEvent {
    pub admin: Address,
    pub protocol_fee_bps: u32,
    pub protocol_fee_recipient: Address,
    pub initialized_at_ledger: u32,
}

/// Event name for global fee configuration update.
pub const FEE_CONFIG_UPDATED_EVENT_NAME: Symbol = symbol_short!("fee_upd");

/// Event shape:
/// - topics: `(FEE_CONFIG_UPDATED_EVENT_NAME, admin)`
/// - data: `FeeConfigUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct FeeConfigUpdatedEvent {
    pub old_bps: u32,
    pub new_bps: u32,
    pub updated_at_ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CoCreatorFeeEarned {
    pub creator_id: Address,
    pub co_creator: Address,
    pub amount: i128,
    pub ledger: u32,
}

pub fn co_creator_fee_earned_topics(
    creator_id: &Address,
    co_creator: &Address,
) -> (Symbol, Address, Address) {
    (
        CO_CREATOR_FEE_EARNED_EVENT_NAME,
        creator_id.clone(),
        co_creator.clone(),
    )
}

/// Emitted by `set_co_creator` whenever a creator designates or updates their
/// co-creator split (issue #782).
///
/// Event shape:
/// - topics: `(CO_CREATOR_SET_EVENT_NAME, creator_id, co_creator)`
/// - data: `CoCreatorSetEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CoCreatorSetEvent {
    pub creator_id: Address,
    pub co_creator: Address,
    pub split_bps: u32,
}

pub fn co_creator_set_topics(
    creator_id: &Address,
    co_creator: &Address,
) -> (Symbol, Address, Address) {
    (
        CO_CREATOR_SET_EVENT_NAME,
        creator_id.clone(),
        co_creator.clone(),
    )
}

/// Event name for a completed holder snapshot (issue #778).
pub const SNAPSHOT_TAKEN_EVENT_NAME: Symbol = symbol_short!("snap_take");

/// Event shape:
/// - topics: `(SNAPSHOT_TAKEN_EVENT_NAME, creator_id, snapshot_id)`
/// - data: `SnapshotTakenEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SnapshotTakenEvent {
    pub creator_id: Address,
    pub snapshot_id: u32,
    pub snapshot_ledger: u32,
    pub total_holders: u32,
}

pub fn snapshot_taken_topics(creator_id: &Address, snapshot_id: u32) -> (Symbol, Address, u32) {
    (SNAPSHOT_TAKEN_EVENT_NAME, creator_id.clone(), snapshot_id)
}

/// Event name for protocol treasury revenue distributed to stakers.
pub const PROTOCOL_REVENUE_DISTRIBUTED_EVENT_NAME: Symbol = symbol_short!("prot_rev");

/// Event shape:
/// - topics: `(PROTOCOL_REVENUE_DISTRIBUTED_EVENT_NAME, creator_id, snapshot_id)`
/// - data: `ProtocolRevenueDistributedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ProtocolRevenueDistributedEvent {
    pub total_distributed: i128,
    pub staker_count: u32,
    pub snapshot_id: u32,
}

pub const PROTOCOL_REVENUE_DISTRIBUTED_DATA_FIELDS: [&str; 3] =
    ["total_distributed", "staker_count", "snapshot_id"];

pub fn protocol_revenue_distributed_topics(
    creator_id: &Address,
    snapshot_id: u32,
) -> (Symbol, Address, u32) {
    (
        PROTOCOL_REVENUE_DISTRIBUTED_EVENT_NAME,
        creator_id.clone(),
        snapshot_id,
    )
}

/// Event name for creator key identity initialization (issue #779).
pub const KEY_INITIALISED_EVENT_NAME: Symbol = symbol_short!("key_init");

/// Event shape:
/// - topics: `(KEY_INITIALISED_EVENT_NAME, creator_id)`
/// - data: `KeyInitialisedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyInitialisedEvent {
    pub creator_id: Address,
    pub name: String,
    pub bio: String,
    pub avatar_uri: String,
    pub symbol: String,
    pub description: String,
    pub image_cid: String,
}

pub fn key_initialised_topics(creator_id: &Address) -> (Symbol, Address) {
    (KEY_INITIALISED_EVENT_NAME, creator_id.clone())
}

/// Event name for a blocked same-ledger buy-then-sell attempt (issue #781).
pub const FLASH_LOAN_BLOCKED_EVENT_NAME: Symbol = symbol_short!("fl_block");

/// Event shape:
/// - topics: `(FLASH_LOAN_BLOCKED_EVENT_NAME, wallet, key_id)`
/// - data: `FlashLoanBlockedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct FlashLoanBlockedEvent {
    pub wallet: Address,
    pub key_id: Address,
    pub ledger: u32,
}

pub fn flash_loan_blocked_topics(wallet: &Address, key_id: &Address) -> (Symbol, Address, Address) {
    (
        FLASH_LOAN_BLOCKED_EVENT_NAME,
        wallet.clone(),
        key_id.clone(),
    )
}

/// Stable referral fee earned event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(REFERRAL_FEE_EARNED_EVENT_NAME, creator_id, referrer)`
/// - data: `ReferralFeeEarnedEvent`
///
/// Emitted when a referrer earns a share of the protocol fee from a buy.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralFeeEarnedEvent {
    pub creator_id: Address,
    pub buyer: Address,
    pub referrer: Address,
    pub amount: i128,
    pub ledger: u32,
}

/// Shared referral fee earned event topics tuple.
pub fn referral_fee_earned_topics(
    creator_id: &Address,
    referrer: &Address,
) -> (Symbol, Address, Address) {
    (
        REFERRAL_FEE_EARNED_EVENT_NAME,
        creator_id.clone(),
        referrer.clone(),
    )
}

/// Event name for key transfer.
pub const KEYS_TRANSFERRED_EVENT_NAME: Symbol = symbol_short!("xfer");

/// Stable field order for key transfer event payloads.
pub const KEYS_TRANSFERRED_DATA_FIELDS: [&str; 5] =
    ["creator_id", "from", "to", "amount", "ledger"];

/// Stable key transfer event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(KEYS_TRANSFERRED_EVENT_NAME, creator_id, from)`
/// - data: `KeysTransferredEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysTransferredEvent {
    pub creator_id: Address,
    pub from: Address,
    pub to: Address,
    pub amount: u32,
    pub ledger: u32,
}

/// Event name for creator key airdrops.
pub const KEYS_AIRDROPPED_EVENT_NAME: Symbol = symbol_short!("airdrop");

/// Stable field order for airdrop event payloads.
pub const KEYS_AIRDROPPED_DATA_FIELDS: [&str; 6] = [
    "creator_id",
    "total_keys",
    "total_cost",
    "recipient_count",
    "skipped_count",
    "ledger",
];

/// Stable airdrop event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(KEYS_AIRDROPPED_EVENT_NAME, creator_id)`
/// - data: `KeysAirdroppedEvent`
///
/// `total_cost` is the full amount charged to the creator (curve cost plus
/// protocol fee), `skipped_count` is the number of recipients skipped due to
/// per-wallet cap, and `ledger` is the Soroban ledger sequence number at airdrop
/// time so off-chain indexers can reconstruct the timeline.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysAirdroppedEvent {
    pub creator_id: Address,
    pub total_keys: u32,
    pub total_cost: i128,
    pub recipient_count: u32,
    pub skipped_count: u32,
    pub ledger: u32,
}

/// Shared airdrop event topics tuple.
pub fn keys_airdropped_topics(creator: &Address) -> (Symbol, Address) {
    (KEYS_AIRDROPPED_EVENT_NAME, creator.clone())
}

/// Event name for treasury withdrawal by the protocol admin.
pub const TREASURY_WITHDRAWAL_EVENT_NAME: Symbol = symbol_short!("treas_out");

/// Event name for creator storage TTL extension.
pub const TTL_EXTENDED_EVENT_NAME: Symbol = symbol_short!("ttl_ext");

/// Stable field order for TTL extension event payloads (#553, #573).
pub const TTL_EXTENDED_DATA_FIELDS: [&str; 3] =
    ["creator_id", "extended_at_ledger", "new_expiry_ledger"];

/// Stable TTL extension event payload for downstream indexers and monitoring tools (#553, #573).
///
/// Event shape:
/// - topics: `(TTL_EXTENDED_EVENT_NAME, creator)`
/// - data: `TtlExtendedEvent`
///
/// Emitted when the contract extends a creator key's persistent storage TTL.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TtlExtendedEvent {
    /// Creator key whose storage TTL was extended.
    pub creator_id: Address,
    /// Ledger sequence number at the time of TTL extension.
    pub extended_at_ledger: u32,
    /// New live-until ledger sequence number after extension.
    pub new_expiry_ledger: u32,
}

/// Stable field order for treasury withdrawal event payloads.
pub const TREASURY_WITHDRAWAL_DATA_FIELDS: [&str; 4] =
    ["amount", "recipient", "remaining_balance", "ledger"];

/// Stable treasury withdrawal event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(TREASURY_WITHDRAWAL_EVENT_NAME, recipient)`
/// - data: `TreasuryWithdrawalEvent`
///
/// `ledger` is the Soroban ledger sequence number at the time of withdrawal so
/// off-chain indexers can reconstruct the timeline without replaying all events.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TreasuryWithdrawalEvent {
    pub amount: i128,
    pub recipient: Address,
    pub remaining_balance: i128,
    pub ledger: u32,
}

/// Shared treasury withdrawal event topics tuple.
pub fn treasury_withdrawal_event_topics(recipient: &Address) -> (Symbol, Address) {
    (TREASURY_WITHDRAWAL_EVENT_NAME, recipient.clone())
}

/// Event name for reward pool top-up.
pub const REWARD_POOL_TOPUP_EVENT_NAME: Symbol = symbol_short!("rwd_top");

/// Stable field order for reward pool top-up event payloads.
pub const REWARD_POOL_TOPUP_DATA_FIELDS: [&str; 3] = ["sender", "amount", "new_pool_balance"];

/// Stable reward pool top-up event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(REWARD_POOL_TOPUP_EVENT_NAME, sender)`
/// - data: `RewardPoolTopUpEvent`
///
/// Emitted when the authorised fee router calls `topup_reward_pool`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RewardPoolTopUpEvent {
    /// Address of the fee router that sent the top-up.
    pub sender: Address,
    /// Amount added to the pool in this call (stroops).
    pub amount: i128,
    /// New total reward pool balance after the top-up (stroops).
    pub new_pool_balance: i128,
}

/// Shared reward pool top-up event topics tuple.
pub fn reward_pool_topup_topics(sender: &Address) -> (Symbol, Address) {
    (REWARD_POOL_TOPUP_EVENT_NAME, sender.clone())
}

/// Event name for bid-ask spread update.
pub const SPREAD_UPDATED_EVENT_NAME: Symbol = symbol_short!("sprd_upd");

/// Stable field order for spread updated event payloads.
pub const SPREAD_UPDATED_DATA_FIELDS: [&str; 3] = ["creator", "old_spread_bps", "new_spread_bps"];

/// Stable spread updated event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(SPREAD_UPDATED_EVENT_NAME, creator)`
/// - data: `SpreadUpdatedEvent`
///
/// Emitted when the admin updates the bid-ask spread for a creator.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SpreadUpdatedEvent {
    /// Creator whose spread was changed.
    pub creator: Address,
    /// Previous spread in basis points.
    pub old_spread_bps: u32,
    /// New spread in basis points.
    pub new_spread_bps: u32,
}

/// Shared spread updated event topics tuple.
pub fn spread_updated_topics(creator: &Address) -> (Symbol, Address) {
    (SPREAD_UPDATED_EVENT_NAME, creator.clone())
}

/// Shared TTL extension event topics tuple.
pub fn ttl_extended_topics(creator: &Address) -> (Symbol, Address) {
    (TTL_EXTENDED_EVENT_NAME, creator.clone())
}

// --- Supply cap events ---

/// Event name for supply cap set.
pub const SUPPLY_CAP_SET_EVENT_NAME: Symbol = symbol_short!("cap_set");

/// Event shape:
/// - topics: `(SUPPLY_CAP_SET_EVENT_NAME, creator)`
/// - data: `SupplyCapSetEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SupplyCapSetEvent {
    pub creator_id: Address,
    pub cap: u32,
}

pub fn supply_cap_set_topics(creator: &Address) -> (Symbol, Address) {
    (SUPPLY_CAP_SET_EVENT_NAME, creator.clone())
}

/// Event name emitted exactly once when a buy fills a capped key's supply to
/// its configured cap. Subsequent buys revert with `SupplyCapExceeded`, so the
/// event is never emitted again for the same key.
pub const SUPPLY_CAP_REACHED_EVENT_NAME: Symbol = symbol_short!("cap_reach");

/// Stable supply-cap-reached event payload.
///
/// Event shape:
/// - topics: `(SUPPLY_CAP_REACHED_EVENT_NAME, creator_id)`
/// - data: `SupplyCapReachedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SupplyCapReachedEvent {
    /// Creator whose key supply just reached the configured cap.
    pub creator_id: Address,
    /// New total supply, which now equals the configured cap.
    pub new_supply: u32,
    /// The configured cap that was reached.
    pub cap: u32,
    /// Ledger in which the cap was reached.
    pub ledger: u32,
}

/// Shared supply-cap-reached event topics tuple.
pub fn supply_cap_reached_topics(creator: &Address) -> (Symbol, Address) {
    (SUPPLY_CAP_REACHED_EVENT_NAME, creator.clone())
}

// --- Multisig pause events ---

/// Event name for pause proposal.
pub const PAUSE_PROPOSED_EVENT_NAME: Symbol = symbol_short!("pp_prop");

/// Event name for trading paused via multisig.
pub const TRADING_PAUSED_EVENT_NAME: Symbol = symbol_short!("pp_exec");

/// Event shape:
/// - topics: `(PAUSE_PROPOSED_EVENT_NAME, creator)`
/// - data: `PauseProposedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PauseProposedEvent {
    pub creator_id: Address,
    pub proposer: Address,
    pub ledger: u32,
}

/// Event shape:
/// - topics: `(TRADING_PAUSED_EVENT_NAME, creator)`
/// - data: `TradingPausedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TradingPausedEvent {
    pub creator_id: Address,
    pub approver: Address,
    pub ledger: u32,
}

pub fn pause_proposed_topics(creator: &Address) -> (Symbol, Address) {
    (PAUSE_PROPOSED_EVENT_NAME, creator.clone())
}

pub fn trading_paused_topics(creator: &Address) -> (Symbol, Address) {
    (TRADING_PAUSED_EVENT_NAME, creator.clone())
}

/// Event name for a key trading pause with a fixed expiry.
pub const PAUSE_EXPIRY_SET_EVENT_NAME: Symbol = symbol_short!("pp_exp");

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PauseExpirySetEvent {
    pub key_id: Address,
    pub pause_expires_at: u32,
}

pub fn pause_expiry_set_topics(key_id: &Address) -> (Symbol, Address) {
    (PAUSE_EXPIRY_SET_EVENT_NAME, key_id.clone())
}

// --- Global emergency pause events (#784) ---

/// Event name emitted when the protocol-wide emergency pause activates.
pub const GLOBAL_PAUSE_ACTIVATED_EVENT_NAME: Symbol = symbol_short!("gpause_on");

/// Event name emitted when the protocol-wide emergency pause is lifted.
pub const GLOBAL_PAUSE_LIFTED_EVENT_NAME: Symbol = symbol_short!("gpause_of");

/// Topics for the `global_pause_activated` event.
///
/// - topics: `(GLOBAL_PAUSE_ACTIVATED_EVENT_NAME, approver)`
/// - data: the ledger sequence at activation (`u32`)
pub fn global_pause_activated_topics(approver: &Address) -> (Symbol, Address) {
    (GLOBAL_PAUSE_ACTIVATED_EVENT_NAME, approver.clone())
}

/// Topics for the `global_pause_lifted` event.
///
/// - topics: `(GLOBAL_PAUSE_LIFTED_EVENT_NAME, approver)`
/// - data: the ledger sequence at the lift (`u32`)
pub fn global_pause_lifted_topics(approver: &Address) -> (Symbol, Address) {
    (GLOBAL_PAUSE_LIFTED_EVENT_NAME, approver.clone())
}

// --- Vesting events ---

/// Event name for vesting schedule created.
pub const VESTING_CREATED_EVENT_NAME: Symbol = symbol_short!("vest_new");

/// Event name for vested keys claimed.
pub const KEYS_CLAIMED_EVENT_NAME: Symbol = symbol_short!("vest_clm");

/// Event shape:
/// - topics: `(VESTING_CREATED_EVENT_NAME, creator)`
/// - data: `VestingCreatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestingCreatedEvent {
    pub creator_id: Address,
    pub beneficiary: Address,
    pub total_keys: u32,
    pub start_ledger: u32,
    pub vesting_period_ledgers: u32,
}

/// Event shape:
/// - topics: `(KEYS_CLAIMED_EVENT_NAME, creator, beneficiary)`
/// - data: `KeysClaimedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysClaimedEvent {
    pub creator_id: Address,
    pub beneficiary: Address,
    pub amount: u32,
    pub ledger: u32,
}

pub fn vesting_created_topics(creator: &Address) -> (Symbol, Address) {
    (VESTING_CREATED_EVENT_NAME, creator.clone())
}

pub fn keys_claimed_topics(creator: &Address, beneficiary: &Address) -> (Symbol, Address, Address) {
    (
        KEYS_CLAIMED_EVENT_NAME,
        creator.clone(),
        beneficiary.clone(),
    )
}

pub const VESTING_CLIFF_CLAIMED_EVENT_NAME: Symbol = symbol_short!("vc_claim");

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestingCliffClaimedEvent {
    pub creator_id: Address,
    pub beneficiary: Address,
    pub amount: i128,
    pub ledger: u32,
}

pub fn vesting_cliff_claimed_topics(
    creator: &Address,
    beneficiary: &Address,
) -> (Symbol, Address, Address) {
    (
        VESTING_CLIFF_CLAIMED_EVENT_NAME,
        creator.clone(),
        beneficiary.clone(),
    )
}

// --- Timelock events ---

/// Event name for config change proposed.
pub const CONFIG_CHANGE_PROPOSED_EVENT_NAME: Symbol = symbol_short!("tl_prop");

/// Event name for config change executed.
pub const CONFIG_CHANGE_EXECUTED_EVENT_NAME: Symbol = symbol_short!("tl_exec");

/// Event name for config change cancelled.
pub const CONFIG_CHANGE_CANCELLED_EVENT_NAME: Symbol = symbol_short!("tl_canc");

/// Helper function to generate event topics for reputation updates
pub fn reputation_updated_topics(creator: &Address) -> (Symbol, Address) {
    (REPUTATION_UPDATED_EVENT_NAME.clone(), creator.clone())
}

/// Event shape:
/// - topics: `(CONFIG_CHANGE_PROPOSED_EVENT_NAME, proposer)`
/// - data: `ConfigChangeProposedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ConfigChangeProposedEvent {
    pub proposal_id: u32,
    pub proposer: Address,
    pub change_type: u32,
    pub proposed_at: u32,
    pub execution_not_before: u32,
}

/// Event shape:
/// - topics: `(CONFIG_CHANGE_EXECUTED_EVENT_NAME,)`
/// - data: `ConfigChangeExecutedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ConfigChangeExecutedEvent {
    pub proposal_id: u32,
    pub executed_at: u32,
}

/// Event shape:
/// - topics: `(CONFIG_CHANGE_CANCELLED_EVENT_NAME,)`
/// - data: `ConfigChangeCancelledEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ConfigChangeCancelledEvent {
    pub proposal_id: u32,
    pub cancelled_at: u32,
}

pub fn config_change_proposed_topics(proposer: &Address) -> (Symbol, Address) {
    (CONFIG_CHANGE_PROPOSED_EVENT_NAME, proposer.clone())
}

pub fn config_change_executed_topics() -> Symbol {
    CONFIG_CHANGE_EXECUTED_EVENT_NAME
}

pub fn config_change_cancelled_topics() -> Symbol {
    CONFIG_CHANGE_CANCELLED_EVENT_NAME
}

// --- Circuit breaker, referral fee, whitelist, burn events ---

pub const CIRCUIT_BREAKER_TRIGGERED_EVENT_NAME: Symbol = symbol_short!("cb_trig");
pub const REFERRAL_FEE_PAID_EVENT_NAME: Symbol = symbol_short!("ref_paid");
pub const WHITELIST_ENABLED_EVENT_NAME: Symbol = symbol_short!("wl_en");
pub const WHITELIST_DISABLED_EVENT_NAME: Symbol = symbol_short!("wl_dis");
pub const ADDRESS_WHITELISTED_EVENT_NAME: Symbol = symbol_short!("wl_add");
pub const ADDRESS_REMOVED_EVENT_NAME: Symbol = symbol_short!("wl_rem");
pub const WALLET_WHITELISTED_EVENT_NAME: Symbol = symbol_short!("wl_wadd");
pub const WALLET_REMOVED_FROM_WHITELIST_EVENT_NAME: Symbol = symbol_short!("wl_wrem");
pub const WHITELIST_MODE_SET_EVENT_NAME: Symbol = symbol_short!("wl_mode");
pub const HOLDING_CAP_UPDATED_EVENT_NAME: Symbol = symbol_short!("hold_cap");
pub const WHITELIST_UPDATED_EVENT_NAME: Symbol = symbol_short!("wl_upd");
pub const EARLY_ACCESS_MODE_CHANGED_EVENT_NAME: Symbol = symbol_short!("ea_mode");
pub const REFERRAL_REGISTERED_EVENT_NAME: Symbol = symbol_short!("ref_reg");
pub const REFERRAL_REWARD_ALLOCATED_EVENT_NAME: Symbol = symbol_short!("ref_rwd");
pub const REFERRAL_REWARDS_CLAIMED_EVENT_NAME: Symbol = symbol_short!("ref_clm");
pub const REFERRAL_TIER_UPGRADED_EVENT_NAME: Symbol = symbol_short!("tier_upg");
pub const KEYS_BURNED_EVENT_NAME: Symbol = symbol_short!("burned");
pub const SELF_FREEZE_APPLIED_EVENT_NAME: Symbol = symbol_short!("sf_add");
pub const SELF_FREEZE_LIFTED_EVENT_NAME: Symbol = symbol_short!("sf_del");

/// Event shape:
/// - topics: `(SELF_FREEZE_APPLIED_EVENT_NAME, key_id, wallet)`
/// - topics: `(SELF_FREEZE_LIFTED_EVENT_NAME, key_id, wallet)`
/// - data: `SelfFreezeEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SelfFreezeEvent {
    pub key_id: Address,
    pub wallet: Address,
    pub quantity: u32,
}

/// Event shape:
/// - topics: `(CIRCUIT_BREAKER_TRIGGERED_EVENT_NAME,)`
/// - data: `CircuitBreakerTriggeredEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CircuitBreakerTriggeredEvent {
    pub pre_price: i128,
    pub post_price: i128,
    pub actual_bps: u32,
}

pub fn circuit_breaker_triggered_topics() -> Symbol {
    CIRCUIT_BREAKER_TRIGGERED_EVENT_NAME
}

/// Event shape:
/// - topics: `(REFERRAL_FEE_PAID_EVENT_NAME,)`
/// - data: `ReferralFeePaidEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralFeePaidEvent {
    pub referrer: Address,
    pub amount: i128,
}

pub fn referral_fee_paid_topics() -> Symbol {
    REFERRAL_FEE_PAID_EVENT_NAME
}

/// Event shape:
/// - topics: `(WHITELIST_ENABLED_EVENT_NAME, creator)`
/// - data: `WhitelistEnabledEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct WhitelistEnabledEvent {
    pub creator: Address,
}

pub fn whitelist_enabled_topics(creator: &Address) -> (Symbol, Address) {
    (WHITELIST_ENABLED_EVENT_NAME, creator.clone())
}

/// Event shape:
/// - topics: `(WHITELIST_DISABLED_EVENT_NAME, creator)`
/// - data: `WhitelistDisabledEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct WhitelistDisabledEvent {
    pub creator: Address,
}

pub fn whitelist_disabled_topics(creator: &Address) -> (Symbol, Address) {
    (WHITELIST_DISABLED_EVENT_NAME, creator.clone())
}

/// Event shape:
/// - topics: `(ADDRESS_WHITELISTED_EVENT_NAME, creator)`
/// - data: `AddressWhitelistedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AddressWhitelistedEvent {
    pub creator: Address,
    pub address: Address,
}

pub fn address_whitelisted_topics(creator: &Address) -> (Symbol, Address) {
    (ADDRESS_WHITELISTED_EVENT_NAME, creator.clone())
}

/// Event shape:
/// - topics: `(ADDRESS_REMOVED_EVENT_NAME, creator)`
/// - data: `AddressRemovedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AddressRemovedEvent {
    pub creator: Address,
    pub address: Address,
}

pub fn address_removed_topics(creator: &Address) -> (Symbol, Address) {
    (ADDRESS_REMOVED_EVENT_NAME, creator.clone())
}

/// Emitted when a creator changes their per-wallet holding cap.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct HoldingCapUpdatedEvent {
    pub creator: Address,
    pub old_cap: Option<u32>,
    pub new_cap: u32,
}

pub fn holding_cap_updated_topics(creator: &Address) -> (Symbol, Address) {
    (HOLDING_CAP_UPDATED_EVENT_NAME, creator.clone())
}

/// Event name emitted when a creator tightens their per-wallet percentage
/// holding cap (`holder_cap_bps`) via `update_holder_cap`.
///
/// The cap can only be reduced, never increased beyond the value stored at
/// launch.
pub const HOLDER_CAP_UPDATED_EVENT_NAME: Symbol = symbol_short!("hcap_upd");

/// Payload emitted when a creator updates their per-wallet percentage holding
/// cap via `update_holder_cap`.
///
/// Event shape:
/// - topics: `(HOLDER_CAP_UPDATED_EVENT_NAME, key_id)`
/// - data: `HolderCapUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct HolderCapUpdatedEvent {
    /// Creator (key) whose holder cap was updated.
    pub key_id: Address,
    /// Previous holder cap in basis points.
    pub old_cap_bps: u32,
    /// New holder cap in basis points.
    pub new_cap_bps: u32,
}

/// Shared holder-cap-updated event topics tuple.
pub fn holder_cap_updated_topics(key_id: &Address) -> (Symbol, Address) {
    (HOLDER_CAP_UPDATED_EVENT_NAME, key_id.clone())
}

// --- Price alerts (#1030) ---

/// Event name emitted when a wallet registers a price alert via `register_alert`.
pub const PRICE_ALERT_REGISTERED_EVENT_NAME: Symbol = symbol_short!("alert_reg");

/// Stable price-alert-registered event field order for downstream indexers.
pub const PRICE_ALERT_REGISTERED_EVENT_DATA_FIELDS: [&str; 6] = [
    "wallet",
    "key_id",
    "alert_id",
    "direction",
    "target_price",
    "ledger",
];

/// Payload emitted when a wallet registers a price alert.
///
/// Event shape:
/// - topics: `(PRICE_ALERT_REGISTERED_EVENT_NAME, key_id, wallet)`
/// - data: `PriceAlertRegisteredEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceAlertRegisteredEvent {
    /// Wallet that registered the alert.
    pub wallet: Address,
    /// Key whose spot price is watched.
    pub key_id: Address,
    /// Id assigned to the alert.
    pub alert_id: u64,
    /// Crossing direction that fires the alert.
    pub direction: crate::price_alerts::AlertDirection,
    /// Spot price threshold.
    pub target_price: i128,
    /// Ledger in which the alert was registered.
    pub ledger: u32,
}

/// Shared price-alert-registered event topics tuple.
pub fn price_alert_registered_topics(
    key_id: &Address,
    wallet: &Address,
) -> (Symbol, Address, Address) {
    (
        PRICE_ALERT_REGISTERED_EVENT_NAME,
        key_id.clone(),
        wallet.clone(),
    )
}

/// Event name emitted when a buy or sell moves a key's spot price across a
/// registered alert's target. Each alert emits it at most once: the alert is
/// deactivated in the same call.
pub const PRICE_ALERT_TRIGGERED_EVENT_NAME: Symbol = symbol_short!("alert_trg");

/// Stable price-alert-triggered event field order for downstream indexers.
pub const PRICE_ALERT_TRIGGERED_EVENT_DATA_FIELDS: [&str; 7] = [
    "wallet",
    "key_id",
    "alert_id",
    "direction",
    "target_price",
    "new_price",
    "ledger",
];

/// Payload emitted when a price alert fires.
///
/// Event shape:
/// - topics: `(PRICE_ALERT_TRIGGERED_EVENT_NAME, key_id, wallet)`
/// - data: `PriceAlertTriggeredEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceAlertTriggeredEvent {
    /// Wallet that registered the alert.
    pub wallet: Address,
    /// Key whose spot price crossed the target.
    pub key_id: Address,
    /// Id of the alert that fired.
    pub alert_id: u64,
    /// Crossing direction of the alert.
    pub direction: crate::price_alerts::AlertDirection,
    /// The alert's spot price threshold.
    pub target_price: i128,
    /// Spot price after the trade that crossed the target.
    pub new_price: i128,
    /// Ledger of the trade that fired the alert.
    pub ledger: u32,
}

/// Shared price-alert-triggered event topics tuple.
pub fn price_alert_triggered_topics(
    key_id: &Address,
    wallet: &Address,
) -> (Symbol, Address, Address) {
    (
        PRICE_ALERT_TRIGGERED_EVENT_NAME,
        key_id.clone(),
        wallet.clone(),
    )
}

/// Event name emitted when a wallet cancels a price alert via `cancel_alert`.
pub const PRICE_ALERT_CANCELLED_EVENT_NAME: Symbol = symbol_short!("alert_cxl");

/// Payload emitted when a wallet cancels a price alert.
///
/// Event shape:
/// - topics: `(PRICE_ALERT_CANCELLED_EVENT_NAME, key_id, wallet)`
/// - data: `PriceAlertCancelledEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceAlertCancelledEvent {
    /// Wallet that cancelled the alert.
    pub wallet: Address,
    /// Key the alert watched.
    pub key_id: Address,
    /// Id of the cancelled alert.
    pub alert_id: u64,
    /// Ledger in which the alert was cancelled.
    pub ledger: u32,
}

/// Shared price-alert-cancelled event topics tuple.
pub fn price_alert_cancelled_topics(
    key_id: &Address,
    wallet: &Address,
) -> (Symbol, Address, Address) {
    (
        PRICE_ALERT_CANCELLED_EVENT_NAME,
        key_id.clone(),
        wallet.clone(),
    )
}

/// Emitted on every early-access whitelist add (`allowed = true`) and remove.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct WhitelistUpdatedEvent {
    pub creator: Address,
    pub wallet: Address,
    pub allowed: bool,
}

pub fn whitelist_updated_topics(creator: &Address) -> (Symbol, Address) {
    (WHITELIST_UPDATED_EVENT_NAME, creator.clone())
}

/// Emitted when a creator toggles early-access mode.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EarlyAccessModeChangedEvent {
    pub creator: Address,
    pub enabled: bool,
}

pub fn early_access_mode_changed_topics(creator: &Address) -> (Symbol, Address) {
    (EARLY_ACCESS_MODE_CHANGED_EVENT_NAME, creator.clone())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralRegisteredEvent {
    pub referee: Address,
    pub referrer: Address,
}

pub fn referral_registered_topics() -> Symbol {
    REFERRAL_REGISTERED_EVENT_NAME
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralRewardAllocatedEvent {
    pub referee: Address,
    pub referrer: Address,
    pub amount: i128,
}

pub fn referral_reward_allocated_topics() -> Symbol {
    REFERRAL_REWARD_ALLOCATED_EVENT_NAME
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralRewardsClaimedEvent {
    pub referrer: Address,
    pub amount: i128,
}

pub fn referral_rewards_claimed_topics() -> Symbol {
    REFERRAL_REWARDS_CLAIMED_EVENT_NAME
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralTierUpgradedEvent {
    pub referrer: Address,
    pub old_tier: u32,
    pub new_tier: u32,
    pub volume: i128,
    pub reward_rate_bps: u32,
}

pub fn referral_tier_upgraded_topics(referrer: &Address) -> (Symbol, Address) {
    (REFERRAL_TIER_UPGRADED_EVENT_NAME, referrer.clone())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysBurnedEvent {
    pub wallet: Address,
    pub key_id: Address,
    pub quantity: u32,
    pub new_supply: u32,
}

pub fn keys_burned_topics(key_id: &Address) -> (Symbol, Address) {
    (KEYS_BURNED_EVENT_NAME, key_id.clone())
}

/// Event shape:
/// - topics: `(QUORUM_UPDATED_EVENT_NAME, creator)`
/// - data: `QuorumUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct QuorumUpdatedEvent {
    pub creator: Address,
    pub quorum_bps: u32,
    pub ledger: u32,
}

pub fn quorum_updated_topics(creator: &Address) -> (Symbol, Address) {
    (QUORUM_UPDATED_EVENT_NAME, creator.clone())
}

/// Event shape:
/// - topics: `(POLL_CLOSED_EVENT_NAME, creator, poll_id)`
/// - data: `PollClosedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PollClosedEvent {
    pub creator_id: Address,
    pub poll_id: u32,
    pub total_weight: u32,
    /// Whether participation actually met the creator's `quorum_bps`.
    ///
    /// This stays `false` for a close that was only permitted because quorum
    /// escalation had no extensions left, so off-chain consumers can tell a
    /// genuine quorum from a deadline finalization.
    pub quorum_reached: bool,
    /// Whether this close was permitted purely by an exhausted extension budget.
    pub finalized_by_exhaustion: bool,
    pub ledger: u32,
}

pub fn poll_closed_topics(creator: &Address, poll_id: u32) -> (Symbol, Address, u32) {
    (POLL_CLOSED_EVENT_NAME, creator.clone(), poll_id)
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PollError {
    NotRegistered = 20,
    Overflow = 21,
    InvalidOptionCount = 22,
    QuestionTooLong = 23,
    OptionTooLong = 24,
    PollNotFound = 25,
    PollExpired = 26,
    NotAHolder = 27,
    InvalidOption = 28,
    QuorumNotReached = 29,
    QuorumTooHigh = 30,
    QuorumTooLow = 31,
    Unauthorized = 32,
    AlreadyClosed = 33,
}

#[derive(Clone)]
#[contracttype]
pub enum PollDataKey {
    NextPollId(Address),
    Poll(Address, u32),
    Vote(Address, u32, Address),
    /// (creator, poll_id) -> number of quorum-escalation extensions consumed.
    ExtensionCount(Address, u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Poll {
    pub question: String,
    pub options: Vec<String>,
    pub vote_counts: Vec<u32>,
    pub total_weight: u32,
    pub expires_at: u32,
    pub closed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PollVote {
    pub option_index: u32,
    pub weight: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PollResult {
    pub question: String,
    pub options: Vec<String>,
    pub vote_counts: Vec<u32>,
    pub total_weight: u32,
    pub expired: bool,
    pub closed: bool,
}

pub fn poll_storage_key(creator_id: &Address, poll_id: u32) -> PollDataKey {
    PollDataKey::Poll(creator_id.clone(), poll_id)
}

pub fn vote_storage_key(creator_id: &Address, poll_id: u32, voter: &Address) -> PollDataKey {
    PollDataKey::Vote(creator_id.clone(), poll_id, voter.clone())
}

/// Storage key for a poll's consumed quorum-escalation extension count.
pub fn poll_extension_count_key(creator_id: &Address, poll_id: u32) -> PollDataKey {
    PollDataKey::ExtensionCount(creator_id.clone(), poll_id)
}

/// Reads the number of quorum-escalation extensions a poll has consumed.
///
/// A poll that has never been evaluated returns `0`.
pub fn read_poll_extension_count(env: &Env, creator_id: &Address, poll_id: u32) -> u32 {
    env.storage()
        .persistent()
        .get(&poll_extension_count_key(creator_id, poll_id))
        .unwrap_or(0)
}

/// Persists a poll's consumed quorum-escalation extension count.
pub fn write_poll_extension_count(env: &Env, creator_id: &Address, poll_id: u32, count: u32) {
    let key = poll_extension_count_key(creator_id, poll_id);
    env.storage().persistent().set(&key, &count);
    extend_key_ttl_to_full_window(env, &key);
}

/// Returns `true` when a poll has consumed every extension the active
/// escalation config allows.
///
/// A `false` here means the proposal can still be extended, so closing it below
/// quorum would cut off a vote that quorum escalation is meant to rescue.
/// Conversely a `true` here makes the proposal final: it closes on its own
/// deadline even if participation never reached quorum.
///
/// When escalation is disabled — no config, or a `max_extensions` of `0` — this
/// returns `false`, which keeps [`close_poll`]'s quorum requirement exactly as it
/// behaved before escalation existed. Reporting "exhausted" there would instead
/// let a creator with a configured `quorum_bps` close any failing proposal.
pub fn poll_extensions_exhausted(env: &Env, creator_id: &Address, poll_id: u32) -> bool {
    let max_extensions = crate::CreatorKeysContract::get_escalation_config(env.clone())
        .map(|c| c.max_extensions)
        .unwrap_or(0);
    if max_extensions == 0 {
        return false;
    }
    read_poll_extension_count(env, creator_id, poll_id) >= max_extensions
}

pub fn read_poll(env: &Env, creator_id: &Address, poll_id: u32) -> Result<Poll, PollError> {
    env.storage()
        .persistent()
        .get(&poll_storage_key(creator_id, poll_id))
        .ok_or(PollError::PollNotFound)
}

/// Persists a poll record and refreshes its TTL.
pub fn write_poll(env: &Env, creator_id: &Address, poll_id: u32, poll: &Poll) {
    let key = poll_storage_key(creator_id, poll_id);
    env.storage().persistent().set(&key, poll);
    extend_key_ttl_to_full_window(env, &key);
}

pub fn is_poll_expired(env: &Env, poll: &Poll) -> bool {
    env.ledger().sequence() >= poll.expires_at
}

fn validate_poll_options(options: &Vec<String>) -> Result<(), PollError> {
    let option_count = options.len();
    if !(MIN_POLL_OPTIONS..=MAX_POLL_OPTIONS).contains(&option_count) {
        return Err(PollError::InvalidOptionCount);
    }

    let mut index = 0;
    while index < option_count {
        let option = options.get(index).ok_or(PollError::InvalidOption)?;
        if option.len() > MAX_OPTION_CHARS {
            return Err(PollError::OptionTooLong);
        }
        index += 1;
    }

    Ok(())
}

#[contractimpl]
impl CreatorKeysContract {
    /// Creates a creator-owned governance poll with two to four options.
    ///
    /// The creator address must authorize the call. Polls expire at the current ledger
    /// sequence plus `duration_ledgers`, and the returned `poll_id` is scoped to the creator.
    pub fn create_poll(
        env: Env,
        creator_id: Address,
        question: String,
        options: Vec<String>,
        duration_ledgers: u32,
    ) -> Result<u32, PollError> {
        creator_id.require_auth();
        read_registered_creator_profile(&env, &creator_id).map_err(|_| PollError::NotRegistered)?;

        if question.len() > MAX_QUESTION_CHARS {
            return Err(PollError::QuestionTooLong);
        }
        validate_poll_options(&options)?;

        let mut vote_counts = Vec::new(&env);
        let mut index = 0;
        while index < options.len() {
            vote_counts.push_back(0);
            index += 1;
        }

        let next_key = PollDataKey::NextPollId(creator_id.clone());
        let poll_id: u32 = env.storage().persistent().get(&next_key).unwrap_or(1);
        let next_poll_id = poll_id.checked_add(1).ok_or(PollError::Overflow)?;
        let expires_at = env
            .ledger()
            .sequence()
            .checked_add(duration_ledgers)
            .ok_or(PollError::Overflow)?;

        let poll = Poll {
            question,
            options,
            vote_counts,
            total_weight: 0,
            expires_at,
            closed: false,
        };

        env.storage()
            .persistent()
            .set(&poll_storage_key(&creator_id, poll_id), &poll);
        env.storage().persistent().set(&next_key, &next_poll_id);
        env.events().publish(
            (POLL_CREATED_EVENT_NAME, creator_id.clone(), poll_id),
            poll.expires_at,
        );

        Ok(poll_id)
    }

    /// Casts or updates a weighted vote for a creator poll.
    ///
    /// The voter must authorize the call and must currently hold at least one liquid key for
    /// the creator. Re-voting before expiry removes the previous weight and adds the voter's
    /// current liquid key balance to the selected option.
    pub fn cast_vote(
        env: Env,
        creator_id: Address,
        voter: Address,
        poll_id: u32,
        option_index: u32,
    ) -> Result<(), PollError> {
        voter.require_auth();
        let mut poll = read_poll(&env, &creator_id, poll_id)?;

        if poll.closed {
            return Err(PollError::AlreadyClosed);
        }
        if is_poll_expired(&env, &poll) {
            return Err(PollError::PollExpired);
        }
        if option_index >= poll.options.len() {
            return Err(PollError::InvalidOption);
        }

        let delegate_key = constants::storage::delegate(&creator_id, &voter);
        if env.storage().persistent().has(&delegate_key)
            || crate::vote_delegation::has_active_delegation(&env, &creator_id, &voter)
        {
            return Err(PollError::Unauthorized);
        }

        let weight: u32 = crate::vote_delegation::get_voting_weight(&env, &voter, &creator_id);
        if weight == 0 {
            return Err(PollError::NotAHolder);
        }

        let vote_key = vote_storage_key(&creator_id, poll_id, &voter);
        if let Some(previous_vote) = env
            .storage()
            .persistent()
            .get::<PollDataKey, PollVote>(&vote_key)
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
            .set(&poll_storage_key(&creator_id, poll_id), &poll);
        env.storage().persistent().set(
            &vote_key,
            &PollVote {
                option_index,
                weight,
            },
        );
        env.events().publish(
            (POLL_VOTE_EVENT_NAME, creator_id.clone(), poll_id, voter),
            (option_index, weight),
        );

        // Voting is a positive reputation signal for the creator whose key
        // holders are exercising governance rights.
        crate::accrue_reputation_on_governance_participation(&env, &creator_id)
            .map_err(|_| PollError::Overflow)?;

        Ok(())
    }

    /// Returns the current weighted result for a creator poll.
    pub fn get_poll_result(
        env: Env,
        creator_id: Address,
        poll_id: u32,
    ) -> Result<PollResult, PollError> {
        let poll = read_poll(&env, &creator_id, poll_id)?;
        let expired = is_poll_expired(&env, &poll);
        Ok(PollResult {
            question: poll.question,
            options: poll.options,
            vote_counts: poll.vote_counts,
            total_weight: poll.total_weight,
            expired,
            closed: poll.closed,
        })
    }

    /// Closes a creator poll if the configured quorum threshold has been reached.
    ///
    /// Computes participation as `total_voting_weight / circulating_supply` (in basis points)
    /// against the creator's configured `quorum_bps`. If participation is below the quorum
    /// threshold, the poll closes only once quorum escalation has no extensions left to give —
    /// see [`poll_extensions_exhausted`]. Otherwise returns
    /// `Err(PollError::QuorumNotReached)`.
    ///
    /// [`PollClosedEvent::quorum_reached`] always reports the real participation
    /// outcome; a close that was only permitted because the extension budget ran
    /// out is reported by [`PollClosedEvent::finalized_by_exhaustion`] instead.
    pub fn close_poll(
        env: Env,
        creator_id: Address,
        poll_id: u32,
    ) -> Result<PollResult, PollError> {
        let mut poll = read_poll(&env, &creator_id, poll_id)?;

        if poll.closed {
            return Err(PollError::AlreadyClosed);
        }

        let circulating_supply = read_creator_supply(&env, &creator_id);
        let quorum_key = constants::storage::quorum_bps(&creator_id);
        let quorum_bps: u32 = env.storage().persistent().get(&quorum_key).unwrap_or(0);

        // A creator with no configured quorum has no participation requirement,
        // so the poll is always closable and has genuinely "reached" quorum.
        let mut quorum_reached = true;
        let mut finalized_by_exhaustion = false;
        if quorum_bps > 0 {
            // With no circulating supply, participation is undefined. A poll whose
            // extensions are spent still has to close; one with budget left does not.
            let participation_bps = if circulating_supply == 0 {
                0
            } else {
                let total_weight_bps = (poll.total_weight as u128)
                    .checked_mul(10_000)
                    .ok_or(PollError::Overflow)?;
                let required_bps = (circulating_supply as u128)
                    .checked_mul(quorum_bps as u128)
                    .ok_or(PollError::Overflow)?;

                if total_weight_bps >= required_bps {
                    10_000
                } else {
                    ((total_weight_bps * 10_000) / required_bps) as u32
                }
            };

            quorum_reached = participation_bps >= 10_000;
            if !quorum_reached {
                finalized_by_exhaustion = poll_extensions_exhausted(&env, &creator_id, poll_id);
                if !finalized_by_exhaustion {
                    return Err(PollError::QuorumNotReached);
                }
            }
        }

        poll.closed = true;
        write_poll(&env, &creator_id, poll_id, &poll);

        env.events().publish(
            poll_closed_topics(&creator_id, poll_id),
            PollClosedEvent {
                creator_id: creator_id.clone(),
                poll_id,
                total_weight: poll.total_weight,
                quorum_reached,
                finalized_by_exhaustion,
                ledger: env.ledger().sequence(),
            },
        );

        let expired = is_poll_expired(&env, &poll);
        Ok(PollResult {
            question: poll.question,
            options: poll.options,
            vote_counts: poll.vote_counts,
            total_weight: poll.total_weight,
            expired,
            closed: true,
        })
    }

    /// Alias for `close_poll`.
    pub fn close_proposal(
        env: Env,
        creator_id: Address,
        poll_id: u32,
    ) -> Result<PollResult, PollError> {
        Self::close_poll(env, creator_id, poll_id)
    }
}

/// Event name for batch buy completion.
pub const BATCH_BUY_COMPLETED_EVENT_NAME: Symbol = symbol_short!("bat_buy");

/// Stable batch buy completed event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(BATCH_BUY_COMPLETED_EVENT_NAME, buyer)`
/// - data: `BatchBuyCompletedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BatchBuyCompletedEvent {
    pub buyer: Address,
    pub total_price_paid: i128,
    pub order_count: u32,
    pub ledger: u32,
}

/// Shared batch buy completed event topics tuple.
pub fn batch_buy_completed_topics(buyer: &Address) -> (Symbol, Address) {
    (BATCH_BUY_COMPLETED_EVENT_NAME, buyer.clone())
}

/// Event name for an atomic bundle purchase (Issue #1057).
pub const BUNDLE_EXECUTED_EVENT_NAME: Symbol = symbol_short!("bundle");

/// Per-item line of a [`BundleExecutedEvent`].
///
/// `price_paid` is the gross cost of `amount` keys of `key_id`, i.e. the same
/// figure `KeysBoughtEvent::price_paid` reports for a stand-alone buy.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BundleItemBreakdown {
    pub key_id: Address,
    pub amount: u32,
    pub price_paid: i128,
}

/// Stable bundle-executed event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(BUNDLE_EXECUTED_EVENT_NAME, buyer)`
/// - data: `BundleExecutedEvent`
///
/// Emitted exactly once, only after **every** item in the bundle succeeded.
/// `total_cost` always equals the sum of `items[*].price_paid`, and `items`
/// preserves the order the buyer submitted.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BundleExecutedEvent {
    pub buyer: Address,
    pub items: Vec<BundleItemBreakdown>,
    pub total_cost: i128,
    pub ledger: u32,
}

/// Shared bundle executed event topics tuple.
pub fn bundle_executed_topics(buyer: &Address) -> (Symbol, Address) {
    (BUNDLE_EXECUTED_EVENT_NAME, buyer.clone())
}

/// Event name for batch sell completion.
pub const BATCH_SELL_COMPLETED_EVENT_NAME: Symbol = symbol_short!("bat_sell");

/// Stable batch sell completed event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(BATCH_SELL_COMPLETED_EVENT_NAME, seller)`
/// - data: `BatchSellCompletedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BatchSellCompletedEvent {
    pub seller: Address,
    pub orders: Vec<(Address, u32, i128)>,
    pub total_proceeds: i128,
    pub ledger: u32,
}

/// Shared batch sell completed event topics tuple.
pub fn batch_sell_completed_topics(seller: &Address) -> (Symbol, Address) {
    (BATCH_SELL_COMPLETED_EVENT_NAME, seller.clone())
}

/// Event name for bonding curve migration.
pub const CURVE_MIGRATED_EVENT_NAME: Symbol = symbol_short!("curve_mig");

/// Stable curve migrated event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(CURVE_MIGRATED_EVENT_NAME, admin)`
/// - data: `CurveMigratedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CurveMigratedEvent {
    pub admin: Address,
    pub new_exponent: u32,
    pub key_count: u32,
    pub ledger: u32,
}

/// Shared curve migrated event topics tuple.
pub fn curve_migrated_topics(admin: &Address) -> (Symbol, Address) {
    (CURVE_MIGRATED_EVENT_NAME, admin.clone())
}

/// Event name for royalty configuration update.
pub const ROYALTY_UPDATED_EVENT_NAME: Symbol = symbol_short!("roy_upd");

/// Stable royalty updated event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(ROYALTY_UPDATED_EVENT_NAME, creator)`
/// - data: `RoyaltyUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RoyaltyUpdatedEvent {
    pub creator: Address,
    pub buy_fee_bps: u32,
    pub sell_fee_bps: u32,
    pub ledger: u32,
}

/// Shared royalty updated event topics tuple.
pub fn royalty_updated_topics(creator: &Address) -> (Symbol, Address) {
    (ROYALTY_UPDATED_EVENT_NAME, creator.clone())
}

// --- Auction events ---

/// Event name for auction purchase.
pub const AUCTION_PURCHASE_EVENT_NAME: Symbol = symbol_short!("auc_buy");

/// Stable auction purchase event payload.
///
/// Event shape:
/// - topics: `(AUCTION_PURCHASE_EVENT_NAME, creator, buyer)`
/// - data: `AuctionPurchaseEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AuctionPurchaseEvent {
    pub buyer: Address,
    pub creator_id: Address,
    pub quantity: u32,
    pub price_paid: i128,
    pub new_supply: u32,
    pub auction_sold: u32,
    pub ledger: u32,
}

/// Shared auction purchase event topics tuple.
pub fn auction_purchase_topics(creator: &Address, buyer: &Address) -> (Symbol, Address, Address) {
    (AUCTION_PURCHASE_EVENT_NAME, creator.clone(), buyer.clone())
}

/// Event name for auction configured.
pub const AUCTION_CONFIGURED_EVENT_NAME: Symbol = symbol_short!("auc_cfg");

/// Stable auction configured event payload.
///
/// Event shape:
/// - topics: `(AUCTION_CONFIGURED_EVENT_NAME, creator)`
/// - data: `AuctionConfiguredEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AuctionConfiguredEvent {
    pub creator_id: Address,
    pub auction_supply: u32,
    pub auction_price: i128,
    pub ledger: u32,
}

/// Shared auction configured event topics tuple.
pub fn auction_configured_topics(creator: &Address) -> (Symbol, Address) {
    (AUCTION_CONFIGURED_EVENT_NAME, creator.clone())
}

/// Event name for a new staking position created via `stake_keys_locked`.
pub const STAKE_EVENT_NAME: Symbol = symbol_short!("stake");

/// Event name for a lock period extension via `stake_extend`.
pub const STAKE_EXTENDED_EVENT_NAME: Symbol = symbol_short!("stk_ext");

/// Event name for an early (pre-maturity) unstake via `early_unstake`.
pub const EARLY_UNSTAKE_EVENT_NAME: Symbol = symbol_short!("stk_chl");

/// Event name for a reward claim at/after maturity via `claim_stake_reward`.
pub const STAKE_REWARD_CLAIMED_EVENT_NAME: Symbol = symbol_short!("stk_clm");

/// Stable stake event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(STAKE_EVENT_NAME, creator_id, holder, stake_id)`
/// - data: `StakeEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakeEvent {
    /// Creator whose keys are staked.
    pub creator_id: Address,
    /// Staker that locked the keys.
    pub holder: Address,
    /// Sequential position id for the `(creator, holder)` pair.
    pub stake_id: u32,
    /// Number of keys locked.
    pub amount: u32,
    /// Ledger sequence at which the position matures.
    pub unlock_ledger: u32,
}

/// Shared stake event topics tuple.
pub fn stake_topics(
    creator: &Address,
    holder: &Address,
    stake_id: u32,
) -> (Symbol, Address, Address, u32) {
    (STAKE_EVENT_NAME, creator.clone(), holder.clone(), stake_id)
}

/// Stable stake-extend event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(STAKE_EXTENDED_EVENT_NAME, creator_id, holder, stake_id)`
/// - data: `StakeExtendedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakeExtendedEvent {
    /// Creator whose keys are staked.
    pub creator_id: Address,
    /// Staker that locked the keys.
    pub holder: Address,
    /// Extended position id.
    pub stake_id: u32,
    /// New maturity ledger sequence after the extension.
    pub unlock_ledger: u32,
    /// Additional ledgers appended to the lock period.
    pub additional_ledgers: u32,
}

/// Shared stake-extend event topics tuple.
pub fn stake_extended_topics(
    creator: &Address,
    holder: &Address,
    stake_id: u32,
) -> (Symbol, Address, Address, u32) {
    (
        STAKE_EXTENDED_EVENT_NAME,
        creator.clone(),
        holder.clone(),
        stake_id,
    )
}

/// Stable early-unstake event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(EARLY_UNSTAKE_EVENT_NAME, creator_id, holder, stake_id)`
/// - data: `EarlyUnstakeEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EarlyUnstakeEvent {
    /// Creator whose keys were staked.
    pub creator_id: Address,
    /// Staker that closed the position.
    pub holder: Address,
    /// Closed position id.
    pub stake_id: u32,
    /// Keys released back to the holder's liquid balance.
    pub amount: u32,
    /// Pro-rata reward entitlement removed from the pool.
    pub forgone_reward: i128,
    /// Penalty retained in the pool.
    pub penalty: i128,
    /// Ledger sequence at which the position was closed.
    pub ledger: u32,
}

/// Shared early-unstake event topics tuple.
pub fn early_unstake_topics(
    creator: &Address,
    holder: &Address,
    stake_id: u32,
) -> (Symbol, Address, Address, u32) {
    (
        EARLY_UNSTAKE_EVENT_NAME,
        creator.clone(),
        holder.clone(),
        stake_id,
    )
}

/// Stable stake-reward-claim event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(STAKE_REWARD_CLAIMED_EVENT_NAME, creator_id, holder, stake_id)`
/// - data: `StakeRewardClaimedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakeRewardClaimedEvent {
    /// Creator whose keys were staked.
    pub creator_id: Address,
    /// Staker that closed the position.
    pub holder: Address,
    /// Closed position id.
    pub stake_id: u32,
    /// Keys released back to the holder's liquid balance.
    pub amount: u32,
    /// Reward paid out from the pool.
    pub reward: i128,
    /// Ledger sequence at which the position matured.
    pub unlock_ledger: u32,
    /// Ledger sequence at which the reward was claimed.
    pub ledger: u32,
}

/// Shared stake-reward-claim event topics tuple.
pub fn stake_reward_claimed_topics(
    creator: &Address,
    holder: &Address,
    stake_id: u32,
) -> (Symbol, Address, Address, u32) {
    (
        STAKE_REWARD_CLAIMED_EVENT_NAME,
        creator.clone(),
        holder.clone(),
        stake_id,
    )
}

// ============================================================================
// Launch Penalty (#798)
// ============================================================================

/// Event name for launch penalty applied on sell.
pub const LAUNCH_PENALTY_APPLIED_EVENT_NAME: Symbol = symbol_short!("lnch_pnl");

/// Stable launch penalty applied event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(LAUNCH_PENALTY_APPLIED_EVENT_NAME, creator, seller)`
/// - data: `LaunchPenaltyAppliedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LaunchPenaltyAppliedEvent {
    /// Address of the creator whose key was sold.
    pub creator_id: Address,
    /// Address of the seller.
    pub seller: Address,
    /// Launch penalty basis points applied.
    pub penalty_bps: u32,
    /// Penalty amount deducted from proceeds.
    pub penalty_amount: i128,
    /// Ledger sequence at the time of the sale.
    pub ledger: u32,
}

/// Shared launch penalty applied event topics tuple.
pub fn launch_penalty_applied_topics(
    creator: &Address,
    seller: &Address,
) -> (Symbol, Address, Address) {
    (
        LAUNCH_PENALTY_APPLIED_EVENT_NAME,
        creator.clone(),
        seller.clone(),
    )
}

/// Event name for set_launch_penalty.
pub const LAUNCH_PENALTY_SET_EVENT_NAME: Symbol = symbol_short!("lnch_set");

/// Stable set launch penalty event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(LAUNCH_PENALTY_SET_EVENT_NAME, creator)`
/// - data: `LaunchPenaltySetEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LaunchPenaltySetEvent {
    /// Address of the creator.
    pub creator_id: Address,
    /// New penalty basis points.
    pub penalty_bps: u32,
    /// Ledger sequence at the time of the update.
    pub ledger: u32,
}

/// Shared set launch penalty event topics tuple.
pub fn launch_penalty_set_topics(creator: &Address) -> (Symbol, Address) {
    (LAUNCH_PENALTY_SET_EVENT_NAME, creator.clone())
}

// ============================================================================
// Per-wallet buy cooldown
// ============================================================================

/// Event name for a buy rejected by the per-wallet cooldown guard.
pub const COOLDOWN_BLOCKED_EVENT_NAME: Symbol = symbol_short!("cd_blk");

/// Stable cooldown-blocked event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(COOLDOWN_BLOCKED_EVENT_NAME, creator_id, wallet)`
/// - data: `CooldownBlockedEvent`
///
/// Emitted inside [`CreatorKeysContract::buy_key`] when the per-wallet
/// cooldown period has not elapsed since the buyer's last purchase.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CooldownBlockedEvent {
    /// Wallet whose buy was rejected.
    pub wallet: Address,
    /// Creator whose keys the buyer attempted to purchase.
    pub creator_id: Address,
    /// Number of ledgers remaining before the cooldown expires.
    pub ledgers_remaining: u32,
}

/// Shared cooldown blocked event topics tuple.
pub fn cooldown_blocked_topics(creator: &Address, wallet: &Address) -> (Symbol, Address, Address) {
    (COOLDOWN_BLOCKED_EVENT_NAME, creator.clone(), wallet.clone())
}

// ============================================================================
// Co-creator removal (#791)
// ============================================================================

/// Event name for a co-creator being removed.
pub const CO_CREATOR_REMOVED_EVENT_NAME: Symbol = symbol_short!("cc_rm");

/// Stable co-creator removed event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(CO_CREATOR_REMOVED_EVENT_NAME, creator_id, co_creator)`
/// - data: `CoCreatorRemovedEvent`
///
/// Emitted inside `remove_co_creator` when a creator removes their co-creator split.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CoCreatorRemovedEvent {
    /// Address of the creator whose co-creator was removed.
    pub creator_id: Address,
    /// Address of the co-creator that was removed.
    pub co_creator: Address,
}

/// Shared co-creator removed event topics tuple.
pub fn co_creator_removed_topics(
    creator_id: &Address,
    co_creator: &Address,
) -> (Symbol, Address, Address) {
    (
        CO_CREATOR_REMOVED_EVENT_NAME,
        creator_id.clone(),
        co_creator.clone(),
    )
}

/// Event name for dividend reinvestment.
pub const DIVIDEND_REINVESTED_EVENT_NAME: Symbol = symbol_short!("div_reinv");

/// Stable dividend reinvested event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(DIVIDEND_REINVESTED_EVENT_NAME, key_id, wallet)`
/// - data: `DividendReinvestedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendReinvestedEvent {
    pub wallet: Address,
    pub key_id: Address,
    pub keys_bought: u32,
    pub remainder_returned: i128,
}

/// Shared dividend reinvested event topics tuple.
pub fn dividend_reinvested_topics(
    key_id: &Address,
    wallet: &Address,
) -> (Symbol, Address, Address) {
    (
        DIVIDEND_REINVESTED_EVENT_NAME,
        key_id.clone(),
        wallet.clone(),
    )
}

// ============================================================================
// Pre-launch auction configuration (#790)
// ============================================================================

/// Event name for an auction being cancelled.
pub const AUCTION_CANCELLED_EVENT_NAME: Symbol = symbol_short!("auc_cxl");

/// Stable auction cancelled event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(AUCTION_CANCELLED_EVENT_NAME, creator_id)`
/// - data: `AuctionCancelledEvent`
///
/// Emitted inside `cancel_auction` when a creator cancels their pre-launch auction.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AuctionCancelledEvent {
    /// Address of the creator whose auction was cancelled.
    pub creator_id: Address,
}

/// Shared auction cancelled event topics tuple.
pub fn auction_cancelled_topics(creator: &Address) -> (Symbol, Address) {
    (AUCTION_CANCELLED_EVENT_NAME, creator.clone())
}

// --- Key deprecation and holder buyback events (#834) ---

/// Event name emitted when a creator deprecates their key.
pub const KEY_DEPRECATED_EVENT_NAME: Symbol = symbol_short!("key_dep");

/// Event name emitted when a holder redeems keys on a deprecated key.
pub const KEYS_REDEEMED_EVENT_NAME: Symbol = symbol_short!("key_rdm");

/// Stable field order for the key_deprecated event payload.
pub const KEY_DEPRECATED_DATA_FIELDS: [&str; 5] = [
    "creator",
    "buyback_price_per_key",
    "circulating_supply",
    "total_escrow",
    "ledger",
];

/// Stable field order for the keys_redeemed event payload.
pub const KEYS_REDEEMED_DATA_FIELDS: [&str; 6] = [
    "creator",
    "holder",
    "quantity",
    "payout",
    "new_supply",
    "ledger",
];

/// Stable key-deprecated event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(KEY_DEPRECATED_EVENT_NAME, creator)`
/// - data: `KeyDeprecatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyDeprecatedEvent {
    /// Creator who deprecated their key.
    pub creator: Address,
    /// Fixed XLM payout per key for all redemptions.
    pub buyback_price_per_key: i128,
    /// Circulating supply at the time of deprecation.
    pub circulating_supply: u32,
    /// Total XLM escrowed (`circulating_supply * buyback_price_per_key`).
    pub total_escrow: i128,
    /// Human-readable reason supplied by the creator (#973).
    pub reason: String,
    /// Optional successor key address (#973). `None` when not designated.
    pub successor_key_id: Option<Address>,
    /// Ledger sequence number at the time of deprecation.
    pub ledger: u32,
}

/// Shared key-deprecated event topics tuple.
pub fn key_deprecated_topics(creator: &Address) -> (Symbol, Address) {
    (KEY_DEPRECATED_EVENT_NAME, creator.clone())
}

/// Stable keys-redeemed event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(KEYS_REDEEMED_EVENT_NAME, creator, holder)`
/// - data: `KeysRedeemedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeysRedeemedEvent {
    /// Creator whose deprecated key is being redeemed.
    pub creator: Address,
    /// Holder who redeemed their keys.
    pub holder: Address,
    /// Number of keys redeemed by the holder.
    pub quantity: u32,
    /// Total XLM payout transferred to the holder.
    pub payout: i128,
    /// Total key supply for the creator after this redemption.
    pub new_supply: u32,
    /// Ledger sequence number at the time of redemption.
    pub ledger: u32,
}

/// Shared keys-redeemed event topics tuple.
pub fn keys_redeemed_topics(creator: &Address, holder: &Address) -> (Symbol, Address, Address) {
    (KEYS_REDEEMED_EVENT_NAME, creator.clone(), holder.clone())
}

// --- Notice-period deprecation (retiring inactive creator keys) ---

/// Event name emitted when an admin proposes to retire a key after a notice period.
pub const DEPRECATION_PROPOSED_EVENT_NAME: Symbol = symbol_short!("dep_prop");

/// Event name emitted when a proposed deprecation is executed.
pub const DEPRECATION_EXECUTED_EVENT_NAME: Symbol = symbol_short!("dep_exec");

/// Stable field order for the `dep_prop` event payload.
pub const DEPRECATION_PROPOSED_DATA_FIELDS: [&str; 5] = [
    "key_id",
    "reason",
    "notice_period_days",
    "unwind_at_timestamp",
    "ledger",
];

/// Stable field order for the `dep_exec` event payload.
pub const DEPRECATION_EXECUTED_DATA_FIELDS: [&str; 5] = [
    "key_id",
    "total_positions_unwound",
    "amount_distributed",
    "unwind_price",
    "ledger",
];

/// Stable deprecation-proposed event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(DEPRECATION_PROPOSED_EVENT_NAME, key_id)`
/// - data: `DeprecationProposedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DeprecationProposedEvent {
    /// Key (creator address) proposed for retirement.
    pub key_id: Address,
    /// Human-readable reason supplied by the admin.
    pub reason: String,
    /// Notice period length in days.
    pub notice_period_days: u32,
    /// Unix timestamp at (or after) which the unwind may execute.
    pub unwind_at_timestamp: u64,
    /// Ledger sequence at which the proposal was recorded.
    pub ledger: u32,
}

/// Shared deprecation-proposed event topics tuple.
pub fn deprecation_proposed_topics(key_id: &Address) -> (Symbol, Address) {
    (DEPRECATION_PROPOSED_EVENT_NAME, key_id.clone())
}

/// Stable deprecation-executed event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(DEPRECATION_EXECUTED_EVENT_NAME, key_id)`
/// - data: `DeprecationExecutedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DeprecationExecutedEvent {
    /// Key that was retired.
    pub key_id: Address,
    /// Total number of key positions unwound across all holders.
    pub total_positions_unwound: u32,
    /// Total XLM distributed to holders during the unwind.
    pub amount_distributed: i128,
    /// Per-key price used for the unwind (last traded price).
    pub unwind_price: i128,
    /// Ledger sequence at which the unwind executed.
    pub ledger: u32,
}

/// Shared deprecation-executed event topics tuple.
pub fn deprecation_executed_topics(key_id: &Address) -> (Symbol, Address) {
    (DEPRECATION_EXECUTED_EVENT_NAME, key_id.clone())
}

// ============================================================================
// Early Unstake Penalty
// ============================================================================

/// Event name for early unstake with forfeited penalty.
pub const EARLY_UNSTAKE_PENALTY_EVENT_NAME: Symbol = symbol_short!("erl_unst");

/// Stable early unstake event payload.
///
/// Event shape:
/// - topics: `(EARLY_UNSTAKE_PENALTY_EVENT_NAME, key_id, wallet)`
/// - data: `EarlyUnstakePenaltyEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EarlyUnstakePenaltyEvent {
    pub wallet: Address,
    pub key_id: Address,
    pub returned_quantity: u32,
    pub penalty_quantity: u32,
}

/// Shared early unstake event topics tuple.
pub fn early_unstake_penalty_topics(
    key_id: &Address,
    wallet: &Address,
) -> (Symbol, Address, Address) {
    (
        EARLY_UNSTAKE_PENALTY_EVENT_NAME,
        key_id.clone(),
        wallet.clone(),
    )
}

/// Event name for a satisfied slippage check on buy or sell.
pub const SLIPPAGE_CHECK_PASSED_EVENT_NAME: Symbol = symbol_short!("slp_ok");

/// Emitted when a buy or sell with a non-None slippage bound passes the
/// price/proceeds check, giving downstream indexers visibility into slippage
/// guard behavior.
///
/// Event shape:
/// - topics: `(SLIPPAGE_CHECK_PASSED_EVENT_NAME, creator)`
/// - data: `SlippageCheckPassedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SlippageCheckPassedEvent {
    pub creator_id: Address,
    pub actual_amount: i128,
    pub bound: i128,
    pub ledger: u32,
}

/// Shared slippage-check-passed event topics tuple.
pub fn slippage_check_passed_topics(creator: &Address) -> (Symbol, Address) {
    (SLIPPAGE_CHECK_PASSED_EVENT_NAME, creator.clone())
}

/// Event name for a bonding-curve swap rejected by its price-impact guard.
pub const SWAP_SLIPPAGE_EXCEEDED_EVENT_NAME: Symbol = symbol_short!("swap_slp");

/// Emitted when a bonding-curve `swap` is rejected because the computed
/// price impact exceeds the caller's `max_slippage_bps` tolerance. The
/// payload carries both the actual impact and the requested ceiling so
/// clients and indexers can surface the exact breach magnitude.
///
/// Event shape:
/// - topics: `(SWAP_SLIPPAGE_EXCEEDED_EVENT_NAME, creator)`
/// - data: `SwapSlippageExceededEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SwapSlippageExceededEvent {
    /// Creator whose bonding curve was being swapped against.
    pub creator_id: Address,
    /// Computed price impact of the rejected swap, in basis points.
    pub actual_bps: u32,
    /// Caller-supplied slippage ceiling that the impact breached.
    pub max_bps: u32,
    /// Ledger sequence at which the breach was detected.
    pub ledger: u32,
}

/// Shared swap-slippage-exceeded event topics tuple.
pub fn swap_slippage_exceeded_topics(creator: &Address) -> (Symbol, Address) {
    (SWAP_SLIPPAGE_EXCEEDED_EVENT_NAME, creator.clone())
}

/// Event name for a completed cross-key atomic swap.
pub const ATOMIC_SWAP_EVENT_NAME: Symbol = symbol_short!("atom_swap");

/// Emitted when two wallets exchange keys of two different creators in one
/// atomic transaction. The prices are the bonding-curve spot prices used to
/// value each side, so `qty_a * price_a` and `qty_b * price_b` reconstruct the
/// values the parity check compared.
///
/// Event shape:
/// - topics: `(ATOMIC_SWAP_EVENT_NAME, key_a, key_b)`
/// - data: `AtomicSwapEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AtomicSwapEvent {
    /// First creator key, given by the sender and received by the second wallet.
    pub key_a: Address,
    /// Quantity of `key_a` swapped.
    pub qty_a: u32,
    /// Bonding-curve spot price of `key_a` used to value the first leg.
    pub price_a: i128,
    /// Second creator key, given by the second wallet and received by the sender.
    pub key_b: Address,
    /// Quantity of `key_b` swapped.
    pub qty_b: u32,
    /// Bonding-curve spot price of `key_b` used to value the second leg.
    pub price_b: i128,
    /// Ledger sequence at which the swap settled.
    pub ledger: u32,
}

/// Shared atomic-swap event topics tuple.
pub fn atomic_swap_topics(key_a: &Address, key_b: &Address) -> (Symbol, Address, Address) {
    (ATOMIC_SWAP_EVENT_NAME, key_a.clone(), key_b.clone())
}

// --- Max buy quantity per transaction (#828) ---

/// Event name emitted when the creator updates the max buy quantity per transaction.
pub const MAX_BUY_QUANTITY_UPDATED_EVENT_NAME: Symbol = symbol_short!("mbq_upd");

/// Stable max-buy-quantity-updated event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(MAX_BUY_QUANTITY_UPDATED_EVENT_NAME, creator_id)`
/// - data: `MaxBuyQuantityUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MaxBuyQuantityUpdatedEvent {
    /// Address of the creator whose limit was changed.
    pub creator_id: Address,
    /// New per-transaction buy quantity limit.
    pub max_qty: u32,
    /// Ledger sequence at the time of the update.
    pub ledger: u32,
}

/// Shared max-buy-quantity-updated event topics tuple.
pub fn max_buy_quantity_updated_topics(creator: &Address) -> (Symbol, Address) {
    (MAX_BUY_QUANTITY_UPDATED_EVENT_NAME, creator.clone())
}

// --- Per-key trade size limits (Issue #1100) ---

/// Event name emitted when the admin updates a key's trade size limits.
pub const TRADE_LIMIT_UPDATED_EVENT_NAME: Symbol = symbol_short!("tl_upd");

/// Stable trade-limit-updated event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(TRADE_LIMIT_UPDATED_EVENT_NAME, key_id)`
/// - data: `TradeLimitUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TradeLimitUpdatedEvent {
    /// Address of the key whose trade limits were changed.
    pub key_id: Address,
    /// New per-swap buy quantity limit (`0` = no limit).
    pub max_buy: u32,
    /// New per-swap sell quantity limit (`0` = no limit).
    pub max_sell: u32,
    /// Ledger sequence at the time of the update.
    pub ledger: u32,
}

/// Shared trade-limit-updated event topics tuple.
pub fn trade_limit_updated_topics(key_id: &Address) -> (Symbol, Address) {
    (TRADE_LIMIT_UPDATED_EVENT_NAME, key_id.clone())
}

/// Event name emitted when a swap is rejected for exceeding a configured
/// per-key trade size limit.
pub const TRADE_LIMIT_EXCEEDED_EVENT_NAME: Symbol = symbol_short!("tl_exc");

/// Stable trade-limit-exceeded event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(TRADE_LIMIT_EXCEEDED_EVENT_NAME, key_id)`
/// - data: `TradeLimitExceededEvent`
///
/// Carries the attempted and permitted amounts plus the direction so indexers
/// can distinguish a buy rejection from a sell rejection without consulting
/// the (dataless) [`crate::TradeLimitError::TradeLimitExceeded`] error code.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TradeLimitExceededEvent {
    /// Address of the key whose limit was breached.
    pub key_id: Address,
    /// Direction of the rejected swap.
    pub direction: SwapDirection,
    /// Quantity the caller attempted to swap.
    pub attempted: u32,
    /// Configured limit for that direction (`0` would mean no limit, but a
    /// `0` limit never triggers this event).
    pub max: u32,
    /// Ledger sequence at the time of the rejection.
    pub ledger: u32,
}

/// Shared trade-limit-exceeded event topics tuple.
pub fn trade_limit_exceeded_topics(key_id: &Address) -> (Symbol, Address) {
    (TRADE_LIMIT_EXCEEDED_EVENT_NAME, key_id.clone())
}

// --- Batch transfer keys (#799) ---

/// Event name emitted when a holder transfers keys to multiple recipients in a
/// single `batch_transfer_keys` call.
pub const BATCH_TRANSFER_COMPLETED_EVENT_NAME: Symbol = symbol_short!("bat_xfer");

/// Stable field order for batch transfer completed event payloads.
pub const BATCH_TRANSFER_COMPLETED_DATA_FIELDS: [&str; 5] = [
    "creator_id",
    "from",
    "transfers",
    "total_transferred",
    "ledger",
];

/// Stable batch transfer completed event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(BATCH_TRANSFER_COMPLETED_EVENT_NAME, creator_id, from)`
/// - data: `BatchTransferCompletedEvent`
///
/// `transfers` is the ordered list of `(recipient, quantity)` pairs processed
/// in the batch. `total_transferred` is the sum of all quantities.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BatchTransferCompletedEvent {
    /// Address of the creator whose key is being transferred.
    pub creator_id: Address,
    /// Holder sending the keys.
    pub from: Address,
    /// Ordered `(recipient, quantity)` pairs processed in the batch.
    pub transfers: Vec<(Address, u32)>,
    /// Sum of all quantities transferred in the batch.
    pub total_transferred: u32,
    /// Ledger sequence at the time of the transfer.
    pub ledger: u32,
}

/// Shared batch transfer completed event topics tuple.
pub fn batch_transfer_completed_topics(
    creator: &Address,
    from: &Address,
) -> (Symbol, Address, Address) {
    (
        BATCH_TRANSFER_COMPLETED_EVENT_NAME,
        creator.clone(),
        from.clone(),
    )
}

/// Event name for a price-oracle read.
pub const PRICE_QUERIED_EVENT_NAME: Symbol = symbol_short!("pri_qry");

/// Stable price-queried event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(PRICE_QUERIED_EVENT_NAME, caller)`
/// - data: `PriceQueriedEvent`
///
/// Emitted on every successful price-oracle read (`get_price` /
/// `get_twap_price`), carrying the calling contract's address and the returned
/// price.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceQueriedEvent {
    /// Contract that invoked the oracle entrypoint.
    pub caller: Address,
    /// Creator whose key's price was read.
    pub creator: Address,
    /// Price returned to the caller.
    pub price: i128,
}

/// Shared price-queried event topics tuple.
pub fn price_queried_topics(caller: &Address) -> (Symbol, Address) {
    (PRICE_QUERIED_EVENT_NAME, caller.clone())
}

// ============================================================================
// Claimable dividends (issue #857)
// ============================================================================

/// Event name for a per-holder dividend credit written at distribution time.
pub const DIVIDEND_CREDITED_EVENT_NAME: Symbol = symbol_short!("div_crd");

/// Stable field order for `dividend_credited` payloads.
pub const DIVIDEND_CREDITED_DATA_FIELDS: [&str; 3] = ["creator", "holder", "amount"];

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendCreditedEvent {
    pub creator: Address,
    pub holder: Address,
    pub amount: i128,
}

pub fn dividend_credited_topics(creator: &Address, holder: &Address) -> (Symbol, Address, Address) {
    (
        DIVIDEND_CREDITED_EVENT_NAME,
        creator.clone(),
        holder.clone(),
    )
}

// ============================================================================

// --- Pause state change (#889) ---

pub const PAUSE_STATE_CHANGED_EVENT_NAME: Symbol = symbol_short!("pause_chg");

/// Emitted by `pause` and `unpause` with the new state and the calling admin.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PauseStateChangedEvent {
    pub paused: bool,
    pub caller: Address,
}

pub fn pause_state_changed_topics() -> (Symbol,) {
    (PAUSE_STATE_CHANGED_EVENT_NAME,)
}

// --- Supply milestone crossings (#887) ---

pub const MILESTONE_CROSSED_EVENT_NAME: Symbol = symbol_short!("mile_x");
pub const MILESTONE_DIRECTION_UP: Symbol = symbol_short!("up");
pub const MILESTONE_DIRECTION_DOWN: Symbol = symbol_short!("down");

/// Emitted once per configured supply milestone crossed by a trade.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MilestoneCrossedEvent {
    pub key_id: Address,
    /// 1-based position of the crossed milestone in the configured list.
    pub tier: u32,
    pub direction: Symbol,
    /// Supply after the trade.
    pub supply: u32,
}

pub fn milestone_crossed_topics(key_id: &Address) -> (Symbol, Address) {
    (MILESTONE_CROSSED_EVENT_NAME, key_id.clone())
}

// --- Supply milestone fee reduction (#1076) ---

/// Emitted when a key's circulating supply crosses a milestone threshold and
/// the fee in effect is reduced.
pub const MILESTONE_REACHED_EVENT_NAME: Symbol = symbol_short!("ms_reach");

/// Event shape:
/// - topics: `(MILESTONE_REACHED_EVENT_NAME, key_id)`
/// - data: `MilestoneReachedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MilestoneReachedEvent {
    /// The key whose supply crossed the milestone threshold.
    pub key_id: Address,
    /// 1-based index of the milestone tier that was reached.
    pub tier: u32,
    /// Circulating supply after the trade that crossed the threshold.
    pub supply: u32,
    /// New fee in basis points after the reduction.
    pub new_fee_bps: u32,
    /// Ledger sequence number at the time of the reduction.
    pub ledger: u32,
}

pub fn milestone_reached_topics(key_id: &Address) -> (Symbol, Address) {
    (MILESTONE_REACHED_EVENT_NAME, key_id.clone())
}

/// Publish a [`MilestoneReachedEvent`] for `key_id`.
pub fn publish_milestone_reached(
    env: &Env,
    key_id: &Address,
    tier: u32,
    supply: u32,
    new_fee_bps: u32,
    ledger: u32,
) {
    env.events().publish(
        milestone_reached_topics(key_id),
        MilestoneReachedEvent {
            key_id: key_id.clone(),
            tier,
            supply,
            new_fee_bps,
            ledger,
        },
    );
}

// --- Contract upgrade (#884) ---

pub const UPGRADE_EXECUTED_EVENT_NAME: Symbol = symbol_short!("upgraded");

/// Emitted by `upgrade` with the version before and after the upgrade.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct UpgradeExecutedEvent {
    pub old_version: u32,
    pub new_version: u32,
}

pub fn upgrade_executed_topics(admin: &Address) -> (Symbol, Address) {
    (UPGRADE_EXECUTED_EVENT_NAME, admin.clone())
}

/// Event name for admin-authorised key registration.
pub const KEY_REGISTERED_EVENT_NAME: Symbol = symbol_short!("key_reg");

/// Emitted by `register_key`. Keys are identified by their creator address, so
/// `key_id` and `creator` carry the same address.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyRegisteredEvent {
    pub key_id: Address,
    pub creator: Address,
    pub auction_pending: bool,
    pub registered_at_ledger: u32,
}

pub fn key_registered_topics(key_id: &Address) -> (Symbol, Address) {
    (KEY_REGISTERED_EVENT_NAME, key_id.clone())
}

/// Event name for a staking vault deposit.
pub const VAULT_DEPOSIT_EVENT_NAME: Symbol = symbol_short!("vlt_dep");

/// Event name for a staking vault withdrawal.
pub const VAULT_WITHDRAW_EVENT_NAME: Symbol = symbol_short!("vlt_wdr");

/// Stable vault deposit event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(VAULT_DEPOSIT_EVENT_NAME, creator_id, holder)`
/// - data: `VaultDepositEvent`
///
/// Emitted once per creator key included in a `vault_deposit` call.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VaultDepositEvent {
    /// Creator whose keys were deposited.
    pub creator_id: Address,
    /// Holder that deposited the keys.
    pub holder: Address,
    /// Number of keys deposited.
    pub amount: u32,
    /// Holder's vault shares for this creator after the deposit.
    pub holder_shares: u32,
    /// Total vault shares for this creator after the deposit.
    pub total_shares: u32,
    /// Ledger sequence number at the time of the deposit.
    pub ledger: u32,
}

/// Stable vault withdraw event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(VAULT_WITHDRAW_EVENT_NAME, creator_id, holder)`
/// - data: `VaultWithdrawEvent`
///
/// Emitted once per creator key included in a `vault_withdraw` call.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VaultWithdrawEvent {
    /// Creator whose keys were withdrawn.
    pub creator_id: Address,
    /// Holder that withdrew the keys.
    pub holder: Address,
    /// Number of keys returned to the holder.
    pub amount: u32,
    /// Holder's vault shares for this creator after the withdrawal.
    pub holder_shares: u32,
    /// Total vault shares for this creator after the withdrawal.
    pub total_shares: u32,
    /// Ledger sequence number at the time of the withdrawal.
    pub ledger: u32,
}

/// Shared vault deposit event topics tuple.
pub fn vault_deposit_topics(creator: &Address, holder: &Address) -> (Symbol, Address, Address) {
    (VAULT_DEPOSIT_EVENT_NAME, creator.clone(), holder.clone())
}

/// Shared vault withdraw event topics tuple.
pub fn vault_withdraw_topics(creator: &Address, holder: &Address) -> (Symbol, Address, Address) {
    (VAULT_WITHDRAW_EVENT_NAME, creator.clone(), holder.clone())
}

/// Event name for an oracle price update.
pub const ORACLE_PRICE_UPDATED_EVENT_NAME: Symbol = symbol_short!("orc_upd");

/// Stable oracle price update event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(ORACLE_PRICE_UPDATED_EVENT_NAME, oracle)`
/// - data: `OraclePriceUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct OraclePriceUpdatedEvent {
    /// Authorised oracle address that published the price.
    pub oracle: Address,
    /// Published price.
    pub price: i128,
    /// Ledger timestamp (seconds) at which the price was published.
    pub timestamp: u64,
}

/// Shared oracle price update event topics tuple.
pub fn oracle_price_updated_topics(oracle: &Address) -> (Symbol, Address) {
    (ORACLE_PRICE_UPDATED_EVENT_NAME, oracle.clone())
}

/// Event name for a timelocked action proposal.
pub const ACTION_PROPOSED_EVENT_NAME: Symbol = symbol_short!("act_prop");

/// Event name for a timelocked action execution.
pub const ACTION_EXECUTED_EVENT_NAME: Symbol = symbol_short!("act_exec");

/// Event name for a timelocked action cancellation.
pub const ACTION_CANCELLED_EVENT_NAME: Symbol = symbol_short!("act_canc");

/// Stable action proposed event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(ACTION_PROPOSED_EVENT_NAME, action_id)`
/// - data: `ActionProposedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ActionProposedEvent {
    pub action_id: u32,
    pub proposer: Address,
    pub change_type: u32,
    /// Ledger timestamp (seconds) at which the action was proposed.
    pub proposed_at: u64,
    /// Earliest ledger timestamp (seconds) at which the action may execute.
    pub execution_not_before: u64,
}

/// Stable action executed event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(ACTION_EXECUTED_EVENT_NAME, action_id)`
/// - data: `ActionExecutedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ActionExecutedEvent {
    pub action_id: u32,
    /// Ledger timestamp (seconds) at which the action was executed.
    pub executed_at: u64,
}

/// Stable action cancelled event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(ACTION_CANCELLED_EVENT_NAME, action_id)`
/// - data: `ActionCancelledEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ActionCancelledEvent {
    pub action_id: u32,
    /// Ledger timestamp (seconds) at which the action was cancelled.
    pub cancelled_at: u64,
}

/// Shared action proposed event topics tuple.
pub fn action_proposed_topics(action_id: u32) -> (Symbol, u32) {
    (ACTION_PROPOSED_EVENT_NAME, action_id)
}

/// Shared action executed event topics tuple.
pub fn action_executed_topics(action_id: u32) -> (Symbol, u32) {
    (ACTION_EXECUTED_EVENT_NAME, action_id)
}

/// Shared action cancelled event topics tuple.
pub fn action_cancelled_topics(action_id: u32) -> (Symbol, u32) {
    (ACTION_CANCELLED_EVENT_NAME, action_id)
}

// ============================================================================
// Feature: timelocked contract upgrade — LogicUpgraded / UpgradeApproved
// ============================================================================

/// Event name emitted when a timelocked upgrade swaps the contract's logic build.
///
/// Emitted alongside (never instead of) `UpgradeExecutedEvent`, so indexers
/// already tracking the legacy `upgraded` event keep working unchanged.
pub const LOGIC_UPGRADED_EVENT_NAME: Symbol = symbol_short!("logic_upg");

/// Stable payload describing a completed timelocked logic upgrade.
///
/// Carries both the outgoing and incoming logic identity, which is the whole
/// point of the event: an operator can diff the pair to confirm which build is
/// live and, if it misbehaved, which build to propose a rollback to.
///
/// Event shape:
/// - topics: `(LOGIC_UPGRADED_EVENT_NAME, action_id)`
/// - data: `LogicUpgradedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LogicUpgradedEvent {
    /// Timelocked action that carried out the swap.
    pub action_id: u32,
    /// Logic (WASM) hash in effect before the upgrade. `None` on the first
    /// recorded upgrade, when no prior hash has been retained yet.
    pub old_wasm_hash: Option<BytesN<32>>,
    /// Logic (WASM) hash now in effect.
    pub new_wasm_hash: BytesN<32>,
    pub old_version: u32,
    pub new_version: u32,
    /// Ledger timestamp (seconds) at which the swap was applied.
    pub executed_at: u64,
}

/// Shared logic-upgraded event topics tuple.
pub fn logic_upgraded_topics(action_id: u32) -> (Symbol, u32) {
    (LOGIC_UPGRADED_EVENT_NAME, action_id)
}

/// Event name emitted when one member of the multi-sig admin set approves a
/// pending timelocked upgrade.
pub const UPGRADE_APPROVED_EVENT_NAME: Symbol = symbol_short!("upg_appr");

/// Stable payload for a single multi-sig approval of a pending upgrade.
///
/// Event shape:
/// - topics: `(UPGRADE_APPROVED_EVENT_NAME, action_id)`
/// - data: `UpgradeApprovedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct UpgradeApprovedEvent {
    /// Timelocked action being approved.
    pub action_id: u32,
    /// Admin that cast this approval.
    pub admin: Address,
    /// Total distinct approvals recorded so far, including this one.
    pub approvals: u32,
    /// Distinct approvals required before the upgrade may execute.
    pub threshold: u32,
    /// Ledger timestamp (seconds) of the approval.
    pub approved_at: u64,
}

/// Shared upgrade-approved event topics tuple.
pub fn upgrade_approved_topics(action_id: u32) -> (Symbol, u32) {
    (UPGRADE_APPROVED_EVENT_NAME, action_id)
}

// ============================================================================
// Feature: holder_count tracking — HolderCountChanged event
// ============================================================================

/// Event name emitted when the holder count for a creator key changes.
pub const HOLDER_COUNT_CHANGED_EVENT_NAME: Symbol = symbol_short!("hc_chg");

/// Stable holder-count-changed event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(HOLDER_COUNT_CHANGED_EVENT_NAME, creator_id)`
/// - data: `HolderCountChangedEvent`
///
/// Emitted every time a wallet crosses the zero-balance boundary (first buy
/// increments, full exit decrements).
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct HolderCountChangedEvent {
    /// Creator whose holder count changed.
    pub creator_id: Address,
    /// Holder count before the change.
    pub old_count: u32,
    /// Holder count after the change.
    pub new_count: u32,
    /// Ledger sequence number at the time of the change.
    pub ledger: u32,
}

/// Shared holder-count-changed event topics tuple.
pub fn holder_count_changed_topics(creator_id: &Address) -> (Symbol, Address) {
    (HOLDER_COUNT_CHANGED_EVENT_NAME, creator_id.clone())
}

// ============================================================================
// Feature: update_metadata / update_config events
// ============================================================================

/// Event name emitted when a creator updates their key metadata.
pub const METADATA_UPDATED_EVENT_NAME: Symbol = symbol_short!("meta_upd");

/// Stable metadata-updated event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(METADATA_UPDATED_EVENT_NAME, creator_id)`
/// - data: `MetadataUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MetadataUpdatedEvent {
    /// Creator whose metadata was updated.
    pub creator_id: Address,
    /// Legacy field; the key name is immutable and this remains empty.
    pub name: String,
    /// Legacy mirror of `description`; empty when unchanged.
    pub bio: String,
    /// Legacy mirror of `image_cid`; empty when unchanged.
    pub avatar_uri: String,
    /// Ledger sequence number at the time of the update.
    pub ledger: u32,
    /// Updated description, or `None` if unchanged.
    pub description: Option<String>,
    /// Updated image CID, or `None` if unchanged.
    pub image_cid: Option<String>,
}

/// Shared metadata-updated event topics tuple.
pub fn metadata_updated_topics(creator_id: &Address) -> (Symbol, Address) {
    (METADATA_UPDATED_EVENT_NAME, creator_id.clone())
}

/// Event name emitted when the admin updates protocol config parameters.
pub const CONFIG_UPDATED_EVENT_NAME: Symbol = symbol_short!("cfg_upd");

/// Stable config-updated event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(CONFIG_UPDATED_EVENT_NAME, admin)`
/// - data: `ConfigUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ConfigUpdatedEvent {
    /// Admin who performed the update.
    pub admin: Address,
    /// New creator fee basis points.
    pub creator_bps: u32,
    /// New protocol fee basis points.
    pub protocol_bps: u32,
    /// New bonding curve slope.
    pub curve_slope: i128,
    /// Ledger sequence number at the time of the update.
    pub ledger: u32,
}

/// Shared config-updated event topics tuple.
pub fn config_updated_topics(admin: &Address) -> (Symbol, Address) {
    (CONFIG_UPDATED_EVENT_NAME, admin.clone())
}

// ============================================================================
// Feature: snapshot mechanism — governance address set/get events
// ============================================================================

/// Event name emitted when the governance contract address is configured.
pub const GOVERNANCE_ADDRESS_SET_EVENT_NAME: Symbol = symbol_short!("gov_set");

/// Shared governance-address-set event topics tuple.
pub fn governance_address_set_topics(admin: &Address) -> (Symbol, Address) {
    (GOVERNANCE_ADDRESS_SET_EVENT_NAME, admin.clone())
}

/// Event name emitted when an old snapshot is pruned.
pub const SNAPSHOT_PRUNED_EVENT_NAME: Symbol = symbol_short!("snap_prn");

/// Stable snapshot-pruned event payload.
///
/// Event shape:
/// - topics: `(SNAPSHOT_PRUNED_EVENT_NAME, creator_id)`
/// - data: `SnapshotPrunedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SnapshotPrunedEvent {
    pub creator_id: Address,
    pub snapshot_id: u32,
    pub ledger: u32,
}

/// Shared snapshot-pruned event topics tuple.
pub fn snapshot_pruned_topics(creator_id: &Address) -> (Symbol, Address) {
    (SNAPSHOT_PRUNED_EVENT_NAME, creator_id.clone())
}

// ============================================================================
// Feature: governance voting weight snapshot contract
// ============================================================================

/// Event name emitted when a governance voting weight snapshot is taken.
pub const GOVERNANCE_SNAPSHOT_TAKEN_EVENT_NAME: Symbol = symbol_short!("gov_snap");

/// Stable governance-snapshot-taken event payload.
///
/// Event shape:
/// - topics: `(GOVERNANCE_SNAPSHOT_TAKEN_EVENT_NAME, proposal_id)`
/// - data: `GovernanceSnapshotTakenEvent`
///
/// Emitted automatically when a governance proposal is created, recording the
/// block height at which all holder balances were captured. Downstream indexers
/// use `snapshot_ledger` to correlate voting weights with the proposal's
/// creation block.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct GovernanceSnapshotTakenEvent {
    /// Monotonic governance proposal identifier.
    pub proposal_id: u32,
    /// Ledger sequence at which the snapshot was taken.
    pub snapshot_ledger: u32,
    /// Number of holder balances recorded in the snapshot.
    pub total_holders: u32,
}

/// Shared governance-snapshot-taken event topics tuple.
pub fn governance_snapshot_taken_topics(proposal_id: u32) -> (Symbol, u32) {
    (GOVERNANCE_SNAPSHOT_TAKEN_EVENT_NAME, proposal_id)
}

/// Event name emitted when a governance snapshot is pruned after resolution.
pub const GOVERNANCE_SNAPSHOT_PRUNED_EVENT_NAME: Symbol = symbol_short!("gov_sprn");

/// Stable governance-snapshot-pruned event payload.
///
/// Event shape:
/// - topics: `(GOVERNANCE_SNAPSHOT_PRUNED_EVENT_NAME, proposal_id)`
/// - data: `GovernanceSnapshotPrunedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct GovernanceSnapshotPrunedEvent {
    /// Proposal whose snapshot was pruned.
    pub proposal_id: u32,
    /// Ledger in which the pruning happened.
    pub ledger: u32,
}

/// Shared governance-snapshot-pruned event topics tuple.
pub fn governance_snapshot_pruned_topics(proposal_id: u32) -> (Symbol, u32) {
    (GOVERNANCE_SNAPSHOT_PRUNED_EVENT_NAME, proposal_id)
}

// ============================================================================
// Feature: batch_buy with per-key slippage — BatchBuyOrderResult event
// ============================================================================

/// Event name emitted per key in a batch buy when a per-order fee is collected.
pub const BATCH_BUY_FEE_COLLECTED_EVENT_NAME: Symbol = symbol_short!("bb_fee");

/// Stable batch-buy fee-collected event payload.
///
/// Event shape:
/// - topics: `(BATCH_BUY_FEE_COLLECTED_EVENT_NAME, creator_id, buyer)`
/// - data: `BatchBuyFeeCollectedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BatchBuyFeeCollectedEvent {
    pub creator_id: Address,
    pub buyer: Address,
    pub quantity: u32,
    pub total_price: i128,
    pub fee_amount: i128,
    pub ledger: u32,
}

/// Shared batch-buy fee-collected event topics tuple.
pub fn batch_buy_fee_collected_topics(
    creator_id: &Address,
    buyer: &Address,
) -> (Symbol, Address, Address) {
    (
        BATCH_BUY_FEE_COLLECTED_EVENT_NAME,
        creator_id.clone(),
        buyer.clone(),
    )
}

// ============================================================================
// Feature: creator reputation scoring
// ============================================================================

/// Event name emitted on every reputation score change.
pub const REPUTATION_UPDATED_EVENT_NAME: Symbol = symbol_short!("rep_upd");

/// Stable reputation-updated event payload.
///
/// Event shape:
/// - topics: `(REPUTATION_UPDATED_EVENT_NAME, creator)`
/// - data: `ReputationUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReputationUpdatedEvent {
    /// Creator whose reputation changed.
    pub creator: Address,
    /// Reputation score before the change.
    pub old_score: i128,
    /// Reputation score after the change.
    pub new_score: i128,
    /// Signed points applied by this change.
    pub delta: i128,
    /// Which on-chain action produced the change.
    pub reason: crate::ReputationReason,
    /// Ledger in which the change was recorded.
    pub ledger: u32,
}

/// Event name for a creator announcement being posted.
pub const ANNOUNCEMENT_POSTED_EVENT_NAME: Symbol = symbol_short!("ann_post");

/// Data emitted when a creator posts an announcement.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AnnouncementPostedEvent {
    pub key_id: Address,
    pub announcement_id: u32,
    pub title: String,
    pub content_hash: String,
    pub category: crate::AnnouncementCategory,
    pub posted_at: u64,
}

/// Shared announcement event topics tuple.
pub fn announcement_posted_topics(
    key_id: &Address,
    announcement_id: u32,
) -> (Symbol, Address, u32) {
    (
        ANNOUNCEMENT_POSTED_EVENT_NAME.clone(),
        key_id.clone(),
        announcement_id,
    )
}

// --- Key ownership handover (#1098) ---

/// Event name for `TransferProposed`: the wallet managing a key proposed
/// handing the key's management rights to a new wallet. Emitted alongside the
/// 48h acceptance window, so it doubles as the holder notification.
pub const TRANSFER_PROPOSED_EVENT_NAME: Symbol = symbol_short!("own_prop");

/// Stable ownership-proposed event field order for downstream indexers.
pub const TRANSFER_PROPOSED_EVENT_DATA_FIELDS: [&str; 7] = [
    "proposal_id",
    "key_id",
    "current_owner",
    "new_owner",
    "proposed_at",
    "expires_at",
    "ledger",
];

/// Payload emitted when an ownership handover is proposed.
///
/// Event shape:
/// - topics: `(TRANSFER_PROPOSED_EVENT_NAME, key_id, new_owner)`
/// - data: `TransferProposedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TransferProposedEvent {
    /// Sequential id of the proposal; also its storage key.
    pub proposal_id: u32,
    /// Key whose management rights are being handed over.
    pub key_id: Address,
    /// Owner that raised the proposal.
    pub current_owner: Address,
    /// Wallet proposed to take over management of `key_id`.
    pub new_owner: Address,
    /// Ledger timestamp at which the proposal was raised.
    pub proposed_at: u64,
    /// `proposed_at + ACCEPTANCE_WINDOW_SECS`; acceptance is rejected from here.
    pub expires_at: u64,
    /// Ledger sequence number at the time of the proposal.
    pub ledger: u32,
}

/// Shared ownership-proposed event topics tuple.
pub fn transfer_proposed_topics(
    key_id: &Address,
    new_owner: &Address,
) -> (Symbol, Address, Address) {
    (
        TRANSFER_PROPOSED_EVENT_NAME,
        key_id.clone(),
        new_owner.clone(),
    )
}

/// Event name for `TransferAccepted`: the proposed new owner accepted the
/// ownership handover, which starts the 24h execution timelock.
pub const TRANSFER_ACCEPTED_EVENT_NAME: Symbol = symbol_short!("own_acpt");

/// Stable ownership-accepted event field order for downstream indexers.
pub const TRANSFER_ACCEPTED_EVENT_DATA_FIELDS: [&str; 7] = [
    "proposal_id",
    "key_id",
    "current_owner",
    "new_owner",
    "accepted_at",
    "executable_at",
    "ledger",
];

/// Payload emitted when the proposed new owner accepts a handover.
///
/// Event shape:
/// - topics: `(TRANSFER_ACCEPTED_EVENT_NAME, key_id, new_owner)`
/// - data: `TransferAcceptedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TransferAcceptedEvent {
    /// Sequential id of the accepted proposal.
    pub proposal_id: u32,
    /// Key whose management rights are being handed over.
    pub key_id: Address,
    /// Owner the key still has until the handover executes.
    pub current_owner: Address,
    /// Wallet that accepted the handover.
    pub new_owner: Address,
    /// Ledger timestamp at which the new owner accepted.
    pub accepted_at: u64,
    /// `accepted_at + EXECUTION_TIMELOCK_SECS`, the earliest execution time.
    pub executable_at: u64,
    /// Ledger sequence number at the time of acceptance.
    pub ledger: u32,
}

/// Shared ownership-accepted event topics tuple.
pub fn transfer_accepted_topics(
    key_id: &Address,
    new_owner: &Address,
) -> (Symbol, Address, Address) {
    (
        TRANSFER_ACCEPTED_EVENT_NAME,
        key_id.clone(),
        new_owner.clone(),
    )
}

/// Event name for `TransferExecuted`: the accepted handover was finalised and
/// `new_owner` is now the wallet managing the key.
pub const TRANSFER_EXECUTED_EVENT_NAME: Symbol = symbol_short!("own_exec");

/// Stable ownership-executed event field order for downstream indexers.
pub const TRANSFER_EXECUTED_EVENT_DATA_FIELDS: [&str; 6] = [
    "proposal_id",
    "key_id",
    "previous_owner",
    "new_owner",
    "executed_at",
    "ledger",
];

/// Payload emitted when an accepted ownership handover executes.
///
/// Event shape:
/// - topics: `(TRANSFER_EXECUTED_EVENT_NAME, key_id, new_owner)`
/// - data: `TransferExecutedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TransferExecutedEvent {
    /// Sequential id of the executed proposal.
    pub proposal_id: u32,
    /// Key whose management rights changed hands.
    pub key_id: Address,
    /// Wallet that managed the key before execution.
    pub previous_owner: Address,
    /// Wallet that manages the key after execution.
    pub new_owner: Address,
    /// Ledger timestamp at which the handover executed.
    pub executed_at: u64,
    /// Ledger sequence number at the time of execution.
    pub ledger: u32,
}

/// Shared ownership-executed event topics tuple.
pub fn transfer_executed_topics(
    key_id: &Address,
    new_owner: &Address,
) -> (Symbol, Address, Address) {
    (
        TRANSFER_EXECUTED_EVENT_NAME,
        key_id.clone(),
        new_owner.clone(),
    )
}

// --- Creator key vote delegation for proxy voting rights (Issue #1116) ---

/// Event name for proxy voting delegation set.
pub const VOTING_DELEGATED_EVENT_NAME: Symbol = symbol_short!("vote_del");

/// Event name for proxy voting delegation revoked.
pub const VOTE_DELEGATION_REVOKED_EVENT_NAME: Symbol = symbol_short!("v_del_rev");

/// Stable field order for voting delegated payloads.
pub const VOTING_DELEGATED_DATA_FIELDS: [&str; 4] =
    ["holder", "delegate_wallet", "key_id", "weight"];

/// Stable field order for delegation revoked payloads.
pub const VOTE_DELEGATION_REVOKED_DATA_FIELDS: [&str; 4] =
    ["holder", "previous_delegate_wallet", "key_id", "weight"];

/// Event payload emitted when a key holder delegates voting weight to a proxy wallet.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VotingDelegated {
    pub holder: Address,
    pub delegate_wallet: Address,
    pub key_id: Address,
    pub weight: u32,
}

/// Event payload emitted when a key holder revokes their voting delegation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DelegationRevoked {
    pub holder: Address,
    pub previous_delegate_wallet: Address,
    pub key_id: Address,
    pub weight: u32,
}

impl DelegationRevoked {
    pub fn previous_delegate(&self) -> &Address {
        &self.previous_delegate_wallet
    }
}

pub fn voting_delegated_topics(key_id: &Address, holder: &Address) -> (Symbol, Address, Address) {
    (VOTING_DELEGATED_EVENT_NAME, key_id.clone(), holder.clone())
}

pub fn vote_delegation_revoked_topics(
    key_id: &Address,
    holder: &Address,
) -> (Symbol, Address, Address) {
    (
        VOTE_DELEGATION_REVOKED_EVENT_NAME,
        key_id.clone(),
        holder.clone(),
    )
}

pub fn publish_voting_delegated(
    env: &Env,
    holder: &Address,
    delegate_wallet: &Address,
    key_id: &Address,
    weight: u32,
) {
    env.events().publish(
        voting_delegated_topics(key_id, holder),
        VotingDelegated {
            holder: holder.clone(),
            delegate_wallet: delegate_wallet.clone(),
            key_id: key_id.clone(),
            weight,
        },
    );
}

pub fn publish_delegation_revoked(
    env: &Env,
    holder: &Address,
    previous_delegate_wallet: &Address,
    key_id: &Address,
    weight: u32,
) {
    env.events().publish(
        vote_delegation_revoked_topics(key_id, holder),
        DelegationRevoked {
            holder: holder.clone(),
            previous_delegate_wallet: previous_delegate_wallet.clone(),
            key_id: key_id.clone(),
            weight,
        },
    );
}

// --- Restored missing event definitions from origin/main ---

pub const UNIQUE_TRADER_ADDED_EVENT_NAME: Symbol = symbol_short!("uniq_trd");

/// Stable unique-trader event payload.
///
/// Event shape:
/// - topics: `(UNIQUE_TRADER_ADDED_EVENT_NAME, key_id, trader)`
/// - data: `UniqueTraderAddedEvent`
///
/// Emitted exactly once per `(key_id, trader)` pair, on that wallet's first
/// buy or sell. Repeat trades from the same wallet emit nothing, so an indexer
/// can count these events directly instead of de-duplicating trade events.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct UniqueTraderAddedEvent {
    /// Creator whose keys were traded.
    pub key_id: Address,
    /// Wallet trading these keys for the first time.
    pub trader: Address,
    /// Unique trader count after this wallet was counted.
    pub unique_trader_count: u64,
    /// Ledger in which the first trade was recorded.
    pub ledger: u32,
}

/// Shared unique-trader event topics tuple.
pub fn unique_trader_added_topics(
    key_id: &Address,
    trader: &Address,
) -> (Symbol, Address, Address) {
    (
        UNIQUE_TRADER_ADDED_EVENT_NAME,
        key_id.clone(),
        trader.clone(),
    )
}

// ============================================================================
// Feature: key transfer allowances (approve / transfer_from)
// ============================================================================

/// Event name emitted when a holder sets or changes a transfer allowance.
pub const APPROVAL_EVENT_NAME: Symbol = symbol_short!("approval");

/// Stable approval event payload.
///
/// Event shape:
/// - topics: `(APPROVAL_EVENT_NAME, owner, spender)`
/// - data: `ApprovalEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ApprovalEvent {
    /// Holder that granted the allowance.
    pub owner: Address,
    /// Address authorised to spend the allowance.
    pub spender: Address,
    /// Remaining allowance in whole keys after this call.
    pub amount: u32,
    /// Creator whose keys the allowance covers.
    pub key_id: Address,
    /// Ledger in which the approval was recorded.
    pub ledger: u32,
}

/// Shared approval event topics tuple.
pub fn approval_topics(owner: &Address, spender: &Address) -> (Symbol, Address, Address) {
    (APPROVAL_EVENT_NAME, owner.clone(), spender.clone())
}

/// Event name emitted when a spender consumes an allowance via `transfer_from`.
pub const TRANSFER_FROM_EVENT_NAME: Symbol = symbol_short!("xfer_from");

/// Stable transfer-from event payload.
///
/// Event shape:
/// - topics: `(TRANSFER_FROM_EVENT_NAME, key_id, spender)`
/// - data: `TransferFromEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TransferFromEvent {
    /// Creator whose keys moved.
    pub key_id: Address,
    /// Address that consumed the allowance.
    pub spender: Address,
    /// Holder whose balance decreased.
    pub from: Address,
    /// Holder whose balance increased.
    pub to: Address,
    /// Number of keys transferred.
    pub amount: u32,
    /// Allowance remaining after the transfer.
    pub remaining_allowance: u32,
    /// Ledger in which the transfer executed.
    pub ledger: u32,
}

/// Shared transfer-from event topics tuple.
pub fn transfer_from_topics(key_id: &Address, spender: &Address) -> (Symbol, Address, Address) {
    (TRANSFER_FROM_EVENT_NAME, key_id.clone(), spender.clone())
}

// ============================================================================
// Feature: sell tax routed to the buyback pool
// ============================================================================

/// Event name emitted when a creator updates their per-key sell tax.
pub const SELL_TAX_UPDATED_EVENT_NAME: Symbol = symbol_short!("tax_upd");

/// Stable sell-tax-updated event payload.
///
/// Event shape:
/// - topics: `(SELL_TAX_UPDATED_EVENT_NAME, creator)`
/// - data: `SellTaxUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SellTaxUpdatedEvent {
    /// Creator whose sell tax changed.
    pub creator: Address,
    /// Previous sell tax in basis points.
    pub old_tax_bps: u32,
    /// New sell tax in basis points.
    pub new_tax_bps: u32,
    /// Ledger in which the change was recorded.
    pub ledger: u32,
}

/// Shared sell-tax-updated event topics tuple.
pub fn sell_tax_updated_topics(creator: &Address) -> (Symbol, Address) {
    (SELL_TAX_UPDATED_EVENT_NAME, creator.clone())
}

/// Event name emitted on every sell that collects a non-zero tax.
pub const SELL_TAX_COLLECTED_EVENT_NAME: Symbol = symbol_short!("tax_col");

/// Stable sell-tax-collected event payload.
///
/// Event shape:
/// - topics: `(SELL_TAX_COLLECTED_EVENT_NAME, creator, seller)`
/// - data: `SellTaxCollectedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SellTaxCollectedEvent {
    /// Creator whose key was sold.
    pub creator: Address,
    /// Seller that paid the tax.
    pub seller: Address,
    /// Tax amount in XLM stroops, already forwarded to `pool`.
    pub amount: i128,
    /// Address credited with the tax.
    pub pool: Address,
    /// Tax rate applied in basis points.
    pub tax_bps: u32,
    /// Gross sell proceeds the tax was deducted from.
    pub gross_proceeds: i128,
    /// Proceeds actually delivered to the seller after the tax.
    pub net_proceeds: i128,
    /// Buyback pool balance after the tax was added.
    pub pool_balance: i128,
    /// Ledger in which the tax was collected.
    pub ledger: u32,
}

/// Shared sell-tax-collected event topics tuple.
pub fn sell_tax_collected_topics(
    creator: &Address,
    seller: &Address,
) -> (Symbol, Address, Address) {
    (
        SELL_TAX_COLLECTED_EVENT_NAME,
        creator.clone(),
        seller.clone(),
    )
}

// ============================================================================
// Feature: governance quorum escalation
// ============================================================================

/// Event name emitted each time a proposal's voting deadline is extended.
pub const PROPOSAL_EXTENDED_EVENT_NAME: Symbol = symbol_short!("prop_ext");

/// Stable proposal-extended event payload.
///
/// Event shape:
/// - topics: `(PROPOSAL_EXTENDED_EVENT_NAME, creator_id, poll_id)`
/// - data: `ProposalExtendedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ProposalExtendedEvent {
    /// Creator that owns the proposal.
    pub creator_id: Address,
    /// Proposal id, scoped to the creator.
    pub poll_id: u32,
    /// Deadline before the extension.
    pub old_expires_at: u32,
    /// Deadline after the extension.
    pub new_expires_at: u32,
    /// Extensions consumed after this call.
    pub extensions_used: u32,
    /// Maximum extensions allowed by the active config.
    pub max_extensions: u32,
    /// Ledger in which the extension was applied.
    pub ledger: u32,
}

/// Shared proposal-extended event topics tuple.
pub fn proposal_extended_topics(creator_id: &Address, poll_id: u32) -> (Symbol, Address, u32) {
    (PROPOSAL_EXTENDED_EVENT_NAME, creator_id.clone(), poll_id)
}

/// Event name emitted when the protocol admin changes the escalation config.
pub const ESCALATION_CONFIG_UPDATED_EVENT_NAME: Symbol = symbol_short!("esc_cfg");

/// Stable escalation-config-updated event payload.
///
/// Event shape:
/// - topics: `(ESCALATION_CONFIG_UPDATED_EVENT_NAME, admin)`
/// - data: `EscalationConfigUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EscalationConfigUpdatedEvent {
    /// Admin that applied the change.
    pub admin: Address,
    /// `true` when a config was already active before this call.
    pub had_previous_config: bool,
    /// Previously active threshold in basis points.
    pub old_threshold_bps: u32,
    /// Previously active extension duration in ledgers.
    pub old_extension_ledgers: u32,
    /// Previously active extension cap.
    pub old_max_extensions: u32,
    /// Newly active threshold in basis points.
    pub new_threshold_bps: u32,
    /// Newly active extension duration in ledgers.
    pub new_extension_ledgers: u32,
    /// Newly active extension cap.
    pub new_max_extensions: u32,
    /// Ledger in which the change was recorded.
    pub ledger: u32,
}

/// Shared escalation-config-updated event topics tuple.
pub fn escalation_config_updated_topics(admin: &Address) -> (Symbol, Address) {
    (ESCALATION_CONFIG_UPDATED_EVENT_NAME, admin.clone())
}

// ============================================================================
// Trade cooldown violation (issue #974)
// ============================================================================

/// Event name emitted when a buy or sell is blocked by the per-wallet trade
/// cooldown configured via `set_cooldown`.
pub const COOLDOWN_VIOLATION_EVENT_NAME: Symbol = symbol_short!("cd_viol");

/// Event name emitted when a creator updates their trade cooldown duration via
/// `set_cooldown`.
pub const COOLDOWN_SET_EVENT_NAME: Symbol = symbol_short!("cd_set");

/// Payload for a blocked trade due to the per-wallet trade cooldown (issue #974).
///
/// Event shape:
/// - topics: `(COOLDOWN_VIOLATION_EVENT_NAME, creator_id, wallet)`
/// - data: `CooldownViolationEvent`
///
/// Emitted inside `buy_keys_with_referrer` and `sell_key` when a trade is
/// rejected because the per-wallet cooldown window has not yet elapsed.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CooldownViolationEvent {
    /// Wallet whose trade was blocked.
    pub wallet: Address,
    /// Creator whose key was being traded.
    pub creator_id: Address,
    /// Unix timestamp at which the cooldown expires and trading resumes.
    pub expires_at: u64,
    /// Seconds remaining until the cooldown expires.
    pub seconds_remaining: u64,
}

/// Shared cooldown-violation event topics tuple.
pub fn cooldown_violation_topics(
    creator: &Address,
    wallet: &Address,
) -> (Symbol, Address, Address) {
    (
        COOLDOWN_VIOLATION_EVENT_NAME,
        creator.clone(),
        wallet.clone(),
    )
}

/// Payload emitted when a creator sets (or updates) their trade cooldown
/// duration via `set_cooldown` (issue #974).
///
/// Event shape:
/// - topics: `(COOLDOWN_SET_EVENT_NAME, creator_id)`
/// - data: `CooldownSetEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CooldownSetEvent {
    /// Creator whose cooldown was updated.
    pub creator_id: Address,
    /// New cooldown duration in seconds (`0` disables the cooldown).
    pub duration_secs: u64,
}

/// Shared cooldown-set event topics tuple.
pub fn cooldown_set_topics(creator: &Address) -> (Symbol, Address) {
    (COOLDOWN_SET_EVENT_NAME, creator.clone())
}

/// Event name for configuring a graduated bonding curve with supply milestones.
pub const GRADUATED_CURVE_CONFIGURED_EVENT_NAME: Symbol = symbol_short!("grad_crv");

/// Stable graduated curve configured event payload for downstream indexers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct GraduatedCurveConfiguredEvent {
    /// Creator whose curve was configured.
    pub creator: Address,
    /// All milestone `(supply_threshold, exponent)` pairs.
    pub milestones: Vec<(u32, u32)>,
    /// Ledger sequence at the time of configuration.
    pub ledger: u32,
}

/// Shared graduated curve configured event topics tuple.
pub fn graduated_curve_configured_topics(creator: &Address) -> (Symbol, Address) {
    (GRADUATED_CURVE_CONFIGURED_EVENT_NAME, creator.clone())
}

// ============================================================================
// Feature: leaderboard snapshot — top holder rankings (issue #924)
// ============================================================================

/// Event name emitted when a leaderboard snapshot is recorded.
pub const LEADERBOARD_SNAPSHOT_TAKEN_EVENT_NAME: Symbol = symbol_short!("ldbrd_tk");

/// Stable leaderboard-snapshot-taken event payload.
///
/// Event shape:
/// - topics: `(LEADERBOARD_SNAPSHOT_TAKEN_EVENT_NAME, creator_id, snapshot_ledger)`
/// - data: `LeaderboardSnapshotTakenEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LeaderboardSnapshotTakenEvent {
    /// Creator whose holder balances were ranked.
    pub creator_id: Address,
    /// Ledger sequence the snapshot was taken at.
    pub snapshot_ledger: u32,
    /// Leaderboard size `N` in effect when the snapshot was taken.
    pub top_n: u32,
    /// Number of candidate wallets holding at least one key.
    pub total_candidates: u32,
    /// Number of ranked entries actually stored (at most `top_n`).
    pub recorded_entries: u32,
}

/// Shared leaderboard-snapshot-taken event topics tuple.
pub fn leaderboard_snapshot_taken_topics(
    creator_id: &Address,
    snapshot_ledger: u32,
) -> (Symbol, Address, u32) {
    (
        LEADERBOARD_SNAPSHOT_TAKEN_EVENT_NAME,
        creator_id.clone(),
        snapshot_ledger,
    )
}

/// Event name emitted when an aged-out leaderboard snapshot is pruned.
pub const LEADERBOARD_SNAPSHOT_PRUNED_EVENT_NAME: Symbol = symbol_short!("ldbrd_pr");

/// Stable leaderboard-snapshot-pruned event payload.
///
/// Event shape:
/// - topics: `(LEADERBOARD_SNAPSHOT_PRUNED_EVENT_NAME, creator_id, snapshot_ledger)`
/// - data: `LeaderboardSnapshotPrunedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LeaderboardSnapshotPrunedEvent {
    /// Creator the pruned snapshot belonged to.
    pub creator_id: Address,
    /// Ledger sequence of the pruned snapshot.
    pub snapshot_ledger: u32,
    /// Ledger in which the pruning happened.
    pub current_ledger: u32,
}

/// Shared leaderboard-snapshot-pruned event topics tuple.
pub fn leaderboard_snapshot_pruned_topics(
    creator_id: &Address,
    snapshot_ledger: u32,
) -> (Symbol, Address, u32) {
    (
        LEADERBOARD_SNAPSHOT_PRUNED_EVENT_NAME,
        creator_id.clone(),
        snapshot_ledger,
    )
}

/// Event name emitted when the protocol admin updates the leaderboard config.
pub const LEADERBOARD_CONFIG_UPDATED_EVENT_NAME: Symbol = symbol_short!("ldbrd_cf");

/// Stable leaderboard-config-updated event payload.
///
/// Event shape:
/// - topics: `(LEADERBOARD_CONFIG_UPDATED_EVENT_NAME, admin)`
/// - data: `LeaderboardConfigUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LeaderboardConfigUpdatedEvent {
    /// Admin that applied the change.
    pub admin: Address,
    /// Leaderboard size in effect before this call.
    pub old_top_n: u32,
    /// Retention window in ledgers before this call.
    pub old_retention_ledgers: u32,
    /// Leaderboard size after this call.
    pub new_top_n: u32,
    /// Retention window in ledgers after this call.
    pub new_retention_ledgers: u32,
    /// Ledger in which the change was recorded.
    pub ledger: u32,
}

/// Shared leaderboard-config-updated event topics tuple.
pub fn leaderboard_config_updated_topics(admin: &Address) -> (Symbol, Address) {
    (LEADERBOARD_CONFIG_UPDATED_EVENT_NAME, admin.clone())
}

// --- Emergency platform pause events (#1000) ---

/// Event name emitted when the platform-wide emergency halt activates.
pub const PLATFORM_PAUSED_EVENT_NAME: Symbol = symbol_short!("plat_pau");

/// Event name emitted when a platform resume is queued behind the 24h timelock.
pub const PLATFORM_RESUME_QUEUED_EVENT_NAME: Symbol = symbol_short!("plat_rq");

/// Event name emitted when the platform-wide emergency halt is lifted.
pub const PLATFORM_RESUMED_EVENT_NAME: Symbol = symbol_short!("plat_res");

/// Event name emitted when a per-key emergency pause override is set or cleared.
pub const KEY_PAUSE_OVERRIDE_EVENT_NAME: Symbol = symbol_short!("key_pau");

/// Stable field order for [`PlatformPausedEvent`].
pub const PLATFORM_PAUSED_EVENT_DATA_FIELDS: [&str; 2] = ["actor", "timestamp"];

/// Stable field order for [`PlatformResumeQueuedEvent`].
pub const PLATFORM_RESUME_QUEUED_EVENT_DATA_FIELDS: [&str; 2] = ["actor", "executable_at"];

/// Stable field order for [`PlatformResumedEvent`].
pub const PLATFORM_RESUMED_EVENT_DATA_FIELDS: [&str; 2] = ["actor", "timestamp"];

/// Stable field order for [`KeyPauseOverrideEvent`].
pub const KEY_PAUSE_OVERRIDE_EVENT_DATA_FIELDS: [&str; 3] = ["key_id", "paused", "actor"];

/// Stable platform-paused event payload.
///
/// Event shape:
/// - topics: `(PLATFORM_PAUSED_EVENT_NAME, actor)`
/// - data: `PlatformPausedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PlatformPausedEvent {
    /// First signer of the multisig call.
    pub actor: Address,
    /// Ledger timestamp at which the halt took effect.
    pub timestamp: u64,
}

/// Stable platform-resume-queued event payload.
///
/// Event shape:
/// - topics: `(PLATFORM_RESUME_QUEUED_EVENT_NAME, actor)`
/// - data: `PlatformResumeQueuedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PlatformResumeQueuedEvent {
    /// First signer of the multisig call.
    pub actor: Address,
    /// Ledger timestamp from which `resume_platform` may execute.
    pub executable_at: u64,
}

/// Stable platform-resumed event payload.
///
/// Event shape:
/// - topics: `(PLATFORM_RESUMED_EVENT_NAME, actor)`
/// - data: `PlatformResumedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PlatformResumedEvent {
    /// First signer of the multisig call.
    pub actor: Address,
    /// Ledger timestamp at which the halt was lifted.
    pub timestamp: u64,
}

/// Stable key-pause-override event payload.
///
/// Event shape:
/// - topics: `(KEY_PAUSE_OVERRIDE_EVENT_NAME, key_id)`
/// - data: `KeyPauseOverrideEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyPauseOverrideEvent {
    /// Key whose override changed.
    pub key_id: Address,
    /// `true` when the key was paused, `false` when the override was cleared.
    pub paused: bool,
    /// First signer of the multisig call.
    pub actor: Address,
}

/// Shared platform-paused event topics tuple.
pub fn platform_paused_topics(actor: &Address) -> (Symbol, Address) {
    (PLATFORM_PAUSED_EVENT_NAME, actor.clone())
}

/// Shared platform-resume-queued event topics tuple.
pub fn platform_resume_queued_topics(actor: &Address) -> (Symbol, Address) {
    (PLATFORM_RESUME_QUEUED_EVENT_NAME, actor.clone())
}

/// Shared platform-resumed event topics tuple.
pub fn platform_resumed_topics(actor: &Address) -> (Symbol, Address) {
    (PLATFORM_RESUMED_EVENT_NAME, actor.clone())
}

/// Shared key-pause-override event topics tuple.
pub fn key_pause_override_topics(key_id: &Address) -> (Symbol, Address) {
    (KEY_PAUSE_OVERRIDE_EVENT_NAME, key_id.clone())
}

// --- Vault rebalancing ---

/// Event name for a completed vault rebalance.
pub const REBALANCE_EXECUTED_EVENT_NAME: Symbol = symbol_short!("rebal");

/// Stable field order for rebalance execution payloads.
pub const REBALANCE_EXECUTED_DATA_FIELDS: [&str; 6] = [
    "creator",
    "trades",
    "allocations",
    "total_value",
    "max_slippage_bps",
    "ledger",
];

/// One trade executed by a vault rebalance.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RebalanceTrade {
    pub from_key: Address,
    pub to_key: Address,
    pub amount: i128,
    pub reference_price: i128,
    pub execution_price: i128,
    pub slippage_bps: u32,
}

/// Stable rebalance execution payload for downstream indexers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RebalanceExecutedEvent {
    pub creator: Address,
    pub trades: Vec<RebalanceTrade>,
    pub allocations: Vec<VaultAllocation>,
    pub total_value: i128,
    pub max_slippage_bps: u32,
    pub ledger: u32,
}

/// Shared rebalance execution event topics tuple.
pub fn rebalance_executed_topics(creator: &Address) -> (Symbol, Address) {
    (REBALANCE_EXECUTED_EVENT_NAME, creator.clone())
}

// --- Dynamic fee tiers ---

/// Event name for a dynamic fee tier transition.
pub const FEE_TIER_CHANGED_EVENT_NAME: Symbol = symbol_short!("fee_tier");

/// Stable field order for fee tier transition payloads.
pub const FEE_TIER_CHANGED_DATA_FIELDS: [&str; 5] = [
    "old_tier_index",
    "new_tier_index",
    "old_protocol_bps",
    "new_protocol_bps",
    "ledger",
];

/// Stable fee tier transition payload for downstream indexers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct FeeTierChangedEvent {
    pub old_tier_index: u32,
    pub new_tier_index: u32,
    pub old_protocol_bps: u32,
    pub new_protocol_bps: u32,
    pub ledger: u32,
}

/// Sentinel tier index used before any dynamic fee tier has ever been resolved.
pub const NO_FEE_TIER_INDEX: u32 = u32::MAX;

// --- Staking and stake receipt NFT ---

/// Event name for a minted stake receipt NFT.
pub const STAKE_NFT_MINTED_EVENT_NAME: Symbol = symbol_short!("snft_mint");

/// Event name for a burned stake receipt NFT.
pub const STAKE_NFT_BURNED_EVENT_NAME: Symbol = symbol_short!("snft_burn");

/// Event name for a stake receipt NFT transfer.
pub const STAKE_NFT_TRANSFERRED_EVENT_NAME: Symbol = symbol_short!("transfer");

/// Stable field order for stake receipt mint payloads.
pub const STAKE_NFT_MINTED_DATA_FIELDS: [&str; 7] = [
    "token_id",
    "creator",
    "stake_id",
    "owner",
    "amount",
    "unlock_ledger",
    "ledger",
];

/// Stable field order for stake receipt transfer payloads.
pub const STAKE_NFT_TRANSFERRED_DATA_FIELDS: [&str; 7] = [
    "token_id", "creator", "stake_id", "from", "to", "amount", "ledger",
];

/// Stable field order for stake receipt burn payloads.
pub const STAKE_NFT_BURNED_DATA_FIELDS: [&str; 6] = [
    "token_id", "creator", "stake_id", "owner", "amount", "ledger",
];

/// Stable stake receipt mint payload for downstream indexers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakeNftMintedEvent {
    pub token_id: u64,
    pub creator: Address,
    pub stake_id: u32,
    pub owner: Address,
    pub amount: u32,
    pub unlock_ledger: u32,
    pub ledger: u32,
}

/// Stable stake receipt transfer payload for downstream indexers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakeNftTransferredEvent {
    pub token_id: u64,
    pub creator: Address,
    pub stake_id: u32,
    pub from: Address,
    pub to: Address,
    pub amount: i128,
    pub ledger: u32,
}

/// Stable stake receipt burn payload for downstream indexers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakeNftBurnedEvent {
    pub token_id: u64,
    pub creator: Address,
    pub stake_id: u32,
    pub owner: Address,
    pub amount: u32,
    pub ledger: u32,
}

/// Shared stake receipt mint event topics tuple.
pub fn stake_nft_minted_topics(creator: &Address, owner: &Address) -> (Symbol, Address, Address) {
    (STAKE_NFT_MINTED_EVENT_NAME, creator.clone(), owner.clone())
}

/// Shared stake receipt transfer event topics tuple.
pub fn stake_nft_transferred_topics(from: &Address, to: &Address) -> (Symbol, Address, Address) {
    (STAKE_NFT_TRANSFERRED_EVENT_NAME, from.clone(), to.clone())
}

/// Shared stake receipt burn event topics tuple.
pub fn stake_nft_burned_topics(creator: &Address, owner: &Address) -> (Symbol, Address, Address) {
    (STAKE_NFT_BURNED_EVENT_NAME, creator.clone(), owner.clone())
}

// --- Bonding curve reset ---

/// Event name for a bonding curve reset.
pub const CURVE_RESET_EVENT_NAME: Symbol = symbol_short!("curve_rst");

/// Stable field order for curve reset payloads.
pub const CURVE_RESET_DATA_FIELDS: [&str; 7] = [
    "creator",
    "old_supply",
    "new_supply",
    "preset",
    "slope",
    "reset_count",
    "ledger",
];

/// Stable curve reset payload for downstream indexers.
///
/// Event shape:
/// - topics: `(CURVE_RESET_EVENT_NAME, creator)`
/// - data: `CurveResetEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CurveResetEvent {
    /// Creator whose curve was reset.
    pub creator: Address,
    /// Supply immediately before the reset. Always `0`; a reset requires a full buyback.
    pub old_supply: u32,
    /// Supply immediately after the reset, the relaunch point.
    pub new_supply: u32,
    /// Curve shape applied from `new_supply` onwards.
    pub preset: crate::CurvePreset,
    /// Curve slope applied from `new_supply` onwards.
    pub slope: i128,
    /// Running count of successful resets for this creator.
    pub reset_count: u32,
    /// Ledger sequence number at reset time.
    pub ledger: u32,
}

/// Shared curve reset event topics tuple.
pub fn curve_reset_topics(creator: &Address) -> (Symbol, Address) {
    (CURVE_RESET_EVENT_NAME, creator.clone())
}

// --- Key rating ---

/// Event name for a key rating submission.
pub const KEY_RATED_EVENT_NAME: Symbol = symbol_short!("key_rated");

/// Event payload emitted when a key holder rates a creator.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyRatedEvent {
    pub creator: Address,
    pub rater: Address,
    pub score: u32,
    pub total_score: u64,
    pub count: u32,
    pub average_score_scaled: u32,
    pub ledger: u32,
}

/// Shared key rated event topics tuple.
pub fn key_rated_topics(creator: &Address, rater: &Address) -> (Symbol, Address, Address) {
    (KEY_RATED_EVENT_NAME, creator.clone(), rater.clone())
}

// ============================================================================
// Liquidity Bootstrapping (#1117)
// ============================================================================

// --- Creator-funded revenue sharing (issue #1135) ---

/// Event name for creator-funded revenue sharing distribution.
pub const REVENUE_DISTRIBUTED_EVENT_NAME: Symbol = symbol_short!("rev_dist");

/// Stable revenue distribution event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(REVENUE_DISTRIBUTED_EVENT_NAME, key_id, distribution_id)`
/// - data: `RevenueDistributedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RevenueDistributedEvent {
    pub key_id: Address,
    pub distribution_id: u32,
    pub amount: i128,
    pub allocated_amount: i128,
    pub remainder: i128,
    pub total_eligible_keys: u32,
    pub eligible_count: u32,
}

pub const REVENUE_DISTRIBUTED_DATA_FIELDS: [&str; 7] = [
    "key_id",
    "distribution_id",
    "amount",
    "allocated_amount",
    "remainder",
    "total_eligible_keys",
    "eligible_count",
];

/// Shared revenue distributed event topics tuple.
pub fn revenue_distributed_topics(
    key_id: &Address,
    distribution_id: u32,
) -> (Symbol, Address, u32) {
    (
        REVENUE_DISTRIBUTED_EVENT_NAME,
        key_id.clone(),
        distribution_id,
    )
}

/// Event name for creator-funded revenue share claim.
pub const REVENUE_SHARE_CLAIMED_EVENT_NAME: Symbol = symbol_short!("rev_clm");

/// Stable revenue share claim event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(REVENUE_SHARE_CLAIMED_EVENT_NAME, distribution_id, claimant)`
/// - data: `RevenueShareClaimedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RevenueShareClaimedEvent {
    pub distribution_id: u32,
    pub key_id: Address,
    pub claimant: Address,
    pub amount: i128,
}

pub const REVENUE_SHARE_CLAIMED_DATA_FIELDS: [&str; 4] =
    ["distribution_id", "key_id", "claimant", "amount"];

/// Shared revenue share claimed event topics tuple.
pub fn revenue_share_claimed_topics(
    distribution_id: u32,
    claimant: &Address,
) -> (Symbol, u32, Address) {
    (
        REVENUE_SHARE_CLAIMED_EVENT_NAME,
        distribution_id,
        claimant.clone(),
    )
}

pub const BOOTSTRAP_STARTED_EVENT_NAME: Symbol = symbol_short!("boot_strt");
pub const BOOTSTRAP_ENDED_EVENT_NAME: Symbol = symbol_short!("boot_end");

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BootstrapStartedEvent {
    pub key_id: Address,
    pub start_price: i128,
    pub end_price: i128,
    pub start_time: u64,
    pub duration_seconds: u64,
    pub ledger: u32,
}

pub fn bootstrap_started_topics(key_id: &Address) -> (Symbol, Address) {
    (BOOTSTRAP_STARTED_EVENT_NAME, key_id.clone())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BootstrapEndedEvent {
    pub key_id: Address,
    pub final_price: i128,
    pub timestamp: u64,
    pub ledger: u32,
}

pub fn bootstrap_ended_topics(key_id: &Address) -> (Symbol, Address) {
    (BOOTSTRAP_ENDED_EVENT_NAME, key_id.clone())
}
