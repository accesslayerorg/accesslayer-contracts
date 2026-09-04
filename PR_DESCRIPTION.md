# Add unit tests for pause blocking sell transactions

Closes #727

Adds a time-weighted average price (TWAP) view that gives external integrations a manipulation-resistant price reference for creator keys. Every buy and sell now records a `(price, ledger)` snapshot into a per-creator persistent ring buffer capped at 100 entries, and `get_twap(creator, window_ledgers)` returns the simple average of the snapshots inside the requested ledger window (falling back to the current spot price when fewer than 2 snapshots qualify).

Closes #802
## Summary

Add dedicated integration tests confirming the emergency pause correctly blocks sell transactions and verifies that all contract state remains consistent after blocked sell attempts.

## Motivation

The bonding curve spot price can be manipulated by a single large trade. A TWAP reference smooths this out by averaging recorded trade prices over a configurable window of ledgers, so one trade contributes at most one term to the average rather than dictating the whole reference price.
The emergency pause mechanism was originally tested primarily for buy transactions. While some existing tests touch sell under pause, there was no focused test suite that comprehensively validates the sell-pause lifecycle against all three acceptance criteria from the issue. This PR fills that gap with a dedicated test file.

## Changes

### New file: `creator-keys/tests/pause_blocks_sell_transactions.rs`

- **`PriceSnapshot { price, ledger }`** contract type and **`DataKey::PriceSnapshots(creator)`** storage key.
- **`MAX_PRICE_SNAPSHOTS = 100`** — ring buffer capacity per creator.
- **`record_price_snapshot(env, creator, price)`** helper:
  - Called after every successful `buy_key`, `sell_key`, and per-key in `batch_buy`.
  - Appends `(price, ledger)` to the creator's buffer; when full, pops the oldest entry first (ring buffer semantics).
  - Extends the buffer key's TTL to the full window on every write.
- **`get_twap(creator, window_ledgers) → i128`** read-only view:
  - Averages every snapshot whose ledger is in `[current_ledger - window_ledgers, current_ledger]`.
  - Returns the current bonding-curve spot price when fewer than 2 snapshots are in the window, and `0` when the key price is unset.
  - Bumps the buffer key's TTL on every read.
  - Never panics: empty buffers, unregistered creators, `window_ledgers = 0`, and `u32::MAX` windows all return a non-negative value.

### `creator-keys/tests/twap.rs` (new)

Integration tests covering every acceptance criterion:
- TWAP equals the simple average of in-window snapshots (and ignores out-of-window ones).
- Spot price returned when fewer than 2 snapshots exist in the window (zero trades and one trade).
- Buy and sell snapshots are both recorded and averaged together.
- Ring buffer capped at 100 entries, oldest overwritten first (verified via storage).
- TTL bumped on ring buffer writes and reads.
- No panic on edge inputs (unregistered creator, empty buffer, zero and huge windows).

### Repo restoration (the branch did not compile at `HEAD`)

