# Mutation Testing Baseline (`cargo-mutants`)

This document records the baseline mutation testing inventory and results for security-critical modules in `accesslayer-contracts`, lists all excluded mutants with their rationale, and explains how contributors should triage and act on surviving mutants.

## Target Scope

Configured in [`.cargo/mutants.toml`](../../.cargo/mutants.toml) and executed via [`scripts/mutants.sh`](../../scripts/mutants.sh):

| Category | Module File | Package | Mutants Generated |
| --- | --- | --- | ---: |
| **Fee** | `fee-discount-contract/src/lib.rs` | `fee-discount-contract` | 29 |
| **Fee** | `creator-keys/src/dynamic_fees.rs` | `creator-keys` | 64 |
| **Curve** | `creator-keys/src/curve_subscriptions_swaps.rs` | `creator-keys` | 51 |
| **Reserve** | `holder-insurance-pool/src/lib.rs` | `holder-insurance-pool` | 44 |
| **Reserve** | `creator-keys/src/multi_asset_payment.rs` | `creator-keys` | 89 |
| **Vesting** | `creator-keys/src/vesting.rs` | `creator-keys` | 16 |
| **Authorization** | `common/src/guard.rs` | `common` | 3 (1 excluded) |
| **Authorization** | `creator-keys/src/acl_roles.rs` | `creator-keys` | 9 |
| **Authorization** | `creator-keys/src/badge_access_control.rs` | `creator-keys` | 18 |
| **Authorization** | `multisig-admin/src/lib.rs` | `multisig-admin` | 56 |
| **Total** | **10 target files** | **5 crates** | **379 active (380 raw)** |

## Baseline vs. Post-Hardening Results

### Fee & Reserve Modules (Hardened — 0 Unexplained Surviving Mutants)

Prior to this issue, `fee-discount-contract` and `holder-insurance-pool` had **0 unit tests**, leaving **100% of viable mutants surviving** in both crates:

| Module | Baseline (Before) | After Hardening | Notes |
| --- | --- | --- | --- |
| `fee-discount-contract/src/lib.rs` | 0 caught, **27 missed**, 2 unviable | **27 caught, 0 missed**, 2 unviable | Added unit tests covering initialization, admin auth, tier validation bounds, volume accumulation/events, `get_discount` threshold re-checks, and literal 30-day (`2_592_000`s) window reset boundaries. |
| `holder-insurance-pool/src/lib.rs` | 0 caught, **44 missed**, 0 unviable | **44 caught, 0 missed**, 0 unviable | Added unit tests covering initialization, positive funding, cover cap ratios, premium calculation (`premium_rate_bps / 10_000`), trigger auth, claim expiry/deduction/double-claim guards, and `RESERVE_BPS` surplus withdrawal boundaries. |

#### Notable Baseline Surviving Mutants Killed

1. **`fee-discount-contract/src/lib.rs:4` (`WINDOW = 30 * 24 * 60 * 60`)** — 6 arithmetic mutations (`*` -> `+` / `/`). Initially survived when tests referenced `WINDOW` directly; killed by asserting against the literal `2_592_000` seconds at `start + 2_591_999` (must accumulate) and `start + 2_592_000` (must reset).
2. **`fee-discount-contract/src/lib.rs:67` (`t.threshold <= last || t.discount_bps > 10_000`)** — 5 boundary/boolean mutations. Killed by testing zero threshold, duplicate thresholds, `discount_bps == 10_001` (rejected), and `discount_bps == 10_000` (accepted).
3. **`fee-discount-contract/src/lib.rs:144` (`v.amount >= t.threshold && (i + 1) as u32 == v.tier`)** — 5 condition/index mutations. Killed by raising tier thresholds after recording volume and verifying `get_discount` returns `discount_bps == 0` when stored volume falls below the updated threshold.
4. **`holder-insurance-pool/src/lib.rs:90,97` (cover cap ratio & premium bps division)** — 9 comparison/arithmetic mutations. Killed by testing exact 50% cap (`3_000 + 2_000 == 5_000` allowed, `+1` rejected with `CapExceeded`) and inspecting stored `Cover.premium` (`300` at `1_000` bps on `3_000`).
5. **`holder-insurance-pool/src/lib.rs:205,208` (`withdraw_premium_surplus` reserve ratio enforcement)** — 13 comparison/boolean/arithmetic mutations. Killed by testing negative amount (rejected), zero amount (allowed), `pool - amount == required_reserve - 1` (rejected with `ReserveViolation`), exact boundary `pool - amount == required_reserve` (allowed), and verifying remaining `Key::Pool` balance.

