//! Centralized event names and helpers for consistent event emission.
//!
//! This module provides a single source of truth for event names used throughout
//! the contract, reducing string duplication and ensuring consistency across
//! event emission paths.
//!
//! ### Event Schema Stability
//!
//! Downstream indexers rely on the stable ordering of fields in event payloads.
//! When modifying event structures:
//! - **Do not reorder** existing fields.
//! - **Add new fields** only at the end of the structure to maintain compatibility.
//! - **Avoid removing fields**; if a field is deprecated, keep it with a default value.
//!
//! This approach ensures that indexers can reliably parse event data across
//! different contract versions.
//!
//! ### Horizon Event Encoding
//!
//! Events are consumed off-chain through the Stellar Horizon / Soroban RPC
//! event stream, which decodes topics and data as ScVal types:
//! - The first topic of every event is a `Symbol` from the `*_EVENT_NAME`
//!   constants in this module (at most 9 characters, via `symbol_short!`).
//!   Never publish a raw `String` as a topic.
//! - Remaining topics are `Address` or integer (`u32`) identifiers, built by
//!   the shared `*_topics` helpers.
//! - Event data is a `#[contracttype]` struct (encoded as an ScVal map) or a
//!   plain ScVal value; every struct documents its topics and fields under
//!   "Event shape" in this module.
//! - Events without a dedicated struct carry a plain value as data:
//!   `pause`, `unpause`, `blk_add`, `blk_rem` (`()`), `dl_set` (deadline
//!   ledger `Option<u32>`), `ttl_ext` (new live-until ledger `u32`),
//!   `gpause_on` / `gpause_of` (ledger `u32`), `poll_new` (expiry ledger) and
//!   `poll_vote` (`(option_index, weight)`).
//!
//! ### Quote-Related Event Field Semantics
//!
//! - `supply`: Number of keys in circulation after the trade (for buy/sell events)
//! - `payment`: Total amount paid by the buyer (for buy events, ≥ key price)

use crate::{
    constants, extend_key_ttl_to_full_window, read_creator_supply, read_registered_creator_profile,
    CreatorKeysContract, CreatorKeysContractArgs, CreatorKeysContractClient,
};
use soroban_sdk::{
    contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

/// Event name for protocol trade fee collected on a buy or sell.
pub const FEE_COLLECTED_EVENT_NAME: Symbol = symbol_short!("fee_coll");

/// Stable fee collection event payload for downstream indexers.
///
/// Event shape:
/// - topics: `(FEE_COLLECTED_EVENT_NAME, treasury)`
/// - data: `FeeCollectedEvent`
///
/// Emitted on every buy and sell once the protocol trade fee is configured,
/// carrying the deducted amount and the treasury address that received it.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct FeeCollectedEvent {
    /// Treasury address that received the fee.
    pub treasury: Address,
    /// Fee amount deducted from the trade.
    pub amount: i128,
    /// Ledger sequence number at the time of the trade.
    pub ledger: u32,
}

/// Shared fee collected event topics tuple.
pub fn fee_collected_topics(treasury: &Address) -> (Symbol, Address) {
    (FEE_COLLECTED_EVENT_NAME, treasury.clone())
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
/// Topic index for the event name in common event topic tuples.
pub const TOPIC_EVENT_NAME_INDEX: u32 = 0;

/// Topic index for the creator address in common event topic tuples.
pub const TOPIC_CREATOR_INDEX: u32 = 1;

/// Topic index for the buyer/seller/actor address in common event topic tuples.
pub const TOPIC_BUYER_INDEX: u32 = 2;

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

/// Event shape:
/// - topics: `(PROTOCOL_FEE_RECIPIENT_UPDATED_EVENT_NAME, admin)`
/// - data: `ProtocolFeeRecipientUpdatedEvent`
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ProtocolFeeRecipientUpdatedEvent {
    pub old_recipient: Address,
    pub new_recipient: Address,
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

// --- Timelock events ---

/// Event name for config change proposed.
pub const CONFIG_CHANGE_PROPOSED_EVENT_NAME: Symbol = symbol_short!("tl_prop");

/// Event name for config change executed.
pub const CONFIG_CHANGE_EXECUTED_EVENT_NAME: Symbol = symbol_short!("tl_exec");

/// Event name for config change cancelled.
pub const CONFIG_CHANGE_CANCELLED_EVENT_NAME: Symbol = symbol_short!("tl_canc");

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
pub const HOLDING_CAP_UPDATED_EVENT_NAME: Symbol = symbol_short!("hold_cap");
pub const WHITELIST_UPDATED_EVENT_NAME: Symbol = symbol_short!("wl_upd");
pub const REFERRAL_REGISTERED_EVENT_NAME: Symbol = symbol_short!("ref_reg");
pub const REFERRAL_REWARD_ALLOCATED_EVENT_NAME: Symbol = symbol_short!("ref_rwd");
pub const REFERRAL_REWARDS_CLAIMED_EVENT_NAME: Symbol = symbol_short!("ref_clm");
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
        if env.storage().persistent().has(&delegate_key) {
            return Err(PollError::Unauthorized);
        }

        let balance_key = constants::storage::holder_balance_key(&creator_id, &voter);
        let weight: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
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
    /// Updated name, or empty string if unchanged.
    pub name: String,
    /// Updated bio, or empty string if unchanged.
    pub bio: String,
    /// Updated avatar URI, or empty string if unchanged.
    pub avatar_uri: String,
    /// Ledger sequence number at the time of the update.
    pub ledger: u32,
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

/// Shared reputation-updated event topics tuple.
pub fn reputation_updated_topics(creator: &Address) -> (Symbol, Address) {
    (REPUTATION_UPDATED_EVENT_NAME, creator.clone())
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
