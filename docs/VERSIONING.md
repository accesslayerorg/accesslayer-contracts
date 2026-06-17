# Contract Versioning and WASM Hash Tracking

## Overview

The AccessLayer CreatorKeys contract is deployed as a Soroban smart contract on the Stellar blockchain. Each deployment produces a unique WASM binary with a distinct hash, which serves as the primary identifier for the contract version across different networks and deployment environments.

## Versioning Approach

### Semantic Versioning

The project follows semantic versioning for releases:
- Major version increments for breaking changes (e.g., incompatible state migrations, removed methods)
- Minor version increments for backward-compatible feature additions
- Patch version increments for backward-compatible bug fixes

Version information can be found in `Cargo.toml` in the workspace root.

### WASM Hash as Deployment Identifier

When the contract is compiled and deployed to the Stellar blockchain, the resulting WASM binary has a unique SHA-256 hash. This hash is not the same as a semantic version number, and a single semantic version may produce different hashes depending on the build environment (Rust compiler version, dependency versions, linker settings, etc.).

The WASM hash is deterministic for a given source tree and build configuration, making it the reliable way to verify that the exact code is running on-chain.

## Retrieving Contract WASM Hash

### From a Deployed Contract

Once a contract is deployed on Stellar, the WASM hash is publicly queryable:

```bash
soroban contract info \
  --id <CONTRACT_ID> \
  --network <NETWORK> \
  --rpc-url <RPC_ENDPOINT>
```

This returns metadata including the WASM hash of the deployed contract.

### From Compiled Source

To compute the WASM hash before deployment or to verify against a deployed instance:

1. Build the contract with the same environment:
   ```bash
   cd creator-keys
   cargo build --target wasm32-unknown-unknown --release
   ```

2. Compute the SHA-256 hash of the compiled binary:
   ```bash
   sha256sum target/wasm32-unknown-unknown/release/creator_keys.wasm
   ```

3. Compare the computed hash against the on-chain hash to verify they match.

## Canonical Hash Recording

### Release Artifacts

For each tagged release, the canonical WASM hash is recorded in:
- **Release notes** on GitHub under the version tag
- **Deployment logs** in the operations documentation (if available)
- **Contract registry** maintained by AccessLayer maintainers

### Verification Workflow

To verify you are running the expected contract version:

1. Note the WASM hash from the GitHub release notes for the version you intend to deploy
2. Query the contract hash on the target Stellar network (see "From a Deployed Contract" above)
3. Compare the two hashes; they must match exactly
4. If building from source, compile using the exact commit/tag and verify the build hash matches

## Build Determinism

The contract build is designed to be deterministic. However, differences may arise from:
- Rust toolchain version (specified in `rust-toolchain.toml`)
- Cargo dependency resolution (use `Cargo.lock` for reproducible builds)
- Build flags and optimization levels

To ensure reproducible builds when verifying a release:
1. Check out the exact release tag from git
2. Use the Rust version specified in `rust-toolchain.toml`
3. Ensure `Cargo.lock` is present and unmodified
4. Build with `--release` flag

## Best Practices

### For Operators
- Always verify the WASM hash before deploying a new contract version
- Record the hash alongside deployment timestamps and network information
- Use the canonical hash from GitHub releases as the source of truth

### For Contributors
- Do not manually edit `Cargo.lock` unless necessary for dependency updates
- When submitting a release, include the WASM hash in release notes
- Document any build configuration changes that might affect reproducibility

### For Users
- Verify the WASM hash when deploying through a third party
- Be skeptical of deployments with WASM hashes that don't match release records
- Use the `soroban contract info` command to inspect any running contract

## Emergency: Hash Mismatch

If the on-chain WASM hash does not match the expected canonical hash:
1. **Stop**: Do not assume it is safe; investigate before proceeding
2. **Verify source**: Recompile from the release tag and check your build environment
3. **Query deployment history**: Check logs or transaction records to understand when the mismatch occurred
4. **Escalate**: Contact AccessLayer maintainers via the security contact in `SECURITY.md`

A mismatch could indicate:
- Accidental deployment of an unreviewed version
- A build environment with different compiler/dependency versions
- A potential security incident (though this is rare if following proper deployment procedures)
