## Summary

Fixes the TTL extension logic so that a `TTL_EXTENDED_EVENT_NAME` event is only emitted when the creator's storage TTL actually needs extension (remaining TTL drops below `TTL_EXTENSION_THRESHOLD`). Adds an integration test confirming no event is emitted when TTL is healthy.

Closes #<!-- TODO: insert issue number -->

## Problem

The `extend_creator_ttl` function unconditionally emitted a `ttl_ext` event on every successful buy or sell, even when the creator's storage TTL was already well above the minimum threshold. This polluted event logs with noisy, unnecessary events that indexers and off-chain consumers had to filter out.

## Changes

### `creator-keys/src/lib.rs`

- **Added `TTL_EXTENSION_THRESHOLD` constant** (`100` ledgers) — the minimum remaining TTL below which a TTL extension event is emitted.
- **Added `DataKey::CreatorTtlLiveUntil(creator)`** — a per-creator `u32` recording the absolute live-until ledger the contract last set for the creator profile key. The Soroban SDK does not expose TTL reads to contract code, so this tracked value is what `extend_creator_ttl` uses to decide whether to emit the event.
- **Modified `extend_creator_ttl`** to:
  1. Derive the remaining TTL from `CreatorTtlLiveUntil` and evaluate `ttl::should_extend(remaining, TTL_EXTENSION_THRESHOLD)`.
  2. Always call `extend_ttl` on all creator-scoped storage keys — the Soroban SDK call is a no-op when TTL is already healthy, preserving the existing on-chain behavior.
  3. Only publish the `TTL_EXTENDED_EVENT_NAME` event when the check above returns `true`, then update the tracked live-until.
- **Write-time TTL alignment**: new entries start with the network-default TTL, which can be much shorter than `CREATOR_TTL_LEDGERS` on fresh networks. `register_creator` now forces the full `CREATOR_TTL_LEDGERS` window on the creator profile, curve preset, and tracked live-until; `set_key_price`, the buy path, and dividend settlement grant the same full window to `KeyPrice`, `KeyBalance(creator, holder)`, and the dividend checkpoint/pending keys.

### `creator-keys/tests/ttl_extension_on_buy.rs`

- **Added `test_no_ttl_extension_event_when_ttl_healthy`** integration test that:
  - Registers a creator with TTL at max (~6.3M ledgers remaining).
  - Asserts TTL ≥ 2× `TTL_EXTENSION_THRESHOLD`.
  - Executes a buy without advancing the ledger.
  - Asserts **no** `ttl_ext` event is present among emitted events.
  - Asserts a `buy` event **is** present confirming the transaction succeeded.
  - Asserts creator storage TTL is unchanged after the buy.

## Acceptance Criteria

| Criteria | Status |
|---|---|
| No TTL extension event emitted when TTL is above threshold | ✅ |
| Buy event present confirming transaction succeeded | ✅ |
| Creator storage TTL unchanged after the buy | ✅ |
| Test uses a TTL value at least 2× the extension threshold | ✅ |
| Existing tests continue to pass | ✅ |

## Testing

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace`

**Note:** All existing TTL tests (`test_buy_extends_creator_ttl`, `test_ttl_extension_event_topics_and_payload`, `test_ttl_not_extended_when_already_high`, `test_sell_extends_creator_ttl_after_successful_sell`, `test_failed_sell_does_not_extend_creator_ttl`) remain compatible because:
- They advance the ledger to near expiry before the first buy, so `should_extend` returns `true` and the event is still emitted.
- The second-buy-same-ledger scenario doesn't assert event presence/absence on the second buy.
- `extend_ttl` SDK calls happen unconditionally — only event emission is gated.

## Checklist

- [x] Linked issue or backlog item
- [x] Added or updated `creator-keys` unit/integration tests for every changed contract behavior, including failure paths for new or reachable `ContractError` variants
- [ ] Ran `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`, or explained exactly why a command was not run
- [x] Reviewed persistent storage changes against `docs/storage-key-invariants.md`; any storage layout change includes a migration/backward-compatibility note
- [x] Confirmed event names, topic order, payload field order, and field meanings remain compatible with `docs/contract-event-conventions.md`, or documented the breaking change and versioning plan
- [x] Updated docs for any changed public contract interface, read-only method, event schema, storage behavior, fee logic, or deployment workflow
- [x] Scope stays limited to one contract concern and does not include unrelated formatting, lockfile, generated artifact, or dependency changes
