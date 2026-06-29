# Contract function contribution guide

This guide captures the conventions used by the `creator-keys` contract so contributors can add a new entrypoint without fighting the existing layout or CI expectations.

## 1. Place the function in the right module

Keep new public entrypoints in the main contract implementation in [creator-keys/src/lib.rs](../creator-keys/src/lib.rs) and group them by responsibility:

- Trade entrypoints such as `buy_key` and `sell_key` belong with the trading flow near the other market operations.
- Config entrypoints such as fee or price updates belong with the configuration helpers in the same contract implementation.
- Read-only view methods belong near the other quote and profile read methods.
- Admin-only mutators should stay near the other admin-facing entrypoints so the access model is easy to audit.

## 2. Follow the authorization pattern

Use the same auth pattern as the surrounding entrypoints:

- `admin.require_auth()` for protocol-admin-only entrypoints.
- `creator.require_auth()` for creator-owned actions.
- `buyer.require_auth()` or `seller.require_auth()` for trade actions that are authorized by the user acting on the market.
- Read-only methods should not call `require_auth()`.

When a function needs to validate the currently authorized address, prefer the same address that the existing entrypoint uses for the relevant role rather than introducing a new authorization model.

## 3. Emit events the same way

New state-changing entrypoints should publish events via `env.events().publish(...)` using the centralized names and helpers in [creator-keys/src/events.rs](../creator-keys/src/events.rs).

For a new event:

1. Add a new event struct to [creator-keys/src/events.rs](../creator-keys/src/events.rs).
2. Define a stable event name constant and any topic helpers there.
3. Publish the event from the contract entrypoint after the state change is persisted.
4. Keep field ordering stable so downstream indexers and tests remain deterministic.

## 4. Add the right tests

Every new contract function should include tests for:

- a happy path,
- the main error case,
- any relevant state change or regression case.

The repo's minimum test structure lives in [docs/minimum-viable-test-structure.md](./minimum-viable-test-structure.md).

## 5. Run the required CI checks before opening a PR

The repository CI workflow in [.github/workflows/ci.yml](../.github/workflows/ci.yml) expects:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run the same checks locally before pushing so the maintainer sees a passing PR.