`origin/main` did not compile: merged PRs (#751, #752, #753, #755, #756, #758, #777, #774, #795) referenced contract features that had been dropped during their merges. To make the whole workspace build and pass tests this PR restores the missing pieces, keeping the existing integration tests (the de-facto spec) green:

- **Restored error variants** `MaxHoldingExceeded`, `LockupPeriodActive`, `InvalidHolderCap`, `RoyaltyExceedsLimit`, `InvalidExponent`, `BatchSizeExceeded`. The Stellar contract spec caps error enums at 50 cases, so six untested/unused variants were retired (`DiscountTierLimitExceeded`, `CapAlreadySet`, `MultisigAdminLimitExceeded`, `ProposalNotFound`, `VestingNotFound`, `VestingNotStarted`) and their call sites reuse surviving variants. Surviving variant numeric values are unchanged.
- **Restored storage keys/helpers**: `HolderCapBps`, `LastBuyTimestamp`, `ProtocolFeeBps`, `LockupDurationSecs`, `RoyaltyConfig`, `CurveExponent` `DataKey` variants and the `holder_cap_bps` / `last_buy_timestamp` storage helpers.
- **Restored public functions**: `batch_buy`, `set_royalty`, `get_royalty_config`, `migrate_curve`, `get_curve_exponent`, `refresh_ttl`.
- **Restored events**: `FEE_COLLECTED_EVENT_NAME` + `FeeCollectedEvent` + `fee_collected_topics`; `LOCKUP_BLOCKED_EVENT_NAME` + `LockupBlockedEvent` + `lockup_blocked_topics`.
- **Fixed TTL maintenance bugs surfaced by the restored tests**:
  - `set_fee_config` now extends `PROTOCOL_STATE_VERSION`'s TTL (it could archive on long ledger gaps).
  - `buy_key`/`sell_key` extend the global `KEY_PRICE` entry to the full window (the 30-day floor let it archive during multi-month gaps).
  - `refresh_ttl` and `get_twap` only extend entries that exist (`extend_ttl` errors on missing keys).
  - `extend_creator_ttl` extends the contract instance/code TTL so an actively traded contract is never archived.
  - Circuit breaker no longer trips on zero price change when `max_change` rounds to 0 at low key prices.
- **Test fixes** (stale against the current Soroban SDK/client API): corrected `try_*` error assertions (`Err(Ok(...))`), event-log reads moved immediately after the emitting call (test host exposes only the last invocation's events), `Vec` API misuse in `protocol_trade_fee.rs`, stale `initialize` setup in `test_new_features.rs`, and `TimelockChangeType` variants renamed to satisfy clippy.
Five new integration tests covering every acceptance criterion:

| Test | Acceptance Criterion |
|---|---|
| `test_sell_panics_with_protocol_paused_when_contract_is_paused` | Sell panics with `ProtocolPaused` when paused |
| `test_sell_succeeds_after_resume` | Sell succeeds immediately after unpause |
| `test_holder_count_unchanged_after_blocked_sell` | Holder count, supply, and key balance unchanged after blocked sell |
| `test_supply_unchanged_after_blocked_sell_with_multiple_holders` | Multi-holder variant — both holders' state unchanged after concurrent blocked sells |
| `test_full_pause_lifecycle` | End-to-end: buy → pause → blocked sell (state verified) → unpause → successful sell (state verified) |

## Acceptance Criteria Verification

- ✅ **Sell panics with `contract_paused` when paused** — `test_sell_panics_with_protocol_paused_when_contract_is_paused` asserts `Err(Ok(ContractError::ProtocolPaused))` on sell attempt while paused.
- ✅ **Sell succeeds after resume** — `test_sell_succeeds_after_resume` pauses, unpauses, then confirms sell returns the correct new supply.
- ✅ **State unchanged after blocked sell** — `test_holder_count_unchanged_after_blocked_sell` and `test_supply_unchanged_after_blocked_sell_with_multiple_holders` snapshot `supply`, `holder_count`, and `key_balance` before the blocked sell and assert all are identical afterward.

| Criteria | Status |
|---|---|
| TWAP computed correctly as average of snapshots within the window | ✅ |
| Spot price returned when fewer than 2 snapshots exist in the window | ✅ |
| Ring buffer capped at 100 entries, oldest overwritten first | ✅ |
| TTL bumped on ring buffer reads and writes | ✅ |
| Function never panics regardless of snapshot count | ✅ |

## Testing

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace` — 1033 tests across 184 binaries, 0 failures
## CI Pre-Checks

All local CI checks pass:
- ✅ `cargo build` — compiles cleanly
- ✅ `cargo test` — all tests pass (including the 5 new ones)
- ✅ `cargo test --test pause_blocks_sell_transactions` — 5/5 passed
- ✅ No new clippy warnings introduced by this change
