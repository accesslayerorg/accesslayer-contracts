# Specification: Access Control List (ACL), Transaction Buy Limits, Key Merges & Inactivity Sunsetting

## 1. Cross-Contract ACL (#948)
- Maintains dynamic instance storage of authorized contract addresses and permission bitmasks.
- Allows admin-governed add and remove operations without modifying contract bytecode.
- Emits `ACLUpdated` event.

## 2. On-Chain Buy Limits per Transaction (#947)
- Creator-configurable purchase cap per single buy / batch entry to prevent single-block whale accumulation.
- Rejects orders exceeding limit with structured overflow check.
- Emits `BuyLimitConfigured` event.

## 3. Creator Key Merges (#949)
- Enables consolidation of two keys into one with a fixed conversion ratio.
- Requires mutual creator and admin authorization.
- Emits `MergeExecuted` event.

## 4. Inactivity Key Sunsetting & Deprecation (#931)
- Tracks `last_trade_ledger` activity across trades.
- Automatically flags keys exceeding inactive ledger threshold with `sunset_pending`.
- Requires admin confirmation to finalize deprecation status.
- Emits `KeySunsetFlagged` and `KeyDeprecated` events.
