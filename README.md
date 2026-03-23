# Access Layer Contracts

This repository contains the on-chain smart contracts for Access Layer on Stellar using Soroban.

These contracts hold the trust-sensitive marketplace rules. The goal is to keep pricing, ownership, and fee logic on-chain while leaving general application features to the server and client.

## Purpose

The contracts layer is responsible for:

- registering creators on-chain
- minting and burning creator keys
- enforcing bonding curve pricing
- handling buy and sell execution
- distributing creator and protocol fees
- exposing ownership and supply state to the app

## Tech

- Rust
- Soroban SDK
- Stellar

## Workspace layout

- [Cargo.toml](./Cargo.toml): Rust workspace configuration
- [creator-keys](./creator-keys): first Soroban contract crate

## Current state

The initial `creator-keys` contract is only a starting point. It currently supports:

- simple creator registration
- a basic purchase action that increments creator supply
- reading stored creator data

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Open source workflow

- Read [CONTRIBUTING.md](./CONTRIBUTING.md) before starting work.
- Browse the maintainer issue inventory in [docs/open-source/issue-backlog.md](./docs/open-source/issue-backlog.md).
- Review [SECURITY.md](./SECURITY.md) before reporting vulnerabilities.
- Use the issue templates in [`.github/ISSUE_TEMPLATE`](./.github/ISSUE_TEMPLATE) for new scoped work.
