# Contributing to Access Layer Contracts

Thanks for contributing to the Soroban contracts behind Access Layer, a Stellar-native creator keys marketplace.

## Before you start

- Read the [README](./README.md) for context.
- Review the scoped backlog in [docs/open-source/issue-backlog.md](./docs/open-source/issue-backlog.md).
- Keep pull requests limited to one contract concern at a time.
- Start a discussion before changing pricing, supply, authorization, or storage-model assumptions.

## Local setup

1. Install the stable Rust toolchain.
2. Make sure `rustfmt` and `clippy` are available.
3. Run the workspace checks from this repo root.

## Verification commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

You can also use the helper targets from the `Makefile` at the repo root (`make fmt-check`, `make clippy`, `make test`).

## Integration test helpers

Shared setup for `creator-keys` integration tests lives in `creator-keys/tests/contract_test_env/`. Import the module with `mod contract_test_env;` and call the small helpers (env with mocked auths, register contract, set key price, set fees, register a test creator) instead of duplicating boilerplate in every file.

For guidance on writing deterministic quote tests, see [docs/deterministic-quote-tests.md](./docs/deterministic-quote-tests.md).

## Documentation for contributors

- **[CI Contract Checks](./docs/ci-contract-checks.md)**: Understanding the CI pipeline, running checks locally, and troubleshooting failures
- **[Storage Key Invariants](./docs/storage-key-invariants.md)**: Storage model, key structure, and invariants that must be maintained across all operations
- **[Deterministic Quote Tests](./docs/deterministic-quote-tests.md)**: Guide for writing tests for quote operations with the fixed price model
- **[Quote Math Refactor Guidelines](./docs/quote-math-refactor-guidelines.md)**: Checklist for preserving quote invariants and required regression coverage during quote-path refactors
- **[Fee Assumptions](./docs/fee-assumptions.md)**: Fee split logic, rounding behavior, and integration points
- **[Read-only Methods](./docs/read-only-methods.md)**: Return value semantics, units, and edge-case behaviour for every `get_*` / `is_*` entrypoint including all bps fields
- **[Error Codes](./docs/error-codes.md)**: Contract error reference with causes and expected caller behavior
- **[Safely Extending Contract Error Codes](./docs/error-extension-guide.md)**: Rules for naming new error variants, preserving discriminants, and updating error tables

For testnet deployment steps, required CLI setup, and the release checklist used for contract updates, see [docs/stellar-testnet-deployment.md](./docs/stellar-testnet-deployment.md). For **wasm artifact** naming, retention, and metadata, see [docs/deploy-artifacts.md](./docs/deploy-artifacts.md). For how **clients and servers** should depend on contract read surfaces and events, see [docs/contract-consumer-boundaries.md](./docs/contract-consumer-boundaries.md).

## Contract contribution rules

- Document storage and event changes clearly. See [docs/storage-evolution.md](./docs/storage-evolution.md) for metadata versioning rules.
- Treat buy, sell, fee, and supply logic as high-sensitivity areas.
- Prefer incremental contract changes over sweeping redesigns.
- Add or update tests for every behavior change.
- Keep names and comments specific to Access Layer and Stellar, not generic template wording.

## Good first issue guidance

Good first issues in this repo should:

- avoid protocol-level economic changes
- have narrow storage or event scope
- include explicit acceptance criteria
- be testable in isolation

## Questions

If a change touches client UX or backend indexing, split that work into the appropriate repository instead of expanding contract scope.
