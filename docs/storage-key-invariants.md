# Storage Key Invariants

This document describes the storage model, key structure, and invariants that must be maintained across all contract operations in Access Layer contracts.

For quote-specific storage behavior (`get_buy_quote`, `get_sell_quote`), see [quote-storage-keys.md](./quote-storage-keys.md).

For Soroban TTL and rent-bump expectations specific to creator profiles and holder balances, see [creator-state-storage-ttl.md](./creator-state-storage-ttl.md).

For key staking storage keys and the data model, see [key-staking-storage.md](./key-staking-storage.md).

## Overview

Access Layer contracts use Soroban's persistent storage to maintain creator profiles, key balances, and protocol configuration. Understanding these storage invariants is critical for contributors working on contract logic.

## Storage Keys

Storage keys are defined in the `DataKey` enum in [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs):

```rust
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
}
```

### Key Types

| Key                            | Type               | Value Type       | Purpose                                                    |
| ------------------------------ | ------------------ | ---------------- | ---------------------------------------------------------- |
| `Creator(Address)`             | Per-creator        | `CreatorProfile` | Stores creator registration metadata, supply, holder_count, and fee recipient |
| `FeeConfig`                    | Global             | `FeeConfig`      | Stores protocol-wide fee split configuration               |
| `KeyPrice`                     | Global             | `i128`           | Stores the fixed price for all keys across all creators    |
| `KeyBalance(Address, Address)` | Per-creator-holder | `u32`            | Stores how many keys a holder owns for a specific creator  |
| `HolderRegistry(Address)`      | Per-creator        | `Vec<Address>`   | Append-only list of wallets that have held keys for a creator |
| `TreasuryAddress`              | Global             | `Address`        | Protocol treasury address for fee routing                  |
| `AdminAddress`                 | Global             | `Address`        | Protocol admin address for configuration                   |
| `ProtocolFeeRecipient`         | Global             | `Address`        | Protocol fee recipient address                             |
| `PendingUpgradeWasm`           | Global             | `BytesN<32>`     | WASM hash staged for the next timelocked upgrade           |
| `LastAppliedWasm`              | Global             | `BytesN<32>`     | WASM hash applied by the most recent timelocked upgrade    |
| `PreviousWasm`                 | Global             | `BytesN<32>`     | WASM hash in effect immediately before `LastAppliedWasm`   |
| `UpgradeApprovalVote(Address, u32)` | Per-action    | `bool`           | One admin's multi-sig approval vote for upgrade `action_id` |
| `RewardRate(Address)`          | Per-pool           | `RewardRate`     | Stores creator-configured LP reward rate and updated_at timestamp |

### Timelocked Upgrade Storage-Key Notes

| Field | Storage Key | Read Ownership | Write Ownership |
| ----- | ----------- | -------------- | --------------- |
| Live logic build | `DataKey::LastAppliedWasm` (`constants::storage::LAST_APPLIED_WASM`) | `get_logic_address` | `execute_action` (admin-auth, timelock elapsed, threshold met) |
| Staged upgrade target | `DataKey::PendingUpgradeWasm` (`constants::storage::PENDING_UPGRADE_WASM`) | `get_upgrade_target` | `upgrade` stages it; `execute_action` or `cancel_action` clears it |
| Prior logic build | `DataKey::PreviousWasm` (`constants::storage::PREVIOUS_WASM`) | `get_previous_wasm` | `execute_action` (admin-auth, timelock elapsed, threshold met) |
| Upgrade approvals | `DataKey::UpgradeApprovalVote` (`constants::storage::upgrade_approval_vote`) | `get_upgrade_approvals` | `approve_upgrade` sets, `approve_upgrade`; `execute_action` and `cancel_action` clear |

**Invariants**:

