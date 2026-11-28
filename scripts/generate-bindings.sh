#!/usr/bin/env bash
#
# Generates the TypeScript bindings in ./bindings from the compiled contract WASM.
#
# For every contract in the Cargo workspace this:
#   1. builds (or consumes) its WASM,
#   2. runs `stellar contract bindings typescript`,
#   3. runs `stellar contract info interface` to capture the contract spec,
#   4. post-processes everything into the committed `bindings/` package
#      (typed clients, error enums, event decoders, manifest with WASM hashes).
#
# Usage:
#   ./scripts/generate-bindings.sh                 # build every contract, regenerate
#   ./scripts/generate-bindings.sh --contracts creator-keys,treasury-contract
#   ./scripts/generate-bindings.sh --wasm-dir target/wasm32v1-none/release
#   ./scripts/generate-bindings.sh --no-build      # reuse an existing WASM build
#   ./scripts/generate-bindings.sh --strict        # fail if a contract cannot be built
#
# Requirements: stellar-cli (>= 22), Rust with the wasm32v1-none target installed,
# and Node.js (>= 18) for the post-processing step.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

STELLAR_BIN="${STELLAR_BIN:-stellar}"
WASM_DIR="${WASM_DIR:-}"
WORK_DIR="$ROOT/target/bindings-work"
OUT_DIR="$ROOT/bindings"
WASM_TARGET="${WASM_TARGET:-wasm32v1-none}"
CONTRACTS=""
BUILD=1
STRICT=0

usage() {
  sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
}

while [ $# -gt 0 ]; do
  case "$1" in
    --wasm-dir) WASM_DIR="$2"; shift 2 ;;
    --wasm-dir=*) WASM_DIR="${1#*=}"; shift ;;
    --contracts) CONTRACTS="$2"; shift 2 ;;
    --contracts=*) CONTRACTS="${1#*=}"; shift ;;
    --work) WORK_DIR="$2"; shift 2 ;;
    --out) OUT_DIR="$2"; shift 2 ;;
    --no-build) BUILD=0; shift ;;
    --strict) STRICT=1; shift ;;
    -h|--help) usage ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if ! command -v "$STELLAR_BIN" >/dev/null 2>&1; then
  echo "error: '$STELLAR_BIN' not found. Install the Stellar CLI (see docs/local-soroban-prerequisites.md)." >&2
  exit 1
fi
if ! command -v node >/dev/null 2>&1; then
  echo "error: node (>= 18) is required for the post-processing step." >&2
  exit 1
fi

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

CONTRACTS_JSON="$WORK_DIR/contracts.json"
SKIPPED_JSON="$WORK_DIR/skipped.json"
printf '[' > "$CONTRACTS_JSON"
printf '[' > "$SKIPPED_JSON"
SKIPPED_COUNT=0
FIRST_ENTRY=1

skip_contract() {
  local crate="$1" reason="$2"
  echo "  ! skipping $crate: $reason" >&2
  if [ "$SKIPPED_COUNT" -gt 0 ]; then printf ',' >> "$SKIPPED_JSON"; fi
  printf '{"crate":"%s","reason":"%s"}' "$crate" "$reason" >> "$SKIPPED_JSON"
  SKIPPED_COUNT=$((SKIPPED_COUNT + 1))
  if [ "$STRICT" -eq 1 ]; then
    SKIPPED_STRICT+=("$crate")
  fi
}
SKIPPED_STRICT=()

