# Curve preset immutability

This document explains why the bonding curve configuration (slope) for a creator's keys
is treated as immutable after registration, what "curve preset" means in this codebase,
and how to read the current value.

---

## What is the curve preset?

The **curve preset** is the bonding curve slope configured via `set_curve_slope` before a
creator registers. The slope determines how the key price scales with supply:

```
price(supply) = base_price + slope * supply
```

When `slope == 0` the curve is flat (constant price regardless of supply). A positive slope
makes each successive key more expensive than the last — a classic bonding curve shape.

The slope value in effect at registration time is the implicit preset for that creator.
Buyers who purchase that creator's keys are pricing their positions against the curve that
was active when the creator first appeared on-chain.

---

## Why the preset is immutable after registration

Once a creator has registered and buyers have purchased keys, changing the curve slope would
retroactively reprice all outstanding positions:

- A buyer who paid `base_price + slope_old * 5` for the sixth key made that decision knowing
  the exit price follows the same curve. Swapping the slope mid-life invalidates that
  expectation.
- Holders cannot hedge against an unknown future slope change. A steeper slope raises the
  exit cost for late sellers; a flatter slope reduces the return for early buyers who paid
  a premium. Either direction harms some subset of holders.
- Price continuity is a social contract. Breaking it mid-stream would destroy trust in the
  key market for that creator.

To prevent these harms, **no `set_curve_slope` call can retroactively alter the price
history of keys that were already bought at a different slope**. The slope is effectively
locked for each cohort of buyers from the moment they transact.

---

## Deploying with a different curve

A creator who wants a different curve shape must complete a new creator registration:

1. Deploy a new contract instance (or re-register on a fork/new protocol instance).
2. Set the desired slope via `set_curve_slope` *before* registration.
3. Register the creator address under the new slope.

Existing holders of the old creator's keys are unaffected — their positions remain priced
on the original curve until they choose to sell.

---

## Read path and absence of an update path

To inspect the current slope:

```
get_curve_slope() -> i128
```

This is the only read path. The return value is the global slope stored at
`constants::storage::CURVE_SLOPE`.

**There is no `set_curve_slope_for_creator` or equivalent per-creator update function.**
This absence is intentional. Providing an update path would introduce the holder-harm
scenarios described above. Any contributor who adds such a function must first address
the migration and consent mechanism for existing holders.

---

## Related documents

- [bonding-curve-formula.md](./bonding-curve-formula.md) — full price formula and fee pipeline
- [fee-assumptions.md](./fee-assumptions.md) — fee split math and rounding
- [read-only-methods.md](./read-only-methods.md) — all read-only contract entry points
- [deterministic-quote-tests.md](./deterministic-quote-tests.md) — how quote regression tests are structured
