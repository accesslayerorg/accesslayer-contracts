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
//! ### Quote-Related Event Field Semantics
//!
//! - `supply`: Number of keys in circulation after the trade (for buy/sell events)
//! - `payment`: Total amount paid by the buyer (for buy events, ≥ key price)

use crate::{
    constants, read_creator_supply, read_registered_creator_profile, CreatorKeysContract,
    CreatorKeysContractArgs, CreatorKeysContractClient,
};
use soroban_sdk::{
    contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

/// Event name for protocol trade fee collected on a buy or sell.
pub const FEE_COLLECTED_EVENT_NAME: Symbol = symbol_short!("fee_coll");

/// Event name for a sell rejected by the anti-flash-trade lockup window.
pub const LOCKUP_BLOCKED_EVENT_NAME: Symbol = symbol_short!("lck_blk");

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
    /// Ledger sequence number at the time of the sale.
    pub ledger: u32,
}

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

/// Stable field order for dividend distributed event payloads.
pub const DIVIDEND_DISTRIBUTED_DATA_FIELDS: [&str; 4] =
    ["creator", "total_amount", "snapshot_supply", "ledger"];

/// Stable field order for dividend claimed event payloads.
pub const DIVIDEND_CLAIMED_DATA_FIELDS: [&str; 3] = ["creator", "claimant", "amount"];

