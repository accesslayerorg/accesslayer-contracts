#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "Usage: build-wasm.sh <cargo-package|--all> [output-directory]" >&2
    exit 2
}

[[ $# -ge 1 && $# -le 2 ]] || usage

package_selector=$1
output_dir=${2:-/output}
workspace=/workspace

if [[ ! -f "$workspace/Cargo.toml" ]]; then
    echo "Error: no Cargo workspace found at $workspace" >&2
    exit 2
fi

mkdir -p "$output_dir"

export SOURCE_DATE_EPOCH=0
export TZ=UTC
export LC_ALL=C
export LANG=C
export CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR=/tmp/cargo-target
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--remap-path-prefix=/workspace=. --remap-path-prefix=/usr/local/cargo/registry=/cargo/registry --remap-path-prefix=/tmp/cargo-target=/target"

packages_file=$(mktemp)
trap 'rm -f "$packages_file"' EXIT

cargo metadata --locked --no-deps --format-version 1 --manifest-path "$workspace/Cargo.toml" |
    python3 -c '
import json
import sys

metadata = json.load(sys.stdin)
selector = sys.argv[1]
targets = []
for package in metadata["packages"]:
    for target in package["targets"]:
        if "cdylib" not in target.get("crate_types", target.get("kind", [])):
            continue
        if selector == "--all" or package["name"] == selector:
            targets.append((package["name"], target["name"]))

if selector != "--all" and not targets:
    raise SystemExit(f"No cdylib Cargo target found for package: {selector}")
if selector != "--all" and len(targets) != 1:
    raise SystemExit(f"Expected one cdylib target for package {selector}, found {len(targets)}")
if not targets:
    raise SystemExit("No cdylib Cargo targets found in this workspace")
if len({name for _, name in targets}) != len(targets):
    raise SystemExit("Duplicate cdylib target names; refusing to overwrite artifacts")
for package, target in targets:
    print(f"{package}\t{target}")
' "$package_selector" > "$packages_file"

if [[ -z "${SOURCE_COMMIT:-}" ]]; then
    SOURCE_COMMIT=$(git -C "$workspace" rev-parse HEAD)
fi
if [[ ! "$SOURCE_COMMIT" =~ ^[0-9a-f]{40}$ ]]; then
    echo "Error: SOURCE_COMMIT must be a full lowercase Git SHA" >&2
    exit 2
fi

while IFS=$'\t' read -r package target; do
    echo "Building Cargo package '$package' (WASM target '$target')"
    cargo build --locked --release --target wasm32v1-none \
        --package "$package" --manifest-path "$workspace/Cargo.toml"
    artifact="$CARGO_TARGET_DIR/wasm32v1-none/release/${target}.wasm"
    if [[ ! -f "$artifact" ]]; then
        echo "Error: expected WASM artifact was not produced: $artifact" >&2
        exit 1
    fi
    cp "$artifact" "$output_dir/${target}.wasm"
done < "$packages_file"

python3 - "$output_dir" "$SOURCE_COMMIT" "$SOROBAN_CLI_VERSION" "$packages_file" <<'PY'
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys

output_dir = pathlib.Path(sys.argv[1])
source_commit = sys.argv[2]
soroban_cli_version = sys.argv[3]
packages_file = pathlib.Path(sys.argv[4])
rust_version = subprocess.check_output(["rustc", "--version"], text=True).split()[1]
cli_output = subprocess.check_output(["stellar", "--version"], text=True).strip()
cli_match = re.search(r"\b\d+\.\d+\.\d+\b", cli_output)
if not cli_match or cli_match.group(0) != soroban_cli_version:
    raise SystemExit(f"Installed Stellar CLI does not match pin {soroban_cli_version}: {cli_output}")

artifacts = []
for line in packages_file.read_text().splitlines():
    package, target = line.split("\t")
    path = output_dir / f"{target}.wasm"
    if not path.is_file():
        raise SystemExit(f"Build output is missing: {path.name}")
    artifacts.append({
        "identifier": package,
        "target": target,
        "file": path.name,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    })
if not artifacts:
    raise SystemExit("No WASM artifacts were produced")

info = {
    "source_commit": source_commit,
    "rust_toolchain": rust_version,
    "soroban_cli_version": cli_match.group(0),
    "cargo_version": subprocess.check_output(["cargo", "--version"], text=True).strip(),
    "target": "wasm32v1-none",
    "source_date_epoch": int(os.environ["SOURCE_DATE_EPOCH"]),
    "artifacts": artifacts,
}
(output_dir / "build-info.json").write_text(json.dumps(info, indent=2) + "\n")
PY