### Other Target Modules (Tracked via Weekly Full Run)

The remaining target modules (`creator-keys/src/dynamic_fees.rs`, `creator-keys/src/curve_subscriptions_swaps.rs`, `creator-keys/src/multi_asset_payment.rs`, `creator-keys/src/vesting.rs`, `creator-keys/src/acl_roles.rs`, `creator-keys/src/badge_access_control.rs`, `common/src/guard.rs`, `multisig-admin/src/lib.rs`) are tracked continuously by:

- **PR CI job** (`.github/workflows/mutants.yml` -> `mutants-pr`): runs `cargo-mutants --in-diff` on changed files in every pull request and reports surviving mutants in the job summary and uploaded artifact.
- **Weekly scheduled run** (`.github/workflows/mutants.yml` -> `mutants-weekly`): runs every Monday at 03:00 UTC across the full target set and uploads `mutants-weekly-report` (`outcomes.json`, `caught.txt`, `missed.txt`, `unviable.txt`, `timeout.txt`) as a 30-day artifact.

## Excluded Mutants

Every exclusion in [`.cargo/mutants.toml`](../../.cargo/mutants.toml) must be listed below with an explicit reason:

| Exclusion Pattern | Scope | Reason |
| --- | --- | --- |
| `**/test.rs`, `**/test_*.rs`, `**/tests/**` | `exclude_globs` | Test suites and test harnesses should not be mutated; only production contract logic is under test. |
| `replace ReentrancyGuard::exit with \(\)` | `exclude_re` (`common/src/guard.rs:34`) | **Equivalent mutant.** `ReentrancyGuard::exit(self)` consumes `self` by value and calls `drop(self)`. Replacing its body with `()` still drops `self` immediately upon returning from `exit(self)`, producing identical runtime and storage behavior. |

## Running Mutation Tests Locally

Install `cargo-mutants`:

```bash
cargo install --locked cargo-mutants
```

Run against a single module or the full target set:

```bash
# Single module
./scripts/mutants.sh fee
./scripts/mutants.sh curve
./scripts/mutants.sh reserve
./scripts/mutants.sh vesting
./scripts/mutants.sh auth

# Single file
./scripts/mutants.sh fee-discount-contract/src/lib.rs

# Full target set
./scripts/mutants.sh all

# List mutants without running tests
./scripts/mutants.sh fee --list
```

The script prints a summary of **Caught**, **Missed**, **Timeout**, and **Unviable** mutants and lists every surviving mutant from `mutants.out/missed.txt`.

## How to Act on a Surviving Mutant

When `scripts/mutants.sh` or the PR CI job reports a surviving (`MISSED`) mutant, follow this decision workflow:

1. **Inspect the mutant diff.**
   Open the generated diff in `mutants.out/diff/` (or download the `mutants-pr-report` / `mutants-weekly-report` artifact in GitHub Actions) to see the exact source change that went undetected.

2. **Check for a missing test assertion (most common).**
   - Did a test execute the function without asserting its return value, error variant, emitted event, or updated storage state?
   - Did a boundary test miss an off-by-one (`<` vs `<=`, `>` vs `>=`)?
   - Did a test reference a production constant (e.g. `WINDOW` or `BPS_MAX`) instead of an independent expected value, allowing mutations to the constant to go unnoticed?
   - **Action:** Write or strengthen a focused unit test that fails when the mutation is applied, then re-run `./scripts/mutants.sh <file>` to confirm the mutant is now `CAUGHT`.

3. **Check for dead or redundant code.**
   - If no caller can ever observe the mutated branch (e.g. a redundant guard already enforced earlier), the code itself may be dead or overly defensive.
   - **Action:** Simplify or remove the redundant branch if safe, or document the defense-in-depth invariant.

4. **Check for an equivalent mutant.**
   - A mutant is *equivalent* if the mutated program has identical observable behavior for all valid inputs (for example, `ReentrancyGuard::exit(self)` dropping `self` explicitly vs. implicitly on return).
   - **Action:** Add a narrow regex to `exclude_re` in `.cargo/mutants.toml` with an inline comment explaining why it is equivalent, **and** add a row to the [Excluded Mutants](#excluded-mutants) table in this document.

5. **Check for timeouts.**
   - A `TIMEOUT` mutant usually means a loop counter or termination condition was mutated into an infinite loop (which is a caught bug). If a valid test is simply slow, verify locally with `--timeout` and optimize slow test setup where possible.
