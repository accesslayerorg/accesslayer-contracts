# Contract Event Naming Conventions and Topic Format

This document defines the naming conventions and structural requirements for events emitted by AccessLayer smart contracts. Following these conventions ensures that off-chain indexers and consumers can reliably discover, filter, and parse contract state changes.

## Event Naming Convention

- **Format**: All event names are `Symbol` types.
- **Casing**: Use `lowercase` for event names.
- **Definition**: Event names must be defined as `pub const` constants in a centralized `events` module (e.g., [events.rs](../creator-keys/src/events.rs)).
- **Macro**: Prefer the `symbol_short!` macro for names up to 10 characters to optimize storage and gas.

Example:
```rust
pub const BUY_EVENT_NAME: Symbol = symbol_short!("buy");
```

## Topic Format

Events use a predictable topic structure to support efficient filtering. Topics are emitted as a list (tuple) where each position has a specific meaning.

| Index | Type | Description |
| :--- | :--- | :--- |
| 0 | `Symbol` | **Event Name**: The canonical name of the event. Used by indexers to identify the schema. |
| 1 | `Address` | **Primary Entity**: The main actor or object of the event (e.g., `creator` address). |
| 2 | `Address` | **Secondary Entity** (Optional): A second party involved (e.g., `buyer` address). |

### Topic Stability
The meaning of a topic at a given index must never change. If a new indexed field is required, it must be added at the next available index.

## Data (Payload) Format

The event data contains the detailed payload of the event. It can be structured as either a `contracttype` struct or a tuple.

### Structural Requirements
- **Stability**: Field order in the data payload must remain stable across contract versions.
- **Appends Only**: New fields may only be added to the end of a struct or tuple.
- **Documentation**: Stable field orders should be documented in [events.rs](../creator-keys/src/events.rs) using a constant array of strings for indexers to reference.

Example of field documentation:
```rust
pub const BUY_EVENT_DATA_FIELDS: [&str; 2] = ["supply", "payment"];
```

## Existing Event Definitions

The following table summarizes the events currently implemented in the `creator-keys` contract.

| Event Name | Topics (Index 0, 1, 2) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `register` | `(Symbol("register"), creator)` | `creator`, `handle`, `supply`, `holder_count`, `creator_bps`, `protocol_bps` | `struct CreatorRegisteredEvent` |
| `buy` | `(Symbol("buy"), creator, buyer)` | `supply`, `payment` | `tuple (u32, i128)` |
| `sell` | `(Symbol("sell"), creator, seller)` | `supply` | `tuple (u32)` |

### Events Added Alongside Reputation, Allowances, Sell Tax and Escalation

| Event Name | Topics (Index 0, 1, 2) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `rep_upd` | `(Symbol("rep_upd"), creator)` | `creator`, `old_score`, `new_score`, `delta`, `reason`, `ledger` | `struct ReputationUpdatedEvent` |
| `approval` | `(Symbol("approval"), owner, spender)` | `owner`, `spender`, `amount`, `key_id`, `ledger` | `struct ApprovalEvent` |
| `xfer_from` | `(Symbol("xfer_from"), key_id, spender)` | `key_id`, `spender`, `from`, `to`, `amount`, `remaining_allowance`, `ledger` | `struct TransferFromEvent` |
| `tax_upd` | `(Symbol("tax_upd"), creator)` | `creator`, `old_tax_bps`, `new_tax_bps`, `ledger` | `struct SellTaxUpdatedEvent` |
| `tax_col` | `(Symbol("tax_col"), creator, seller)` | `creator`, `seller`, `amount`, `pool`, `tax_bps`, `gross_proceeds`, `net_proceeds`, `pool_balance`, `ledger` | `struct SellTaxCollectedEvent` |
| `prop_ext` | `(Symbol("prop_ext"), creator_id, poll_id)` | `creator_id`, `poll_id`, `old_expires_at`, `new_expires_at`, `extensions_used`, `max_extensions`, `ledger` | `struct ProposalExtendedEvent` |
| `esc_cfg` | `(Symbol("esc_cfg"), admin)` | `admin`, `had_previous_config`, `old_threshold_bps`, `old_extension_ledgers`, `old_max_extensions`, `new_threshold_bps`, `new_extension_ledgers`, `new_max_extensions`, `ledger` | `struct EscalationConfigUpdatedEvent` |