- `PendingUpgradeWasm` is present only while an `Upgrade` action is queued. It must be cleared on both execution and cancellation, so a stale hash can never be applied by a later action.
- `LastAppliedWasm` only changes through `execute_action`. There is deliberately no ungated setter and no rollback entrypoint — an incident is reversed by proposing a fresh timelocked upgrade back to `get_previous_wasm`.
- `UpgradeApprovalVote` keys are scoped by `action_id` and cleared on execution and cancellation, so a vote can never be inherited by a subsequent upgrade.
- `PreviousWasm` is only overwritten once a second upgrade has been applied; it is `None` before that.

**Storage layout / backward compatibility**: all four keys are additive `DataKey` variants holding values that no existing key reads. Deploys that omit this change are unaffected, and a contract upgraded from an earlier build keeps every pre-existing key untouched — `execute_action` swaps only WASM code, never storage. No migration step is required in either direction.

### Fee Configuration Storage-Key Notes

| Field | Storage Key | Read Ownership | Write Ownership |
| ----- | ----------- | -------------- | --------------- |
| Protocol fee split (`creator_bps`, `protocol_bps`) | `DataKey::FeeConfig` (`constants::storage::FEE_CONFIG`) | `get_fee_config`, `get_protocol_fee_view`, `get_creator_fee_bps`, `get_creator_treasury_share`, quote methods through shared readers | `set_fee_config` (admin-auth only) |
| Protocol fee recipient | `DataKey::ProtocolFeeRecipient` (`constants::storage::PROTOCOL_FEE_RECIPIENT`) | `get_protocol_fee_recipient` | `set_protocol_fee_recipient` (admin-auth only) |
| Graduated milestone fee reduction (#1076) | `DataKey::KeyMilestones(Address)` (`constants::storage::key_milestones`) | `get_current_milestone`, `check_and_apply_milestone` | `set_milestones` (admin-auth only) writes the configured tiers; `check_and_apply_milestone` writes the applied tier only when a newly reached tier lowers the fee |

`set_fee_config` uses the shared `fee::assert_valid_fee_bps` guard to keep key writes aligned with the current constants (`BPS_MAX = 10_000`, `PROTOCOL_BPS_MAX = 5_000`) before persisting the config.

**Storage layout / backward compatibility**: `DataKey::KeyMilestones(Address)` is an additive `DataKey` variant appended at the end, so all existing variant discriminants are unchanged. No existing key is read or modified, and no migration is required in either direction.

### Price Alert Storage-Key Notes

Price alerts (issue #1030) use their own `PriceAlertDataKey` enum in [`creator-keys/src/price_alerts.rs`](../creator-keys/src/price_alerts.rs), like the other feature modules. All entries are persistent and extended to the full `CREATOR_TTL_LEDGERS` window on write.

| Field | Storage Key | Value | Read Ownership | Write Ownership |
| ----- | ----------- | ----- | -------------- | --------------- |
| Active alerts on a key | `PriceAlertDataKey::KeyAlerts(key_id)` | `Vec<PriceAlert>` | every buy and sell on `key_id`, `get_alerts`, `register_alert`, `cancel_alert` | `register_alert` appends; a crossing buy/sell or `cancel_alert` removes |
| A wallet's alert references | `PriceAlertDataKey::WalletAlerts(wallet)` | `Vec<PriceAlertRef>` | `get_alerts`, `register_alert`, `cancel_alert` | `register_alert`, `cancel_alert` (wallet-auth) |
| Last issued alert id | `PriceAlertDataKey::NextAlertId` | `u64` | `register_alert` | `register_alert` |

**Invariants**:

- `KeyAlerts(key_id)` is the single source of truth for whether an alert is active. An alert is active if and only if it is in that vector. The entry is removed once it is empty, and holds at most `MAX_ACTIVE_ALERTS_PER_KEY` (16) alerts.
- A buy or sell reads only `KeyAlerts` for the key it traded, and writes it only if an alert fired. Firing does **not** touch `WalletAlerts`, so a trade's alert cost stays at one ledger entry however many wallets it fires for.
- `WalletAlerts(wallet)` may therefore reference alerts that have already fired. `get_alerts` resolves each reference against `KeyAlerts` and drops the stale ones; `register_alert` and `cancel_alert` rewrite the entry with only active references. Its length never exceeds `MAX_ACTIVE_ALERTS_PER_WALLET` (10).
- Alert ids are never reused. A fired or cancelled id can never become active again.
- A trade that reads `KeyAlerts` extends its TTL, so a key that keeps trading keeps its alerts live.

**Storage layout / backward compatibility**: the keys are new and additive. No existing key is read or changed, so no migration is needed in either direction.

### LP Reward Rate Storage-Key Notes

LP reward rate management (issue #1032) uses `DataKey::RewardRate(Address)` in [`creator-keys/src/lp_reward.rs`](../creator-keys/src/lp_reward.rs).

| Field | Storage Key | Value | Read Ownership | Write Ownership |
| ----- | ----------- | ----- | -------------- | --------------- |
| LP reward rate (`rate_bps`, `updated_at`) | `lp_reward::DataKey::RewardRate(key_id)` | `RewardRate` | `get_reward_rate`, `set_reward_rate` | `set_reward_rate` (creator-auth only) |

**Invariants**:

- `RewardRate(key_id)` stores the configured reward rate in basis points (`1..=5000`) and the ledger timestamp of the update.
- Unset pools return zero defaults (`rate_bps = 0`, `updated_at = 0`) before any rate is set.
- Rate changes are prospective only; existing positions' `reward_debt` snapshots remain untouched.

**Storage layout / backward compatibility**: `DataKey::RewardRate(Address)` is an additive storage key with a zero default. No existing storage key is read or modified, and no migration is required.

### Key Ownership Handover Storage-Key Notes (#1098)

Key ownership handover (issue #1098) uses its own `OwnershipDataKey` enum in [`creator-keys/src/key_ownership_transfer.rs`](../creator-keys/src/key_ownership_transfer.rs), like the other feature modules. All entries are persistent and extended to the full `CREATOR_TTL_LEDGERS` window on write.

| Field | Storage Key | Value | Read Ownership | Write Ownership |
| ----- | ----------- | ----- | -------------- | --------------- |
| Wallet that currently manages a key | `OwnershipDataKey::Owner(key_id)` | `Address` (absent = `key_id`, the registered creator) | `get_key_owner`, and the owner checks in `propose_transfer` / `cancel_transfer` / `execute_transfer` | `execute_transfer` |
| A proposal record | `OwnershipDataKey::Proposal(proposal_id)` | `OwnershipTransferProposal` | `get_transfer_proposal`, `accept_transfer`, `execute_transfer`, `cancel_transfer` | `propose_transfer`, `accept_transfer` write; `execute_transfer` and `cancel_transfer` delete |
| The key's single live proposal | `OwnershipDataKey::ActiveProposal(key_id)` | `u32` proposal id | `propose_transfer` | `propose_transfer` writes; `execute_transfer` and `cancel_transfer` delete |
| Next proposal id to issue | `OwnershipDataKey::NextProposalId` | `u32` (ids start at `0`) | `propose_transfer` | `propose_transfer` |

**Invariants**:

- `Owner(key_id)` is absent until a handover executes, so every pre-existing reader that treats the registered creator as the owner keeps seeing `key_id` itself. It only ever changes through `execute_transfer`.
- At most one live proposal exists per key: `ActiveProposal(key_id)` is written with every proposal and cleared on execution or cancellation, so a stale proposal can never execute over a newer one. A proposal whose 48h window passed without acceptance is dropped and replaced by the next `propose_transfer` for that key.
- `Proposal(proposal_id)` is deleted on both execution and cancellation, so ids are never reused against live state and `get_transfer_proposal` reports `TransferProposalNotFound` for consumed ids.
- Proposal ids are globally sequential and monotonic (`NextProposalId`), independent of `key_id`, so two keys never share a proposal id.

**Storage layout / backward compatibility**: the keys are new and additive and live outside `DataKey`, so no existing key is read or changed and no migration is needed in either direction.

### Key Freeze Storage-Key Notes (#1174)

Key-level freeze/unfreeze (issue #1174) uses its own `KeyFreezeDataKey` enum in [`creator-keys/src/key_freeze.rs`](../creator-keys/src/key_freeze.rs), like the other feature modules. All entries are persistent and extended to the full `CREATOR_TTL_LEDGERS` window on write.

| Field | Storage Key | Value | Read Ownership | Write Ownership |
| ----- | ----------- | ----- | -------------- | --------------- |
| Current freeze status of a key | `KeyFreezeDataKey::Status(key_id)` | `KeyStatus` (`Active` \| `Frozen`) | `read_key_status`, `is_key_frozen`, `assert_key_not_frozen`, `get_key_status`, `is_key_frozen` | `freeze` writes `Frozen`; `unfreeze` writes `Active` |
| Freeze audit log of a key | `KeyFreezeDataKey::Log(key_id)` | `Vec<FreezeLogEntry>` | `read_freeze_log`, `get_freeze_log` | `freeze` and `unfreeze` append one entry each |

**Invariants**:

- `Status(key_id)` is absent until the key is frozen for the first time; an absent entry means `KeyStatus::Active`, so every pre-existing key keeps trading normally without a migration.
- `Status(key_id)` only ever changes through `freeze` and `unfreeze`, both of which require the acting admin's authorization and a role in the key's multi-sig admin set (or the protocol admin).
- `Log(key_id)` is append-only: `freeze` and `unfreeze` push one entry and never rewrite or remove entries, so the full history of a key can be replayed off-chain. Each entry records the action, the reason, the acting admin and the ledger timestamp.
- Every trade path that moves a position reads `Status(key_id)` before touching any balance, so a frozen key rejects the trade with `KeyFrozen` without side effects.

**Storage layout / backward compatibility**: the keys are new and additive and live outside `DataKey`, so no existing key is read or changed and no migration is needed in either direction.

## Storage Invariants

These invariants must hold true after every contract operation:

### 1. Creator Profile Invariants

**Invariant**: A creator exists if and only if `DataKey::Creator(address)` is present in storage.

### Creator registration metadata ownership

**Key ownership**: `DataKey::Creator(address)` is the single source of truth for creator registration metadata and may only be created by `register_creator`.

- `register_creator` must set this key exactly once for a new creator address.
- The creator address must authorize registration via `creator.require_auth()`.
- The profile stores `creator`, `handle`, `supply`, `holder_count`, and `fee_recipient`.
- Indexers may derive registration status, handle, supply, holder_count, and fee recipient from this single entry.
- Any future change to creator registration metadata storage shape should be treated like an event/schema breaking change and require explicit coordination.

**Implications**:

**Implications**:

- `register_creator` must set this key
- `get_creator` must check for key presence
- Buy/sell operations must verify creator exists before proceeding

**Invariant**: For any registered creator, `profile.supply == sum of all KeyBalance(creator, *)`.

**Implications**:

- When buying a key, increment both `profile.supply` and `KeyBalance(creator, buyer)`
- When selling a key, decrement both `profile.supply` and `KeyBalance(creator, seller)`
- When processing a creator buyback, decrement both `profile.supply` and `KeyBalance(creator, creator)` by the burned amount
- Never modify supply without updating the corresponding balance

### Holder registry invariants

`HolderRegistry(creator)` is an append-only list of addresses that have received
keys through purchases, transfers, airdrops, or claimed allocations. Every
balance-credit path adds the recipient only if they are not already present.
Selling or transferring all keys out never removes an address. The registry
uses persistent storage and its TTL is extended to the full creator TTL window
on every read of an existing entry and every write.

The storage key is additive, so no migration is required for existing state.
Soroban storage cannot enumerate the full set of historical `KeyBalance`
holders on upgrade; consequently, the registry tracks holders from their first
successful balance credit after this version is deployed and does not backfill
pre-upgrade history.

**Invariant**: For any registered creator, `profile.holder_count == count of non-zero KeyBalance(creator, *)`.

**Implications**:

- When a holder's balance goes from 0 to 1, increment `holder_count`
- When a holder's balance goes from 1 to 0, decrement `holder_count`
- Never increment/decrement holder_count for balance changes that don't cross the zero boundary

**Example**:

```rust
// Buying first key for a holder
let current_balance = 0;
let new_balance = 1;
// Must increment holder_count because balance crossed from 0 to non-zero

// Buying second key for same holder
let current_balance = 1;
let new_balance = 2;
// Must NOT increment holder_count because balance was already non-zero
```

### 2. Key Balance Invariants

**Invariant**: `KeyBalance(creator, holder)` is only stored if the balance is non-zero.

**Implications**:

- When balance reaches 0, the key should be removed from storage (or left as 0)
- `get_key_balance` must return 0 for missing keys
- Storage reads use `.get().unwrap_or(0)` pattern

**Invariant**: Key balances are always non-negative (`u32` type enforces this).

**Implications**:

- Selling when balance is 0 must return `InsufficientBalance` error
- Use `checked_sub` to detect underflow attempts

### 3. Global Configuration Invariants

**Invariant**: `KeyPrice` must be positive when set.

**Implications**:

- `set_key_price` must reject zero or negative values
- Quote operations must check if price is set before computing

**Invariant**: `FeeConfig` must satisfy `creator_bps + protocol_bps == 10000` and `protocol_bps <= 5000`.

**Implications**:

- `set_fee_config` must validate before storing
- Fee computation can assume valid config if present
- See [docs/fee-assumptions.md](./fee-assumptions.md) for details

**Invariant**: Global config keys are optional until explicitly set by admin.

**Implications**:

- Operations requiring config must check for presence and return appropriate errors
- Read-only views should return `None` or default values for unset config

### 4. Supply and Balance Conservation

**Invariant**: Total supply across all creators equals the sum of all key balances.

**Mathematical representation**:

```
∀ creator: creator.supply = Σ(KeyBalance(creator, holder)) for all holders
```

**Implications**:

- Supply and balances must be updated atomically in the same transaction
- Never update supply without updating a corresponding balance
- Use checked arithmetic to prevent overflow

**Invariant**: Holder count equals the number of holders with non-zero balance.

**Mathematical representation**:

```
∀ creator: creator.holder_count = count({holder | KeyBalance(creator, holder) > 0})
```

**Implications**:

- Holder count must be updated when balance crosses zero boundary
- Holder count should never exceed supply (each holder has at least 1 key)

## Storage Access Patterns

### Reading Creator Profiles

**Helper function**: `read_creator_profile(env, creator) -> Option<CreatorProfile>`

**Usage**:

```rust
// For operations that tolerate missing creators
let profile = read_creator_profile(&env, &creator);
if profile.is_none() {
    // Handle unregistered creator
}

// For operations that require a creator
let profile = read_registered_creator_profile(&env, &creator)?;
```

**Invariant**: Always use helper functions instead of direct storage access to maintain consistency.

### Reading Key Balances

**Helper function**: `read_key_balance(env, creator) -> u32`

**Usage**:

```rust
// Returns 0 for unregistered creators
let supply = read_key_balance(&env, &creator);

// For holder balances
let balance_key = constants::storage::key_balance(&creator, &holder);
let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
```

**Invariant**: Always use `.unwrap_or(0)` when reading balances to handle missing keys.

### Writing Storage

**Pattern**: Always update related storage keys atomically.

**Example** (from `buy_key`):

```rust
// 1. Update holder balance
let new_balance = current_balance.checked_add(1)?;
env.storage().persistent().set(&balance_key, &new_balance);

// 2. Update holder count if needed
if current_balance == 0 {
    profile.holder_count = profile.holder_count.checked_add(1)?;
}

// 3. Update supply
profile.supply = profile.supply.checked_add(1)?;

// 4. Write updated profile
env.storage().persistent().set(&creator_key, &profile);
```

**Invariant**: Never return early between related storage updates. Use `?` operator carefully.

## Storage Consistency Checks

When modifying contract logic, verify these consistency checks:

### Before Committing Code

1. **Supply matches balances**: After buy/sell, verify `profile.supply` was updated correctly
2. **Holder count matches non-zero balances**: Verify holder_count increments/decrements only when crossing zero
3. **No orphaned balances**: Ensure balance updates always have corresponding supply updates
4. **Overflow protection**: Use `checked_add`, `checked_sub` for all arithmetic

### In Tests

Add assertions to verify invariants:

```rust
#[test]
fn test_buy_key_maintains_invariants() {
    // ... setup and buy operation ...

    let profile = client.get_creator(&creator).unwrap();
    let buyer_balance = client.get_key_balance(&creator, &buyer);

    // Verify supply invariant
    assert_eq!(profile.supply, buyer_balance, "supply must equal sum of balances");

    // Verify holder count invariant
    assert_eq!(profile.holder_count, 1, "holder_count must be 1 for single holder");
}
```

## Common Pitfalls

### 1. Forgetting to Update Holder Count

**Wrong**:

```rust
// Buying first key
profile.supply += 1;
// Forgot to increment holder_count!
```

**Right**:

```rust
// Buying first key
if current_balance == 0 {
    profile.holder_count = profile.holder_count.checked_add(1)?;
}
profile.supply = profile.supply.checked_add(1)?;
```

### 2. Updating Supply Without Balance

**Wrong**:

```rust
// Only updating supply
profile.supply += 1;
env.storage().persistent().set(&creator_key, &profile);
// Forgot to update KeyBalance!
```

**Right**:

```rust
// Update balance first
let new_balance = current_balance.checked_add(1)?;
env.storage().persistent().set(&balance_key, &new_balance);

// Then update supply
profile.supply = profile.supply.checked_add(1)?;
env.storage().persistent().set(&creator_key, &profile);
```

### 3. Not Using Checked Arithmetic

**Wrong**:

```rust
profile.supply = profile.supply + 1; // Can panic on overflow
```

**Right**:

```rust
profile.supply = profile.supply.checked_add(1).ok_or(ContractError::Overflow)?;
```

### 4. Returning Early Between Updates

**Wrong**:

```rust
// Update balance
env.storage().persistent().set(&balance_key, &new_balance);

// Some operation that might fail
some_operation()?; // If this fails, balance is updated but supply is not!

// Update supply
profile.supply += 1;
```

**Right**:

```rust
// Validate everything first
some_operation()?;

// Then update all storage atomically
env.storage().persistent().set(&balance_key, &new_balance);
profile.supply = profile.supply.checked_add(1)?;
env.storage().persistent().set(&creator_key, &profile);
```

## Storage Migration

If storage structure changes in a future version:

1. **Document the migration**: Describe old and new formats
2. **Provide migration function**: Write a function to convert old data to new format
3. **Test migration**: Verify all invariants hold after migration
4. **Version the storage**: Consider adding a version field to detect old data

## Debugging Storage Issues

### Viewing Storage in Tests

```rust
#[test]
fn debug_storage_state() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);

    // ... perform operations ...

    // Read storage directly
    let profile: Option<CreatorProfile> = env.as_contract(&contract_id, || {
        env.storage().persistent().get(&DataKey::Creator(creator.clone()))
    });

    println!("Profile: {:?}", profile);
}
```

### Common Storage Errors

| Error                 | Likely Cause                             | Solution                               |
| --------------------- | ---------------------------------------- | -------------------------------------- |
| `NotRegistered`       | Creator key not in storage               | Call `register_creator` first          |
| `Overflow`            | Supply or holder_count exceeded u32::MAX | Check arithmetic operations            |
| `InsufficientBalance` | KeyBalance is 0 or missing               | Verify holder owns keys before selling |

## Questions

For questions about storage invariants or debugging storage issues:

1. Review this document and [docs/error-codes.md](./error-codes.md)
2. Check existing tests for examples of correct storage usage
3. Ask in pull request comments or discussions
