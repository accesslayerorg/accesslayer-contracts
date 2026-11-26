# Reproducible WASM build and deployment verification

This guide lets an independent auditor compare a deployed Soroban contract's WASM
hash with a build of a specific tagged source commit. Verification uses public
source, a public Stellar/Soroban RPC, and Docker; it needs no signing key,
funded account, or privileged credentials.

## Required tools

- Git
- Docker
- Python 3
- `sha256sum`

The release container pins Rust **1.84.0**, the `wasm32v1-none` target,
`clippy`/`rustfmt`, and Stellar CLI **22.0.0**. Cargo dependencies are resolved
from the committed `Cargo.lock` with `--locked`.

## Choose the release tag, commit, and contract package

Start from the GitHub release for the deployed contract and note its tag. Confirm
that the tag and commit correspond to the release metadata asset `releases.json`.
Each entry identifies the Cargo contract target, full Git commit, WASM SHA-256,
Rust toolchain, and Stellar CLI version.

For example:

```sh
git fetch --tags https://github.com/accesslayerorg/accesslayer-contracts.git
git rev-parse 'v1.2.3^{commit}'
```

Select the `contract_wasm_identifier` / Cargo package for the contract being
verified. This repository contains multiple contracts; the package is required
so the verifier never compares a deployment to an unrelated WASM artifact.
The deployment's contract ID alone does not identify which source package built
its WASM.

## Reproduce a WASM build manually

From a checkout containing the selected tag:

```sh
git checkout --detach v1.2.3
commit="$(git rev-parse HEAD)"
docker build --file Dockerfile --tag accesslayer-wasm-build .
mkdir -p out
docker run --rm \
  --env "SOURCE_COMMIT=$commit" \
  --volume "$PWD:/workspace:ro" \
  --volume "$PWD/out:/output" \
  accesslayer-wasm-build creator-keys /output
sha256sum out/creator_keys.wasm
cat out/build-info.json
```

Replace `creator-keys` with the package named in the release metadata. Artifacts
are named after the Cargo `cdylib` target.
The build fixes `SOURCE_DATE_EPOCH=0`, locale and timezone, disables Cargo
incremental compilation, uses a fresh container-local target directory, and
remaps workspace and registry source paths.

The image pins the official `rust:1.84.0-bookworm` base image by digest and
targets `linux/amd64`; the Stellar CLI binary is pinned to its official release
asset by SHA-256. Rust, Cargo dependencies, OS image, and CLI are pinned.
Non-amd64 hosts need Docker emulation to use this image.

## Verify a deployed contract

The default public network is Stellar mainnet. For testnet or another compatible
public RPC, pass its URL as the fourth argument or set `SOROBAN_RPC_URL`.

```sh
scripts/verify-deployment.sh v1.2.3 C... creator-keys
scripts/verify-deployment.sh v1.2.3 C... creator-keys https://soroban-testnet.stellar.org
```

The script fetches the exact tag from the public source repository, builds only
the selected Cargo package in the pinned container, and requests the contract
instance ledger entry using the public RPC `getLedgerEntries` method. It reads
the WASM hash from the contract instance's executable field and compares that
value with SHA-256 of the local WASM file.

To retrieve only the on-chain hash, without building a WASM artifact, run:

```sh
python3 scripts/verify-rpc.py C... https://soroban-rpc.mainnet.stellar.org
```

The helper constructs the contract-instance ledger key, calls
`getLedgerEntries`, and prints the executable WASM hash as lowercase hex. RPC
errors and missing contracts are reported as errors, not hashes.

To verify a fork or mirror, set `SOURCE_REPOSITORY` to its Git URL. The specified
tag must exist in that repository.

## Interpret the result

- **MATCH** with exit status `0`: the selected package built from the tag's
  resolved commit has the same SHA-256 as the deployed contract's WASM executable.
- **MISMATCH** with exit status `1`: both hashes were retrieved successfully but
  differ. Check the package, network, release tag, and any build/deployment
  transformations before drawing conclusions.
- **Verification error** with exit status `2`: argument/dependency validation,
  Docker build, tag checkout, RPC, or response parsing failed. RPC failures are
  not reported as mismatches, and no comparison is claimed.

The contract ID must be a valid checksummed Stellar contract StrKey. A contract
on the wrong RPC network or a contract with no current instance will produce a
verification error rather than a mismatch.

## Inspect `releases.json`

The checked-in `releases.json` is an empty schema template. For each published
GitHub release, the release workflow checks out the exact release tag, builds
all workspace `cdylib` artifacts, and attaches a generated `releases.json` asset
to that GitHub release. The generated document has a record for each WASM
artifact with its tag, full commit SHA, identifier, SHA-256, Rust version, and
Stellar CLI version. Verify that the tag resolves to the recorded commit and
that the artifact identifier is the package you intend to verify.

Pretty-print a checked-in or downloaded release asset with:

```sh
python3 -m json.tool releases.json
```

## Troubleshooting

- **Docker daemon unavailable:** start Docker and confirm `docker info` works;
  no host Rust installation is needed for the container build.
- **Image build or package install fails:** check Docker access to the Rust
  distribution server and the pinned Stellar CLI release asset. WASM builds
  also need access to the Cargo registry on first use.
- **Cargo fails with a lockfile error:** ensure the selected tag includes its
  `Cargo.lock`; builds intentionally use `--locked` and do not update it.
- **Unknown package / missing artifact:** use an identifier from the release
  metadata and confirm that it is a workspace package with a `cdylib` target.
- **RPC error or missing contract:** verify the contract ID and choose an RPC
  endpoint for the network where it is deployed. Public RPC service availability
  and retention are external dependencies.
- **Different hashes:** confirm the exact release tag, commit, package, target
  architecture, and release asset. The verifier compares only the explicitly
  selected package.
