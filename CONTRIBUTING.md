# Contributing to Access Layer Contracts

Thanks for contributing to the Soroban contracts behind Access Layer, a Stellar-native creator keys marketplace.

## Before you start

- Read the [README](./README.md) for context.
- Review the scoped backlog in [docs/open-source/issue-backlog.md](./docs/open-source/issue-backlog.md).
- Keep pull requests limited to one contract concern at a time.
- Start a discussion before changing pricing, supply, authorization, or storage-model assumptions.

## Local setup

Follow [docs/local-soroban-prerequisites.md](./docs/local-soroban-prerequisites.md) before running the contract checks for the first time. It covers the required Rust components, Soroban wasm target, Stellar CLI version, setup health checks, and common troubleshooting notes.

## Verification commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Or use the helper targets:

```bash
make fmt-check
make clippy
make test
```

For testnet deployment steps, required CLI setup, and the release checklist used for contract updates, see [docs/stellar-testnet-deployment.md](./docs/stellar-testnet-deployment.md).

## Contract contribution rules

- Document storage and event changes clearly.
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