Notes for indexers:

- `rep_upd` reports the **raw** `delta` requested by the triggering action. When a
  negative delta is clamped at the zero floor, `old_score + delta` will not equal
  `new_score`; the score is path-dependent once it has been floored. Treat
  `new_score` as authoritative and `delta` as the attempted adjustment.
- `tax_col` carries both `gross_proceeds` and `net_proceeds` so a consumer can
  verify `net_proceeds + amount == gross_proceeds` without re-deriving the fee
  split. `pool` is the zero address until an admin configures a pool, in which
  case the tax is still held internally and accounted in `pool_balance`.
- `xfer_from` is emitted in addition to the existing `hc_chg` holder-count event
  whenever a delegated transfer crosses a holder's zero balance boundary. The
  holder-count event reports the **net** count, so a full transfer that swaps a
  departing sender for an arriving recipient produces no `hc_chg` event.
- `PollClosedEvent` gained a `finalized_by_exhaustion` boolean. `quorum_reached`
  now always reflects real participation, so a proposal that closed only because
  its escalation budget ran out reports `quorum_reached == false` together with
  `finalized_by_exhaustion == true`.

### Events Added Alongside the Timelocked Logic Upgrade

| Event Name | Topics (Index 0, 1) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `upg_appr` | `(Symbol("upg_appr"), action_id)` | `admin`, `action_id`, `approvals`, `threshold` | `struct UpgradeApprovedEvent` |
| `logic_upg` | `(Symbol("logic_upg"), action_id)` | `action_id`, `admin`, `old_wasm_hash`, `new_wasm_hash`, `previous_wasm_hash`, `applied_at_ledger` | `struct LogicUpgradedEvent` |

Notes for indexers:

- Topic index 1 is the timelock `action_id` (a `u32`), **not** an `Address`. These
  two events are the only ones in the contract whose secondary topic is not an
  address, so indexers that assume `Address` at index 1 must special-case them.
- `old_wasm_hash` and `previous_wasm_hash` are both absent on a first upgrade
  (`Option::None`), since neither a prior build nor a rollback target exists yet.
- `logic_upg` is emitted **in addition to** the pre-existing
  `UpgradeExecutedEvent` when a timelocked upgrade applies, so existing
  consumers of the older event keep working. `LogicUpgradedEvent` is the
  hash-specific, indexer-facing event and is the one to prefer.
- Because the upgrade swaps code via `update_current_contract_wasm`, `logic_upg`
  is emitted by the **new** build at the same contract address; there is no
  separate logic-contract address to follow.
### Emergency Platform Pause Events (#1000)

| Event Name | Topics (Index 0, 1, 2) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `plat_pau` | `(Symbol("plat_pau"), actor)` | `actor`, `timestamp` | `struct PlatformPausedEvent` |
| `plat_rq` | `(Symbol("plat_rq"), actor)` | `actor`, `executable_at` | `struct PlatformResumeQueuedEvent` |
| `plat_res` | `(Symbol("plat_res"), actor)` | `actor`, `timestamp` | `struct PlatformResumedEvent` |
| `key_pau` | `(Symbol("key_pau"), key_id)` | `key_id`, `paused`, `actor` | `struct KeyPauseOverrideEvent` |

`actor` is the first signer of the multisig call. `timestamp` and
`executable_at` are ledger timestamps in seconds, not ledger sequence numbers.

### Price Alert Events (#1030)

