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

Or use the helper targets:

```bash
make fmt-check
make clippy
make test
```

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
