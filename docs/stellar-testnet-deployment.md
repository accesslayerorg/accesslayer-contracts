# Stellar Testnet Deployment

This guide documents the minimum path to build, deploy, and smoke-test the Access Layer contracts on Stellar testnet.

It is intentionally lightweight. Use it for contributor validation, review environments, and Wave work before any mainnet discussion.

## Tooling and environment assumptions

Before deploying, make sure the machine running the release checks has:

- Rust stable installed
- `rustfmt` and `clippy` components installed
- the `wasm32v1-none` target installed
- the `stellar` CLI installed and available on `PATH`
- a funded Stellar testnet identity available in the local CLI config

This repository currently assumes:

- Rust workspace tooling from [`rust-toolchain.toml`](../rust-toolchain.toml)
- Soroban SDK `22.0.0` from [`Cargo.toml`](../Cargo.toml)
- a deployable contract crate at [`creator-keys`](../creator-keys)

If your local `stellar` CLI or RPC environment differs, verify the command flags with `stellar --help` before deploying.

## One-time local setup

Install the Rust target used by Soroban contracts:

```bash
rustup target add wasm32v1-none
```

Create a local testnet identity and fund it:

```bash
stellar keys generate accesslayer-testnet --network testnet --fund
```

If you already have an identity, funding it again is usually enough:

```bash
stellar keys fund accesslayer-testnet --network testnet
```

## Build and verify before deployment

Run the normal repository checks first:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Build the contract wasm artifact from the repo root:

```bash
stellar contract build --package creator-keys
```

Expected artifact:

```text
target/wasm32v1-none/release/creator_keys.wasm
```

## Deploy to Stellar testnet

Deploy the built wasm and save a local alias for follow-up calls:

```bash
stellar contract deploy \
  --network testnet \
  --source accesslayer-testnet \
  --alias creator-keys-testnet \
  --wasm target/wasm32v1-none/release/creator_keys.wasm
```

Record the returned contract ID in the pull request or release notes for reviewers.

## Post-deploy smoke test

After deployment, confirm the contract can accept a state-changing call and a read call.

Register a creator profile:

```bash
CREATOR_ADDRESS="$(stellar keys public-key accesslayer-testnet)"

stellar contract invoke \
  --network testnet \
  --source accesslayer-testnet \
  --id creator-keys-testnet \
  --send=yes \
  -- register_creator \
  --creator "$CREATOR_ADDRESS" \
  --handle "wave-test"
```

Read the stored creator profile back:

```bash
stellar contract invoke \
  --network testnet \
  --source accesslayer-testnet \
  --id creator-keys-testnet \
  --send=no \
  -- get_creator \
  --creator "$CREATOR_ADDRESS"
```

If the second command returns a populated profile with `supply: 0`, the deployment is working at the minimum expected level for this repository.

## Lightweight release checklist

Use this checklist for any contract update that is intended for shared review or a testnet rollout.

- Confirm the branch includes the intended contract changes only.
- Run `cargo fmt --all -- --check`.
- Run `cargo clippy --workspace --all-targets -- -D warnings`.
- Run `cargo test --workspace`.
- Rebuild the wasm with `stellar contract build --package creator-keys`.
- Deploy the new wasm to Stellar testnet from a funded identity.
- Run the register and read smoke tests against the deployed contract.
- Capture the deployed contract ID and relevant CLI output in the PR description, issue, or release notes.
- Call out any storage layout, event schema, or authorization changes explicitly for reviewers.
