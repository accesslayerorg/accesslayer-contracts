# Integration Test Guide

This guide walks you through setting up a local Soroban environment, building the contract, deploying to a local testnet, and running the integration test suite.

For prerequisites and health checks, see [`local-soroban-prerequisites.md`](./local-soroban-prerequisites.md). For test categories and example structures, see [`minimum-viable-test-structure.md`](./minimum-viable-test-structure.md).

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | stable (per `rust-toolchain.toml`) | [rustup.rs](https://rustup.rs) |
| `rustfmt`, `clippy` | bundled with stable | `rustup component add rustfmt clippy` |
| `wasm32v1-none` target | — | `rustup target add wasm32v1-none` |
| Stellar CLI | v22.x (`v22.0.1` recommended) | `cargo install --locked stellar-cli --version 22.0.1` |

Verify your setup:

```bash
rustup show active-toolchain
stellar --version
```

## Build the contract WASM

From the repository root:

```bash
stellar contract build --package creator-keys
```

Expected output artifact:

```text
target/wasm32v1-none/release/creator_keys.wasm
```

Verify the binary is valid:

```bash
ls -lh target/wasm32v1-none/release/creator_keys.wasm
```

## Set up a local testnet

Start a local Soroban testnet instance. This runs a lightweight stellar node in the background:

```bash
stellar local start
```

The local node listens on `http://localhost:8000` by default. Leave this terminal running.

In a **second terminal**, create an identity and fund it on the local network:

```bash
stellar keys generate local-admin --network local
```

## Deploy the contract

Deploy the built WASM to the local testnet:

```bash
stellar contract deploy \
  --network local \
  --source local-admin \
  --alias creator-keys-local \
  --wasm target/wasm32v1-none/release/creator_keys.wasm
```

The CLI returns a contract address and saves it under the alias `creator-keys-local`. Record it for manual invocations:

```bash
stellar contract address --alias creator-keys-local
```

## Run the full integration test suite

The integration tests use the Soroban `Env::default()` test harness and do **not** require a running local testnet. They deploy an in-memory contract instance per test. Run from the repository root:

```bash
cargo test --workspace
```

To run a single test file:

```bash
cargo test --package creator-keys --test buy_price_monotonicity_bonding_curve
```

To run tests matching a pattern:

```bash
cargo test --package creator-keys -- buy_price
```

### Interpreting output

- `test result: ok. N passed; 0 failed` — all tests pass.
- A failure shows the test name, the assertion message, and a backtrace. The assertion message includes the expected and actual values.
- `warning: ... dead_code` warnings are expected for shared helpers in `contract_test_env` — they are compiled once per test binary but not all are used by every binary.

## Run the linter and type checker

Before opening a PR, run the full CI sequence:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Or use the Makefile shortcut:

```bash
make ci
```

## Resetting the local testnet

To wipe all local contract state and start fresh:

```bash
stellar local stop
rm -rf .soroban/local
stellar local start
```

After restarting, re-deploy the contract (the previous contract address is no longer valid).

## Writing a new integration test

1. Create a new file in `creator-keys/tests/<feature>.rs`.
2. Add `mod contract_test_env;` at the top.
3. Use shared helpers from `contract_test_env` (e.g., `register_creator_keys`, `register_test_creator`, `set_pricing_and_fees`, `test_env_with_auths`).
4. Follow the naming convention: `test_<entrypoint>_<expected_outcome>_<condition>`.
5. See [`minimum-viable-test-structure.md`](./minimum-viable-test-structure.md) for full examples.

### Example skeleton

```rust
mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_my_new_feature_behaves_as_expected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, 1000, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");

    // ... your test logic here ...
}
```

## Cross-references

- [`local-soroban-prerequisites.md`](./local-soroban-prerequisites.md) — toolchain versions and health checks
- [`minimum-viable-test-structure.md`](./minimum-viable-test-structure.md) — test categories and example structures
- [`stellar-testnet-deployment.md`](./stellar-testnet-deployment.md) — testnet deployment and smoke test
- [`deterministic-quote-tests.md`](./deterministic-quote-tests.md) — quote-specific test patterns
- [`error-codes.md`](./error-codes.md) — error variants to cover in failure tests
