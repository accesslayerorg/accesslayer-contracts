# Kani arithmetic proofs

The Kani harnesses in `kani/src/lib.rs` verify the production arithmetic in
`creator-keys/src/math.rs`. The proof crate includes that source file directly
and has no Soroban dependency.

These are **model-checking bounds, not new contract limits**. They are applied
only with `kani::assume`; no setter, governance, quote, or trade behavior is
changed or restricted by the proofs.

## Assumptions

| Input | Verified range |
|---|---|
| Fee amount / gross price | `0..=u32::MAX` |
| Individual fee rate | `0..=10_000` bps |
| Combined creator + protocol rate | `0..=10_000` bps |
| Standalone curve base price | `1..=1_000_000` |
| Standalone curve slope | `0..=10_000` |
| Standalone curve starting supply | `0..=10_000` |
| Standalone curve batch quantity | `1..=4` |
| Round-trip base price | `1..=64` |
| Round-trip slope | `0..=4` |
| Round-trip starting supply | `0..=8` |
| Round-trip quantity | `1..=2` |
| Checked-power base | `0..=1_000` |
| Checked-power exponent | `0..=5` |

All three presets (`Flat`, `Linear`, and `Quadratic`) are selected
symbolically. Batch proofs unwind the pricing loop for every quantity in the
stated range. The round-trip harness composes pricing and fee arithmetic, so
it uses smaller curve inputs and an unwind limit of 4. Its fee rates still
range over all valid splits.

## Properties

The harnesses prove that, within those assumptions:

- percentage fees equal `floor(amount * bps / 10_000)` and stay between zero
  and the gross amount;
- a complete fee split conserves the gross amount exactly, while a partial
  valid split never allocates more than the gross amount;
- buy and sell quote totals preserve their accounting identities without
  overflow or negative sell proceeds;
- buyback net cost is at most gross cost, which is at most total cost;
- spot prices are positive and nondecreasing for nonnegative slopes;
- batch curve calculations do not overflow and cost at least
  `quantity * initial_spot_price`;
- buying a quantity from supply `S` and selling it at resulting supply `S + Q`
  traverses the same gross curve range, and net sell proceeds never exceed the
  corresponding buy cost; and
- checked exponentiation does not overflow or panic within its stated range.

`broken_fee_split_has_a_counterexample` is an expected-failure harness. It
intentionally gives the creator the full amount and then adds a protocol fee.
Kani must find the resulting over-allocation panic; `#[kani::should_panic]`
turns that expected counterexample into a successful non-vacuity check. Removing
that attribute demonstrates the failing proof and counterexample.

## Running locally

Kani 0.68.0 is pinned in CI:

```bash
cargo install --locked --version 0.68.0 kani-verifier
cargo kani setup
cd kani
cargo kani --output-format terse
```

Kani installation support depends on the host platform. The GitHub workflow
runs on an x86-64 Ubuntu runner, caches the installed verifier, and applies a
job timeout.
