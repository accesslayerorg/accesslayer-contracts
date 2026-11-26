#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "Usage: verify-deployment.sh <git-tag> <contract-id> <cargo-package> [rpc-url]" >&2
    echo "Set SOURCE_REPOSITORY to verify a tag from a repository mirror or fork." >&2
    exit 2
}

verification_error() {
    echo "Error: $1" >&2
    exit 2
}

[[ $# -ge 3 && $# -le 4 ]] || usage

tag=$1
contract_id=$2
package=$3
rpc_url=${4:-${SOROBAN_RPC_URL:-https://soroban-rpc.mainnet.stellar.org}}
repository=${SOURCE_REPOSITORY:-https://github.com/accesslayerorg/accesslayer-contracts.git}

for dependency in git docker python3 sha256sum cut; do
    if ! command -v "$dependency" >/dev/null 2>&1; then
        echo "Error: required dependency not found: $dependency" >&2
        exit 2
    fi
done

if ! git check-ref-format "refs/tags/$tag"; then
    echo "Error: invalid Git tag: $tag" >&2
    exit 2
fi
if [[ -z "$package" || "$package" == -* ]]; then
    echo "Error: invalid Cargo package name: $package" >&2
    exit 2
fi

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
python3 "$script_dir/verify-rpc.py" --validate-only "$contract_id" "$rpc_url"

if ! temporary_dir=$(mktemp -d); then
    verification_error "could not create a temporary working directory"
fi
trap 'rm -rf "$temporary_dir"' EXIT
source_dir="$temporary_dir/source"
output_dir="$temporary_dir/output"
if ! mkdir -p "$output_dir"; then
    verification_error "could not create a temporary artifact directory"
fi

echo "Checking out tag '$tag' from $repository"
if ! git clone --quiet --depth 1 --branch "$tag" "$repository" "$source_dir"; then
    verification_error "could not check out tag '$tag' from the configured repository"
fi
if ! commit=$(git -C "$source_dir" rev-parse HEAD); then
    verification_error "could not resolve tag '$tag' to a commit"
fi
if [[ ! "$commit" =~ ^[0-9a-f]{40}$ ]]; then
    verification_error "tag '$tag' did not resolve to a full commit SHA"
fi

image="accesslayer-reproducible-build:${commit}"
if ! docker build --quiet --file "$source_dir/Dockerfile" --tag "$image" "$source_dir"; then
    verification_error "Docker could not build the pinned release image"
fi
if ! docker run --rm \
    --env "SOURCE_COMMIT=$commit" \
    --volume "$source_dir:/workspace:ro" \
    --volume "$output_dir:/output" \
    "$image" "$package" /output; then
    verification_error "the tagged source failed to build the selected Cargo package"
fi
if ! local_wasm=$(python3 - "$output_dir/build-info.json" "$package" "$output_dir" <<'PY'
import json
import pathlib
import sys

info = json.loads(pathlib.Path(sys.argv[1]).read_text())
for artifact in info.get("artifacts", []):
    if artifact.get("identifier") == sys.argv[2]:
        print(pathlib.Path(sys.argv[3]) / artifact["file"])
        break
else:
    raise SystemExit(f"Error: build metadata has no artifact for Cargo package '{sys.argv[2]}'")
PY
); then
    verification_error "build metadata has no usable artifact for Cargo package '$package'"
fi
if [[ ! -f "$local_wasm" ]]; then
    verification_error "expected artifact for Cargo package '$package' was not produced"
fi
if ! local_hash=$(sha256sum "$local_wasm" | cut -d ' ' -f 1); then
    verification_error "could not calculate the local WASM SHA-256 hash"
fi

echo "Querying deployed contract via RPC"
if ! deployed_hash=$(python3 "$source_dir/scripts/verify-rpc.py" "$contract_id" "$rpc_url"); then
    echo "Error: unable to retrieve deployed contract WASM hash; no comparison was made" >&2
    exit 2
fi

printf 'Tag: %s\nCommit: %s\nCargo package: %s\nLocal WASM SHA-256: %s\nDeployed WASM SHA-256: %s\n' \
    "$tag" "$commit" "$package" "$local_hash" "$deployed_hash"

if [[ "$local_hash" == "$deployed_hash" ]]; then
    echo "MATCH: deployed contract uses the WASM built from $tag ($commit)."
    exit 0
fi

echo "MISMATCH: deployed contract WASM does not match package '$package' built from $tag."
exit 1