# Discover contracts: every workspace member whose crate produces a cdylib.
discover_contracts() {
  local members
  members="$(sed -n '/^members = \[/,/^\]/p' Cargo.toml | grep -o '"[^"]*"' | tr -d '"' | sort -u)"
  for dir in $members; do
    [ -f "$dir/Cargo.toml" ] || continue
    grep -q 'cdylib' "$dir/Cargo.toml" || continue
    local crate
    crate="$(grep -m1 '^name' "$dir/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
    printf '%s\n' "$crate"
  done | sort -u
}

resolve_wasm() {
  local crate="$1"
  local underscored
  underscored="${crate//-/_}"
  if [ -n "$WASM_DIR" ]; then
    for candidate in "$WASM_DIR/$crate.wasm" "$WASM_DIR/$underscored.wasm"; do
      if [ -f "$candidate" ]; then
        printf '%s\n' "$candidate"
        return 0
      fi
    done
    return 1
  fi
  local target_dir="${CARGO_TARGET_DIR:-$ROOT/target}"
  local built="$target_dir/$WASM_TARGET/release/$underscored.wasm"
  if [ -f "$built" ] && [ "$BUILD" -eq 0 ]; then
    printf '%s\n' "$built"
    return 0
  fi
  if [ "$BUILD" -eq 1 ]; then
    if ! cargo build --release --target "$WASM_TARGET" -p "$crate" >"$WORK_DIR/$crate.build.log" 2>&1; then
      return 1
    fi
    if [ -f "$built" ]; then
      printf '%s\n' "$built"
      return 0
    fi
    return 1
  fi
  return 1
}

if [ -n "$CONTRACTS" ]; then
  CRATES="$(printf '%s' "$CONTRACTS" | tr ',' '\n')"
else
  CRATES="$(discover_contracts)"
fi

for crate in $CRATES; do
  [ -n "$crate" ] || continue
  crate_dir="$(grep -rl "^name = \"$crate\"$" --include Cargo.toml . 2>/dev/null | grep -v '^./target' | head -1 || true)"
  crate_dir="$(dirname "${crate_dir:-./$crate}")"
  crate_dir="${crate_dir#./}"

  echo "==> $crate"
  if ! wasm="$(resolve_wasm "$crate")"; then
    if [ -f "$WORK_DIR/$crate.build.log" ]; then
      # The reason is committed, so it must not contain compiler wording:
      # diagnostics change between Rust releases and would show up as binding
      # drift. Print the real error here and keep the manifest entry stable.
      reason="wasm build failed"
      grep -m1 '^error' "$WORK_DIR/$crate.build.log" >&2 || true
    else
      reason="wasm not found (run without --no-build, or pass --wasm-dir)"
    fi
    skip_contract "$crate" "$reason"
    continue
  fi
  wasm="$(cd "$(dirname "$wasm")" && pwd)/$(basename "$wasm")"

  mkdir -p "$WORK_DIR/$crate"
  if ! "$STELLAR_BIN" contract bindings typescript \
      --wasm "$wasm" \
      --output-dir "$WORK_DIR/$crate/client" \
      --overwrite >"$WORK_DIR/$crate/bindings.log" 2>&1; then
    skip_contract "$crate" "stellar contract bindings typescript failed (see $WORK_DIR/$crate/bindings.log)"
    continue
  fi
  if ! "$STELLAR_BIN" contract info interface \
      --wasm "$wasm" \
      --output json >"$WORK_DIR/$crate/spec.json" 2>"$WORK_DIR/$crate/spec.log"; then
    skip_contract "$crate" "stellar contract info interface failed (see $WORK_DIR/$crate/spec.log)"
    continue
  fi

  # Only the interface hash is recorded. WASM bytes embed absolute toolchain
  # paths, so a WASM hash is not reproducible across machines or Rust releases.
  interface_sha="$(sha256sum "$WORK_DIR/$crate/spec.json" | cut -d' ' -f1)"
  if [ "$FIRST_ENTRY" -eq 0 ]; then printf ',' >> "$CONTRACTS_JSON"; fi
  FIRST_ENTRY=0
  printf '{"name":"%s","crate":"%s","crateDir":"%s","interfaceSha256":"%s"}' \
    "$crate" "$crate" "$crate_dir" "$interface_sha" >> "$CONTRACTS_JSON"
  printf '\n'
done
printf ']' >> "$CONTRACTS_JSON"
printf ']' >> "$SKIPPED_JSON"
printf '\n' >> "$CONTRACTS_JSON"

echo "==> post-processing into $OUT_DIR"
node "$ROOT/scripts/postprocess-bindings.mjs" --work "$WORK_DIR" --out "$OUT_DIR"

if [ "$STRICT" -eq 1 ] && [ "${#SKIPPED_STRICT[@]}" -gt 0 ]; then
  echo "error: --strict and these contracts could not be built: ${SKIPPED_STRICT[*]}" >&2
  exit 1
fi
