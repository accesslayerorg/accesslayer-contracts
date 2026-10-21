# Curve presets and timed updates

This document explains how bonding-curve defaults and per-creator parameters are applied,
including the timelocked path for changing a creator's curve after registration.

---

## What is the curve preset?

The **curve preset** selects the curve shape for a creator's keys. The protocol-wide key
price and slope are defaults; a creator's timelocked curve update can set a per-creator
intercept and slope. The slope determines how the key price scales with supply:

```
price(supply) = base_price + slope * supply
```

When `slope == 0` the curve is flat (constant price regardless of supply). A positive slope
makes each successive key more expensive than the last.

---

## Timelocked parameter updates

Changing the curve parameters reprices outstanding keys, so creators cannot change them
immediately. Instead, the creator proposes a new slope and intercept, which takes effect
only after a 48-hour timelock:

1. `propose_curve_update(key_id, new_slope, new_intercept)` is creator-authorized and emits
   `CurveUpdateProposed`, including both proposed values and the execution time.
2. The creator can cancel a pending proposal with `cancel_curve_update(proposal_id)`.
3. Anyone can execute a pending proposal with `execute_curve_update(proposal_id)` after
   the 48-hour delay. Execution emits `CurveUpdateExecuted` with the old and new values.

---

## Read path

To inspect the protocol-wide default slope:

```
get_curve_slope() -> i128
```

The return value is the default slope stored at `constants::storage::CURVE_SLOPE`. Curve
pricing uses the creator-specific slope and intercept after an update has executed, and
falls back to the protocol defaults otherwise.

---

## Related documents

- [bonding-curve-formula.md](./bonding-curve-formula.md) — full price formula and fee pipeline
- [fee-assumptions.md](./fee-assumptions.md) — fee split math and rounding
- [read-only-methods.md](./read-only-methods.md) — all read-only contract entry points
- [deterministic-quote-tests.md](./deterministic-quote-tests.md) — how quote regression tests are structured