/// Stable field order for co-creator fee earned event payloads.
pub const CO_CREATOR_FEE_EARNED_DATA_FIELDS: [&str; 4] =
    ["creator_id", "co_creator", "amount", "ledger"];

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DividendDistributedEvent {
    pub creator: Address,
    pub total_amount: i128,
    pub snapshot_supply: u32,
    pub ledger: u32,
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AllocationLockedEvent {
    pub creator_id: Address,
    pub amount: u32,
    pub unlock_ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AllocationClaimedEvent {
    pub creator_id: Address,
    pub amount: u32,
    pub ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ProtocolFeeRecipientUpdatedEvent {
    pub old_recipient: Address,
    pub new_recipient: Address,
}

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

/// Shared TTL extension event topics tuple.
pub fn ttl_extended_topics(creator: &Address) -> (Symbol, Address) {
    (TTL_EXTENDED_EVENT_NAME, creator.clone())
}

// --- Supply cap events ---

/// Event name for supply cap set.
pub const SUPPLY_CAP_SET_EVENT_NAME: Symbol = symbol_short!("cap_set");

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

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PauseProposedEvent {
    pub creator_id: Address,
    pub proposer: Address,
    pub ledger: u32,
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestingCreatedEvent {
    pub creator_id: Address,
    pub beneficiary: Address,
    pub total_keys: u32,
    pub start_ledger: u32,
    pub vesting_period_ledgers: u32,
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ConfigChangeProposedEvent {
    pub proposal_id: u32,
    pub proposer: Address,
    pub change_type: u32,
    pub proposed_at: u32,
    pub execution_not_before: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ConfigChangeExecutedEvent {
    pub proposal_id: u32,
    pub executed_at: u32,
}

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
pub const KEYS_BURNED_EVENT_NAME: Symbol = symbol_short!("burned");
pub const SELF_FREEZE_APPLIED_EVENT_NAME: Symbol = symbol_short!("sf_add");
pub const SELF_FREEZE_LIFTED_EVENT_NAME: Symbol = symbol_short!("sf_del");

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SelfFreezeEvent {
    pub key_id: Address,
    pub wallet: Address,
    pub quantity: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CircuitBreakerTriggeredEvent {
    pub pre_price: i128,
    pub post_price: i128,
}

pub fn circuit_breaker_triggered_topics() -> Symbol {
    CIRCUIT_BREAKER_TRIGGERED_EVENT_NAME
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralFeePaidEvent {
    pub referrer: Address,
    pub amount: i128,
}

pub fn referral_fee_paid_topics() -> Symbol {
    REFERRAL_FEE_PAID_EVENT_NAME
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct WhitelistEnabledEvent {
    pub creator: Address,
}

pub fn whitelist_enabled_topics(creator: &Address) -> (Symbol, Address) {
    (WHITELIST_ENABLED_EVENT_NAME, creator.clone())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct WhitelistDisabledEvent {
    pub creator: Address,
}

pub fn whitelist_disabled_topics(creator: &Address) -> (Symbol, Address) {
    (WHITELIST_DISABLED_EVENT_NAME, creator.clone())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AddressWhitelistedEvent {
    pub creator: Address,
    pub address: Address,
}

pub fn address_whitelisted_topics(creator: &Address) -> (Symbol, Address) {
    (ADDRESS_WHITELISTED_EVENT_NAME, creator.clone())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AddressRemovedEvent {
    pub creator: Address,
    pub address: Address,
}

pub fn address_removed_topics(creator: &Address) -> (Symbol, Address) {
    (ADDRESS_REMOVED_EVENT_NAME, creator.clone())
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

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PollClosedEvent {
    pub creator_id: Address,
    pub poll_id: u32,
    pub total_weight: u32,
    pub quorum_reached: bool,
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

pub fn read_poll(env: &Env, creator_id: &Address, poll_id: u32) -> Result<Poll, PollError> {
    env.storage()
        .persistent()
        .get(&poll_storage_key(creator_id, poll_id))
        .ok_or(PollError::PollNotFound)
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
            (POLL_VOTE_EVENT_NAME, creator_id, poll_id, voter),
            (option_index, weight),
        );

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
    /// threshold, returns `Err(PollError::QuorumNotReached)`.
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

        if quorum_bps > 0 {
            if circulating_supply == 0 {
                return Err(PollError::QuorumNotReached);
            }
            let total_weight_bps = (poll.total_weight as u128)
                .checked_mul(10_000)
                .ok_or(PollError::Overflow)?;
            let required_bps = (circulating_supply as u128)
                .checked_mul(quorum_bps as u128)
                .ok_or(PollError::Overflow)?;

            if total_weight_bps < required_bps {
                return Err(PollError::QuorumNotReached);
            }
        }

        poll.closed = true;
        env.storage()
            .persistent()
            .set(&poll_storage_key(&creator_id, poll_id), &poll);

        env.events().publish(
            poll_closed_topics(&creator_id, poll_id),
            PollClosedEvent {
                creator_id: creator_id.clone(),
                poll_id,
                total_weight: poll.total_weight,
                quorum_reached: true,
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

/// Event name for bonding curve migration.
pub const CURVE_MIGRATED_EVENT_NAME: Symbol = symbol_short!("curve_mig");

/// Stable curve migrated event payload for downstream indexers.
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

/// Event name for the protocol trade fee collected on a buy or sell.
pub const FEE_COLLECTED_EVENT_NAME: Symbol = symbol_short!("fee_coll");

/// Event name for a sell rejected by the anti-flash-trade lockup window.
pub const LOCKUP_BLOCKED_EVENT_NAME: Symbol = symbol_short!("lck_blk");

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
pub fn stake_topics(creator: &Address, holder: &Address, stake_id: u32) -> (Symbol, Address, Address, u32) {
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
    (STAKE_EXTENDED_EVENT_NAME, creator.clone(), holder.clone(), stake_id)
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
    (EARLY_UNSTAKE_EVENT_NAME, creator.clone(), holder.clone(), stake_id)
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
    (STAKE_REWARD_CLAIMED_EVENT_NAME, creator.clone(), holder.clone(), stake_id)
}


// ============================================================================
// Launch Penalty (#798)
// ============================================================================

/// Event name for launch penalty applied on sell.
pub const LAUNCH_PENALTY_APPLIED_EVENT_NAME: Symbol = symbol_short!("lnch_pnl");

/// Stable launch penalty applied event payload for downstream indexers.
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
    (LAUNCH_PENALTY_APPLIED_EVENT_NAME, creator.clone(), seller.clone())
}

/// Event name for set_launch_penalty.
pub const LAUNCH_PENALTY_SET_EVENT_NAME: Symbol = symbol_short!("lnch_set");

/// Stable set launch penalty event payload for downstream indexers.
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