| Event Name | Topics (Index 0, 1, 2) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `alert_reg` | `(Symbol("alert_reg"), key_id, wallet)` | `wallet`, `key_id`, `alert_id`, `direction`, `target_price`, `ledger` | `struct PriceAlertRegisteredEvent` |
| `alert_trg` | `(Symbol("alert_trg"), key_id, wallet)` | `wallet`, `key_id`, `alert_id`, `direction`, `target_price`, `new_price`, `ledger` | `struct PriceAlertTriggeredEvent` |
| `alert_cxl` | `(Symbol("alert_cxl"), key_id, wallet)` | `wallet`, `key_id`, `alert_id`, `ledger` | `struct PriceAlertCancelledEvent` |

Notes for indexers:

- `alert_trg` is emitted from inside a buy or sell (`buy_key`, `buy_keys`,
  `buy_key_with_referrer`, `forward_buy`, `sell_key`, `batch_buy`,
  `batch_buy_v2`, `batch_sell`, `batch_sell_v2`, `reinvest_dividend`), after
  that trade's own `buy` / `sell` event. One trade can emit several, one per
  alert it fires, in registration order.
- Each `alert_id` produces at most one `alert_trg` or one `alert_cxl`, never
  both and never twice. After either event the alert is gone.
- `new_price` is the key's bonding-curve spot price after the trade (the price
  of the next key at the new supply), in the same raw `i128` units as the key
  price. It is not the price paid for the trade.
- `direction` is the `AlertDirection` enum, encoded as a `u32`: `0` = `Above`,
  `1` = `Below`.

### Referral Tier Upgrade Events (#1034)

| Event Name | Topics (Index 0, 1) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `tier_upg` | `(Symbol("tier_upg"), referrer)` | `referrer`, `old_tier`, `new_tier`, `volume`, `reward_rate_bps` | `struct ReferralTierUpgraded` |

Notes for indexers:

- `tier_upg` is emitted when a referrer's cumulative referred trade volume crosses a tier threshold.
- Tiers are ratchet-only (`Bronze` = 0, `Silver` = 1, `Gold` = 2) and never downgrade regardless of subsequent volume drops.
- `reward_rate_bps` reports the new reward rate in basis points (10,000 bps = 100%).

### LP Reward Rate Update Events (#1032)

| Event Name | Topics (Index 0, 1) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `lp_rate` | `(Symbol("lp_rate"), key_id)` | `key_id`, `old_rate_bps`, `new_rate_bps`, `updated_at` | `struct RewardRateUpdated` |

Notes for indexers:

- `lp_rate` is emitted by `set_reward_rate` when a creator updates the reward rate for their LP pool.
- Topic index 1 is `key_id` (`Address`).
- `old_rate_bps` reports the previous rate in basis points (or `0` on first configuration); `new_rate_bps` reports the newly configured rate (`1..=5000` bps).
- `updated_at` is the ledger timestamp in seconds.

### Per-Key Dynamic Swap Fee Events (#1056)

| Event Name | Topics (Index 0, 1) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `fee_adj` | `(Symbol("fee_adj"), key_id)` | `key_id`, `previous_fee_bps`, `new_fee_bps`, `rolling_volume`, `timestamp` | `struct FeeAdjustedEvent` |

Notes for indexers:

- `rolling_volume` is gross trade volume in the raw price base units at the
  transition point. An expiry transition reports volume before the current
  trade is added; a bracket crossing caused by that trade reports volume after
  it is added.
- Swaps select their fee from pre-trade volume. Fee changes caused by the
  current swap apply to later swaps.
- Time expiration and bracket updates do not autonomously emit events. A
  later successful trade detects and emits the transition; reads never emit
  or mutate.
- No event is emitted if the fee bps is unchanged.

### Supply Milestone Fee Reduction Events (#1076)

| Event Name | Topics (Index 0, 1) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `ms_reach` | `(Symbol("ms_reach"), key_id)` | `key_id`, `tier`, `supply`, `new_fee_bps`, `ledger` | `struct MilestoneReachedEvent` |

