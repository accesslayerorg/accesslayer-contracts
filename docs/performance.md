# Performance and Budget Profiles

This repository enforces explicit WASM size and per-call resource budgets so contract growth fails CI before it becomes a deployment or transaction problem. [`budgets.toml`](../budgets.toml) is the single source of truth for every limit and for the state sizes used by the benchmarks.

## Budgeted State

| Scenario | State from `budgets.toml` |
|----------|---------------------------|
| `buy` | 100 holders, 4 keys per holder |
| `sell` | 100 holders, 4 keys per holder |
| `claim` | 100 holders, 4 keys per holder, 5 dividend distributions |
| `governance` | 100 holders, 4 keys per holder, 20 existing actions |

The benchmarks live in [`resource_budget_benchmarks.rs`](../creator-keys/tests/resource_budget_benchmarks.rs) and read these values from `budgets.toml` at compile time.

## Measuring Locally

Build the release WASM artifacts for every budgeted contract and compare their sizes:

```bash
./scripts/check-wasm-size.sh
```

Run the resource benchmarks:

```bash
cargo test -p creator-keys --test resource_budget_benchmarks -- --nocapture
```

Each benchmark prints a line in this form:

```text
RESOURCE_USAGE<tab>entrypoint<tab>cpu_insns<tab>mem_bytes<tab>cpu_budget<tab>mem_budget
```

The benchmarks reset the host budget before the measured invocation and read `env.cost_estimate().budget().cpu_instruction_cost()` and `memory_bytes_cost()` immediately after it. Setup calls are intentionally outside the measured window so the numbers describe the entrypoint, not fixture creation.

Native cost estimates undercount WASM execution and transaction-size costs. Use RPC simulation for exact preflight numbers before a release.

## Reading the CI Report

The `budgets` job in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) prints one row per contract and per resource dimension:

| Column | Meaning |
|--------|---------|
| `Current` | Size or resource use measured in this pull request |
| `Budget` | Limit from `budgets.toml` |
| `Δ usage vs base` | Measured change relative to the pull request base commit |
| `Δ budget vs base` | Budget change relative to the pull request base commit |
| `Status` | `OK`, `OVER`, `MISSING`, or `STALE BUDGET` |

The job measures the base commit in a separate worktree when that commit already contains the budget tooling. The first pull request introducing the tooling reports usage changes as `-` because the base commit has no matching benchmarks.

The job fails when a measured value exceeds its budget, a benchmark report is missing, the benchmark used a stale budget value, or a budget increased without `budgets.toml` appearing in the pull request diff.

## Reducing WASM Size

1. Build with the workspace release profile: `cargo build --release --target wasm32v1-none -p <contract>`.
2. Confirm the profile still uses `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = "symbols"`, and `panic = "abort"` in [`Cargo.toml`](../Cargo.toml).
3. Compare the current artifact with the previous one using `ls -l target/wasm32v1-none/release/<contract>.wasm`.
4. Remove unused dependencies and feature flags from the contract crate before adding new abstractions.
5. Split rarely used entrypoints into a separate contract instead of growing a hot contract.
6. Avoid large embedded constants, format strings, and broad generic error types in contract code.
7. Re-run `./scripts/check-wasm-size.sh` after each change to isolate which edit reduced size.

For storage access patterns that drive CPU and memory cost, see [docs/storage-benchmarks.md](./storage-benchmarks.md).

## Reducing Resource Use

1. Run the benchmark with `--nocapture` and record the `RESOURCE_USAGE` line for the affected entrypoint.
2. Inspect the hot path with `env.cost_estimate().budget().print()` around a single invocation when the marker line shows a regression.
3. Reduce repeated persistent reads by loading shared configuration once per invocation.
4. Keep hot-path structs small so writes do not rewrite large records.
5. Replace per-holder scans with index keys or incremental accumulators.
6. Re-run the benchmark against the documented state sizes to confirm the improvement.

## Raising a Budget

Raise a limit only when the new value is justified by simulation or profiling. Edit `budgets.toml` in the same pull request, update the state sizes if the benchmark scenario changed, and explain the reason in the pull request description. CI rejects budget increases that do not include that file.
