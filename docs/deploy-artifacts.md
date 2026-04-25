# Deploy artifacts: storage, naming, and metadata

This note complements [stellar-testnet-deployment.md](./stellar-testnet-deployment.md). The wasm produced by `stellar contract build` is the source of truth for what gets deployed, but the default build output under `target/` is **local, ephemeral, and not suitable as a long-term record** on its own.

## Where to store release artifacts

- **Do not** rely on a developer’s `target/` tree as the canonical copy: it is easy to rebuild from a different commit or toolchain and get a different hash.
- **Do** copy the built wasm (and a small sidecar file with the metadata below) to a team-controlled, access-managed location appropriate to your org, for example:
  - release assets on a VCS host (tagged release attachments)
  - an object store with access policies (e.g. S3 with restricted IAM)
  - a secured internal artifact registry
- **Never** put signing keys, funded seed phrases, or unredacted private RPC URLs next to the wasm; keep secrets in a proper secret store.

## Naming

Use a predictable pattern so humans and scripts can find the right binary without opening it. Recommended components (include at least **network** and **git commit**; add **version** or **date** when releasing):

- `creator_keys_<network>_<git_short_sha>.wasm` — ad-hoc or CI builds
- `creator_keys_<semver>_<network>_<build_date_utc>.wasm` — tagged releases

`network` examples: `testnet`, `mainnet-future` (or a Stellar network passphrase short id if you standardize one internally).

## Minimum metadata to record

Store alongside each wasm (JSON or a short `README` next to the file):

| Field | Why it matters |
|--------|----------------|
| **Git commit** (full or short SHA) | Reproducible code review and diff |
| **Repository** (URL or `org/repo` name) | Distinguish forks and mirrors |
| **Soroban / SDK version** (from this repo’s [Cargo workspace](../Cargo.toml)) | Toolchain compatibility for auditors and re-builds |
| **Rustc version** (from the build that produced the wasm) | Bit-identical rebuild attempts |
| **`stellar contract build` invocation** (or `Makefile` target) | Proves which package and profile were used |
| **Target triple** (e.g. `wasm32v1-none`) | Soroban standard |
| **SHA-256 of the wasm file** | Integrity check; compare before deploy |
| **Network** (e.g. testnet / future mainnet) | Prevents wrong-env deploys |
| **Contract id** (after deploy) | Bind off-chain config and indexers to an on-chain instance |

If you re-deploy the same built wasm, keep the same wasm hash and add a new row or note for the new contract id if the previous deployment is superseded.

## Retention

- **Testnet / review builds**: keep enough history for debugging (e.g. last N builds or last 30–90 days), then prune according to your org policy.
- **Tagged or production releases**: retain at least the wasm and metadata for the life of that deployment; longer if required for compliance or user support.

Contributors can validate a local build still matches the documented process using [stellar-testnet-deployment.md](./stellar-testnet-deployment.md) and `cargo test --workspace` in this repository.