Notes for indexers:

- `ms_reach` is emitted by `check_and_apply_milestone` (also invoked post-swap from buy/sell core paths) when a key's circulating supply newly reaches a milestone tier whose fee lowers the fee currently in effect.
- Topic index 1 is `key_id` (`Address`).
- `tier` is the 1-based index of the milestone tier reached; tiers are applied in ascending supply order and never roll back.
- `supply` is the circulating supply after the trade that crossed the threshold.
- `new_fee_bps` reports the new fee in basis points (`0..=10000`).
- `ledger` is the ledger sequence number at the time of the reduction.

### Key Liquidity Depth Events (#1133)

| Event Name | Topics (Index 0, 1) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `depth_upd` | `(Symbol("depth_upd"), key_id)` | `key_id`, `price`, `amount`, `side`, `volume` | `struct DepthUpdated` |

Notes for indexers:

- `depth_upd` is emitted on each order placement (`record_order`) and cancellation (`cancel_order_depth`) when order book liquidity depth changes.
- Topic index 1 is `key_id` (`Address`).
- `price` is the affected price level (`i128`).
- `amount` is the order volume added or cancelled (`i128`).
- `side` is the order book side (`OrderSide::Buy` = 0, `OrderSide::Sell` = 1).
- `volume` is the remaining volume at this specific price level after the update.

### Creator Key Vote Delegation Events (#1116)

| Event Name | Topics (Index 0, 1, 2) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `vote_del` | `(Symbol("vote_del"), key_id, holder)` | `holder`, `delegate_wallet`, `key_id`, `weight` | `struct VotingDelegated` |
| `v_del_rev` | `(Symbol("v_del_rev"), key_id, holder)` | `holder`, `previous_delegate_wallet`, `key_id`, `weight` | `struct DelegationRevoked` |

Notes for indexers:

- `vote_del` is emitted by `delegate_votes` when a key holder assigns their voting weight on `key_id` to a proxy delegate wallet.
- `v_del_rev` is emitted by `revoke_delegation` when a key holder revokes their voting delegation on `key_id`.
- Topic index 1 is `key_id` (`Address`) and topic index 2 is `holder` (`Address`).
- `weight` reports the key balance held by the delegator at delegation/revocation time.
- Note that because delegated weight dynamically tracks the delegator's balance while delegation is active, indexers tracking current voting weight should compute delegate weight dynamically from the active delegator set and their balances.

### Key Freeze Events (#1174)

| Event Name | Topics (Index 0, 1, 2) | Data Fields | Data Type |
| :--- | :--- | :--- | :--- |
| `key_frz` | `(Symbol("key_frz"), key_id)` | `key_id`, `admin_wallet`, `reason`, `timestamp`, `ledger` | `struct KeyFrozenEvent` |
| `key_unfrz` | `(Symbol("key_unfrz"), key_id)` | `key_id`, `admin_wallet`, `timestamp`, `ledger` | `struct KeyUnfrozenEvent` |

Notes for indexers:

- `key_frz` is emitted by `freeze` when a key's multi-sig admins freeze it. The key rejects buys, sells and transfers with `ContractError::KeyFrozen` from that point on.
- `key_unfrz` is emitted by `unfreeze` when the freeze is lifted and the key trades normally again.
- Topic index 1 is `key_id` (`Address`); there is no topic index 2. The acting admin is carried in the payload as `admin_wallet`.
- `reason` is the admin-supplied text stored in the freeze audit log; it is empty on the unfreeze event.
- `timestamp` is the ledger timestamp in seconds, `ledger` the ledger sequence number.

## Data Type Inconsistency
While the general preference is for `struct` payloads (like `register`), some high-frequency events like `buy` and `sell` use `tuples` for gas efficiency. Indexers should check the `contracttype` encoding to distinguish between map-based structs and array-based tuples.


