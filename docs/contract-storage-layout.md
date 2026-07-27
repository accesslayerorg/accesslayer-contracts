# Contract Storage Layout & Key Naming Conventions

This reference document describes the complete persistent storage layout, key naming conventions, data types, TTL policies, and read/write function mappings for the Access Layer `creator-keys` Soroban smart contract.

---

## 1. Overview & DataKey Architecture

All contract state is stored in Soroban's persistent storage schema defined by the `DataKey` enum in [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs):

```rust
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
}
```

---

## 2. Comprehensive Storage Key Directory

Below is the complete table of every persistent storage key used by the contract:

| Key Variant | Key Construction / Format | Stored Data Type | TTL Policy | Read Functions / Accessors | Write Functions / Mutators |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `Creator(Address)` | `DataKey::Creator(creator)` | `CreatorProfile` | Extended on trade (`CREATOR_TTL_LEDGERS`) | `get_creator`, `get_creator_details`, `read_registered_creator_profile` | `register_creator`, `update_creator_fee_recipient` |
| `FeeConfig` | `DataKey::FeeConfig` | `fee::FeeConfig` | Persistent | `get_fee_config`, `get_protocol_fee_view`, `read_protocol_fee_config` | `set_fee_config` |
| `KeyPrice` | `DataKey::KeyPrice` | `i128` (stroops) | Persistent | `get_key_price`, `get_buy_quote`, `get_sell_quote` | `set_key_price` |
| `KeyBalance(Address, Address)` | `DataKey::KeyBalance(creator, holder)` | `u32` | Extended on trade; removed on 0 balance | `get_key_balance`, `get_holder_key_count` | `buy_key`, `sell_key`, `transfer_keys` |
| `TreasuryAddress` | `DataKey::TreasuryAddress` | `Address` | Persistent | `get_treasury_address` | `set_treasury_address` |
| `AdminAddress` | `DataKey::AdminAddress` | `Address` | Persistent | `get_protocol_admin` | `set_protocol_admin` |
| `ProtocolFeeRecipient` | `DataKey::ProtocolFeeRecipient` | `Address` | Persistent | `get_protocol_fee_recipient` | `set_protocol_fee_recipient`, `update_protocol_fee_recipient` |
| `ProtocolFeeRecipientBalance` | `DataKey::ProtocolFeeRecipientBalance` | `i128` (stroops) | Persistent | `get_protocol_recipient_balance` | `sell_key`, `buyback` |
| `CreatorFeeBalance(Address)` | `DataKey::CreatorFeeBalance(creator)` | `i128` (stroops) | Extended on trade | `get_creator_fee_balance` | `buy_key`, `sell_key`, `withdraw_creator_fee` |
| `ProtocolStateVersion` | `DataKey::ProtocolStateVersion` | `u32` | Persistent | `get_protocol_state_version` | `set_fee_config` (increments version) |
| `Paused` | `DataKey::Paused` | `bool` | Persistent | `get_is_paused` | `pause`, `unpause` |
| `DividendPerKeyAccumulated(Address)` | `DataKey::DividendPerKeyAccumulated(creator)` | `i128` (per key) | Extended on trade | `read_dividend_accumulator` | `distribute_dividend` |
| `HolderDividendCheckpoint(Address, Address)` | `DataKey::HolderDividendCheckpoint(creator, holder)` | `i128` | Per-holder persistent | `compute_claimable_dividend`, `settle_holder_dividends` | `claim_dividend`, `settle_holder_dividends` |
| `HolderDividendPending(Address, Address)` | `DataKey::HolderDividendPending(creator, holder)` | `i128` | Per-holder persistent | `compute_claimable_dividend`, `settle_holder_dividends` | `claim_dividend`, `settle_holder_dividends` |
| `LockedAllocation(Address)` | `DataKey::LockedAllocation(creator)` | `LockedAllocation` | Extended on trade; deleted after claim | `get_locked_allocation` | `register_creator`, `claim_locked_allocation` |
| `MaxSupply(Address)` | `DataKey::MaxSupply(creator)` | `u32` | Extended on trade | `get_max_supply` | `register_creator` |
| `CurveSlope` | `DataKey::CurveSlope` | `i128` | Persistent | `get_curve_slope` | `set_curve_slope` |
| `CurvePreset(Address)` | `DataKey::CurvePreset(creator)` | `CurvePreset` | Extended on trade | `get_curve_preset` | `register_creator` |
| `TreasuryBalance` | `DataKey::TreasuryBalance` | `i128` (stroops) | Persistent | `get_treasury_balance` | `buy_key`, `withdraw_treasury` |
| `CoCreator(Address)` | `DataKey::CoCreator(creator)` | `CoCreatorConfig` | Extended on trade | `read_co_creator_config` | `register_creator` |
| `CoCreatorFeeBalance(Address, Address)` | `DataKey::CoCreatorFeeBalance(creator, co_creator)` | `i128` (stroops) | Extended on trade | `get_co_creator_fee_balance` | `buy_key`, `sell_key`, `withdraw_co_creator_fee` |
| `Whitelist(Address)` | `DataKey::Whitelist(creator)` | `WhitelistConfig` | Persistent | `get_whitelist_config`, `get_whitelist_status` | `register_creator` |
| `MaxKeysPerWallet(Address)` | `DataKey::MaxKeysPerWallet(creator)` | `u32` | Persistent | `get_max_keys_per_wallet` | `register_creator` |
| `ReferralFeeBps` | `DataKey::ReferralFeeBps` | `u32` | Persistent | `get_referral_fee_bps` | `set_referral_fee_bps` |
| `DiscountTiers` | `DataKey::DiscountTiers` | `Vec<DiscountTier>` | Persistent | `get_discount_tiers` | `set_discount_tiers` |
| `CreatorVolume(Address)` | `DataKey::CreatorVolume(creator)` | `i128` (stroops) | Persistent | `get_creator_volume` | `buy_key`, `sell_key` |

