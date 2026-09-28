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

## Data Type Inconsistency
While the general preference is for `struct` payloads (like `register`), some high-frequency events like `buy` and `sell` use `tuples` for gas efficiency. Indexers should check the `contracttype` encoding to distinguish between map-based structs and array-based tuples.
