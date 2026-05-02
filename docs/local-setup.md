# Local Setup Guide

Follow this guide to set up your local environment for Access Layer contract development.

## Prerequisites

Access Layer contracts are built for **Stellar Soroban** using Rust.

### 1. Install Rust
Install the stable Rust toolchain via [rustup.rs](https://rustup.rs/):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Add WASM Target
Soroban contracts compile to WebAssembly. Add the `wasm32-unknown-unknown` target:
```bash
rustup target add wasm32-unknown-unknown
```

### 3. Install Stellar CLI
The `stellar-cli` (formerly `soroban-cli`) is required for building and deploying:
```bash
cargo install --locked stellar-cli --features opt
```

## Setup Verification

Run these commands to ensure your environment is ready:

| Command | Expected Output |
|---------|-----------------|
| `cargo --version` | `cargo 1.70.0` or higher |
| `rustup target list --installed` | Should include `wasm32-unknown-unknown` |
| `stellar --version` | `stellar 21.0.0` or higher |

### Quick Health Check
Run the workspace tests from the repository root:
```bash
cargo test --workspace
```

## Troubleshooting

### `error: target 'wasm32-unknown-unknown' not found`
This happens if the WASM target was not added correctly.
**Fix**: Run `rustup target add wasm32-unknown-unknown`.

### `'stellar' is not recognized as an internal or external command`
This occurs if the Cargo bin directory is not in your PATH.
**Fix**: Ensure `$HOME/.cargo/bin` (Linux/macOS) or `%USERPROFILE%\.cargo\bin` (Windows) is added to your environment's `PATH`.

### `soroban-cli` vs `stellar-cli`
The Soroban CLI was recently renamed to Stellar CLI.
**Note**: This repository uses `stellar-cli` commands. If you have an older `soroban-cli` installed, we recommend updating to `stellar-cli` to avoid command mismatches.

### `cargo` not found in certain shells
If you just installed Rust, you may need to restart your terminal or source your profile:
```bash
source $HOME/.cargo/env
```