---

## 3. Composite Key Naming & Construction Conventions

Composite keys in `creator-keys` bind multiple entity addresses into a single storage tuple.

### Naming Pattern
Constructors for composite keys live in `constants::storage` in [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs). They accept references to addresses and return `DataKey` enum instances.

### Example: Holder Balance Key
```rust
pub fn holder_balance_key(creator_id: &Address, holder: &Address) -> DataKey {
    DataKey::KeyBalance(creator_id.clone(), holder.clone())
}
```

### Usage
- `creator_id` identifies the key creator contract scope.
- `holder` identifies the wallet address holding the keys.
- **Order Invariant**: `holder_balance_key(creator, holder)` is strictly ordered (`(creator, holder)`). Order matters and reversing parameters produces a different storage key.

Other composite key helpers follow the same parameter convention:
- `co_creator_fee_balance(creator, co_creator)` -> `DataKey::CoCreatorFeeBalance(creator, co_creator)`
- `holder_dividend_checkpoint(creator, holder)` -> `DataKey::HolderDividendCheckpoint(creator, holder)`
- `holder_dividend_pending(creator, holder)` -> `DataKey::HolderDividendPending(creator, holder)`

---

## 4. TTL Extension Behavior & Minimum Thresholds

### TTL Extension Mechanism
Soroban persistent storage entries expire unless their TTL (time-to-live) is extended. The `creator-keys` contract automatically extends the TTL for all primary creator-related storage entries after every successful trade (`buy_key` or `sell_key`).

### Constants
- **`CREATOR_TTL_LEDGERS`**: `6311520` ledgers (~2 years at 5 seconds per ledger).
- **Extension Function**: `extend_creator_ttl(env: &Env, creator: &Address)`

### Threshold Decision Logic
Storage extension is evaluated using the pure helper `ttl::should_extend`:

```rust
pub mod ttl {
    pub fn should_extend(current_ttl: u32, threshold: u32) -> bool {
        current_ttl < threshold
    }
}
```

- **Trigger Condition**: Extension only fires when the remaining ledger count drops strictly below `threshold` (`current_ledger`).
- **Target Ledger**: Entries are extended to `current_ledger + CREATOR_TTL_LEDGERS`.
- **Event Notification**: When an extension occurs on the main creator key, a `(TTL_EXTENDED_EVENT_NAME, creator)` event is emitted with the target ledger count.
