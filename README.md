# Access Layer Contracts

This repository contains the on-chain smart contracts for Access Layer on Stellar using Soroban.

These contracts hold the trust-sensitive marketplace rules. The goal is to keep pricing, ownership, and fee logic on-chain while leaving general application features to the server and client.

## Purpose

The contracts layer is responsible for:

* registering creators on-chain
* minting and burning creator keys
* enforcing bonding curve pricing
* handling buy and sell execution
* distributing creator and protocol fees
* exposing ownership and supply state to the app

## Tech

* Rust
* Soroban SDK
* Stellar

## Workspace layout

* [Cargo.toml](./Cargo.toml): Rust workspace configuration
* [creator-keys](./creator-keys): first Soroban contract crate

## Current state

The initial `creator-keys` contract is only a starting point. It currently supports:

* simple creator registration
* a basic purchase action that increments creator supply
* reading stored creator data

## Verification

For first-time local setup, install the required Soroban tooling with [docs/local-soroban-prerequisites.md](./docs/local-soroban-prerequisites.md).

### Prerequisites

Before running the full contract test suite locally, ensure you have:

* Rust (stable toolchain)
* Cargo
* rustfmt component
* Clippy component
* Soroban tooling as described in `docs/local-soroban-prerequisites.md`

Install the required Rust components:

```bash
rustup component add rustfmt clippy
```

### Format Check

Verify that all code is properly formatted:

```bash
cargo fmt --all -- --check
```

Or using the Makefile:

```bash
make fmt-check
```

### Linting

Run Clippy with warnings treated as errors:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Or:

```bash
make clippy
```

### Unit and Integration Tests

Run the full workspace test suite:

```bash
cargo test --workspace
```

Or:

```bash
make test
```

### Full Local Verification

Run the same checks executed by CI:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### Environment Configuration

No additional environment variables are currently required to run the contract verification suite locally. If future contract tests require local configuration, those requirements will be documented alongside the affected tests.

## Testnet deployment

For contributor test deployments and release checks, use the guide in [docs/stellar-testnet-deployment.md](./docs/stellar-testnet-deployment.md). Store and name release wasm files using [docs/deploy-artifacts.md](./docs/deploy-artifacts.md). For client and server integration expectations (read methods, events, version bumps), see [docs/contract-consumer-boundaries.md](./docs/contract-consumer-boundaries.md). For detailed return value semantics of every read-only entrypoint (units, precision, edge-case outputs), see [docs/read-only-methods.md](./docs/read-only-methods.md). For contract event naming conventions and topic format, see [docs/contract-event-conventions.md](./docs/contract-event-conventions.md). For quote-related storage key behavior and invariants, see [docs/quote-storage-keys.md](./docs/quote-storage-keys.md). For caller restrictions and the authorization model for each public function, see [docs/authorization-model.md](./docs/authorization-model.md). For the bonding curve formula, fee application, and worked price examples, see [docs/bonding-curve-formula.md](./docs/bonding-curve-formula.md).

## Open source workflow

* Read [CONTRIBUTING.md](./CONTRIBUTING.md) before starting work.
* Browse the maintainer issue inventory in [docs/open-source/issue-backlog.md](./docs/open-source/issue-backlog.md).
* Review [SECURITY.md](./SECURITY.md) before reporting vulnerabilities.
* Use the issue templates in [`.github/ISSUE_TEMPLATE`](./.github/ISSUE_TEMPLATE) for new scoped work.
