# Add unit tests for pause blocking sell transactions

Closes #727

## Summary

Add dedicated integration tests confirming the emergency pause correctly blocks sell transactions and verifies that all contract state remains consistent after blocked sell attempts.

## Motivation

The emergency pause mechanism was originally tested primarily for buy transactions. While some existing tests touch sell under pause, there was no focused test suite that comprehensively validates the sell-pause lifecycle against all three acceptance criteria from the issue. This PR fills that gap with a dedicated test file.

## Changes

### New file: `creator-keys/tests/pause_blocks_sell_transactions.rs`

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

## CI Pre-Checks

All local CI checks pass:
- ✅ `cargo build` — compiles cleanly
- ✅ `cargo test` — all tests pass (including the 5 new ones)
- ✅ `cargo test --test pause_blocks_sell_transactions` — 5/5 passed
- ✅ No new clippy warnings introduced by this change
