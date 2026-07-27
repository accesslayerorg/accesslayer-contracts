# [FEAT] Make TTL Extension a No-Op When Creator TTL Is Healthy

## Summary

The `extend_creator_ttl` helper now skips all storage writes and does not emit the `TTL_EXTENDED_EVENT_NAME` event when the creator's remaining TTL is still at or above `CREATOR_TTL_LEDGERS`. Previously it would unconditionally call `extend_ttl` on all creator-scoped keys and always emit the event, even when the Soroban SDK's `extend_ttl` was a no-op internally.

A new integration test confirms the no-op behavior: no TTL extension event, buy event present, and TTL unchanged.

## Changes
    
### `creator-keys/src/lib.rs`

- **`extend_creator_ttl`**: Added an early-return guard that reads the creator key's remaining TTL via `get_ttl` and uses `ttl::should_extend(remaining, CREATOR_TTL_LEDGERS)` to decide whether an actual extension is needed. When the remaining TTL is at or above `CREATOR_TTL_LEDGERS` (i.e., the key is still fresh), the function returns immediately — no `extend_ttl` calls and no event emission.

### `creator-keys/tests/ttl_extension_noop_when_above_threshold.rs` (new)

Integration test `test_no_ttl_extension_event_when_ttl_healthy` that:

1. Registers a creator with a fresh TTL equal to `CREATOR_TTL_LEDGERS`
2. Executes a buy immediately
3. **Asserts no `TTL_EXTENDED_EVENT_NAME` event** is present among emitted events
4. **Asserts the buy event IS present** (transaction succeeded)
5. **Asserts creator storage TTL is unchanged** after the buy

## Acceptance Criteria

| Criterion | Status |
|-----------|--------|
| No TTL extension event emitted when TTL is above threshold | ✅ Verified by test |
| Buy event present confirming the transaction succeeded | ✅ Verified by test |
| Creator storage TTL unchanged after the buy | ✅ Verified by test |
| Test uses a TTL value at least as large as the extension threshold | ✅ Uses `CREATOR_TTL_LEDGERS` (6,311,520 ledgers, ~2 years) |
