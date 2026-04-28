# Quote Math Refactor Guidelines

Use this checklist when changing buy/sell quote math paths.

## Invariants Checklist

1. Preserve deterministic output for identical inputs before and after state transitions.
2. Keep totals bounded: buy quote `total_amount >= price`, sell quote `total_amount <= price`.
3. Preserve fee accounting identity:
   - Buy path: `total_amount = price + creator_fee + protocol_fee`
   - Sell path: `total_amount = price - creator_fee - protocol_fee`
4. Maintain zero-supply boundary behavior:
   - Buy quote at zero supply is valid and deterministic.
   - Sell quote without holder balance remains rejected.
5. Keep rounding behavior stable (`protocol_fee` floor, remainder to creator).

## Test Update Checklist

1. Add or update regression tests that cross supply edges (`0 -> 1 -> 0`).
2. Include at least one zero-supply boundary case.
3. Cover both quote determinism and boundedness assertions.
4. Update snapshot tests when expected outputs intentionally change.

## Implementation Notes

1. Reuse shared helpers (`normalize_quote_amount`, fee helpers) instead of duplicating math.
2. Keep error mapping stable for callers and indexers.
3. Document externally visible quote behavior changes in docs and PR notes.
