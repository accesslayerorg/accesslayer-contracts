# Local Soroban Prerequisites

Use this guide to prepare a local machine for building and testing the Access Layer contracts.

## Required Tools

- Rust stable, as selected by [`rust-toolchain.toml`](../rust-toolchain.toml)
- Cargo from the active Rust toolchain
- `rustfmt` and `clippy` Rust components
- `wasm32v1-none` Rust target for contract wasm builds
- Stellar CLI `v22.x`; `v22.0.1` is the matching Protocol 22 CLI listed by Stellar
- A shell that can run the commands in this repository from the repo root

This repository currently pins `soroban-sdk = 22.0.11` in [`Cargo.toml`](../Cargo.toml). If the broader Stellar ecosystem has newer releases, use the repo-pinned SDK for local validation unless a maintainer asks you to upgrade it.

## One-Time Setup

Install Rust from <https://rustup.rs>, then install the required Rust components:

```bash
rustup component add rustfmt clippy
rustup target add wasm32v1-none
```

Install the Stellar CLI by following the official setup guide:

<https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup>

If you need to install a specific CLI version with Cargo:

```bash
cargo install --locked stellar-cli --version 22.0.1
```

## Health Checks

Run these commands from the repository root:

```bash
rustup show active-toolchain
cargo --version
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
stellar --version
stellar contract build --package creator-keys
```

The contract build should produce:

```text
target/wasm32v1-none/release/creator_keys.wasm
```

You can also use the Makefile wrappers:

```bash
make fmt-check
make clippy
make test
```

## Troubleshooting

### `can't find crate for core` or missing `wasm32v1-none`

Install the target for the active toolchain:

```bash
rustup target add wasm32v1-none
```

If you changed Rust versions, reinstall the target because Rust targets are installed per toolchain.

### `cargo-clippy` is missing or not applicable

Reinstall the component for the active toolchain:

```bash
rustup component remove clippy
rustup component add clippy
```

Then confirm the repo is using the expected toolchain:

```bash
rustup show active-toolchain
```

### Windows links with Git's `link.exe`

If `cargo test` fails with `link: extra operand` and `Get-Command link.exe` points to `C:\Program Files\Git\usr\bin\link.exe`, Rust is finding Git's POSIX link tool instead of the MSVC linker.

Fix the shell environment by installing Visual Studio Build Tools with the C++ workload, then open a Developer PowerShell or move the Visual Studio linker ahead of Git's `usr\bin` directory on `PATH`.

### `stellar` is not found

Install the Stellar CLI and restart the shell so `PATH` is refreshed:

```bash
stellar --version
```

If the command exists but deploy or invoke commands fail, confirm you are using the intended network and identity. Testnet deployment details are documented in [`stellar-testnet-deployment.md`](./stellar-testnet-deployment.md).
