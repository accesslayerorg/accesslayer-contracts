#!/usr/bin/env bash
# Run mutation tests with cargo-mutants for a single module or the full target set.
#
# Usage:
#   scripts/mutants.sh [module|file|all] [options] [-- <extra cargo-mutants args>]
#
# Modules:
#   all           Run mutation tests across the full target set in .cargo/mutants.toml (default)
#   fee           Fee modules (fee-discount-contract/src/lib.rs, creator-keys/src/dynamic_fees.rs)
#   curve         Curve module (creator-keys/src/curve_subscriptions_swaps.rs)
#   reserve       Reserve modules (holder-insurance-pool/src/lib.rs, creator-keys/src/multi_asset_payment.rs)
#   vesting       Vesting module (creator-keys/src/vesting.rs)
#   auth          Authorization modules (acl_roles.rs, badge_access_control.rs, guard.rs, multisig-admin)
#   <path.rs>     Any specific Rust source file in the workspace
#
# Options:
#   -o, --output <dir>    Output directory for cargo-mutants artifacts (default: mutants.out)
#   -j, --jobs <n>        Number of parallel mutation test jobs
#   --in-diff <file>      Only test mutants intersecting the given unified diff file
#   --list                List mutants without executing tests
#   --no-fail             Exit 0 even when surviving (missed) mutants are found
#   -h, --help            Show this help message

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

usage() {
    sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//'
}

if ! cargo mutants --version >/dev/null 2>&1; then
    echo "Error: cargo-mutants is not installed." >&2
    echo "Install it with: cargo install --locked cargo-mutants" >&2
    exit 1
fi

TARGET="all"
OUTPUT_DIR="."
JOBS=""
IN_DIFF=""
LIST_ONLY=0
NO_FAIL=0
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)
            usage
            exit 0
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -j|--jobs)
            JOBS="$2"
            shift 2
            ;;
        --in-diff)
            IN_DIFF="$2"
            shift 2
            ;;
        --list)
            LIST_ONLY=1
            shift
            ;;
        --no-fail)
            NO_FAIL=1
            shift
            ;;
        --)
            shift
            EXTRA_ARGS+=("$@")
            break
            ;;
        -*)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
        *)
            TARGET="$1"
            shift
            ;;
    esac
done

PKG_ARGS=()
FILE_ARGS=()

case "${TARGET}" in
    all)
        PKG_ARGS=(
            -p fee-discount-contract
            -p holder-insurance-pool
            -p common
            -p multisig-admin
            -p creator-keys
        )
        ;;
    fee)
        PKG_ARGS=(-p fee-discount-contract -p creator-keys)
        FILE_ARGS=(
            -f "fee-discount-contract/src/lib.rs"
            -f "creator-keys/src/dynamic_fees.rs"
        )
        ;;
    curve)
        PKG_ARGS=(-p creator-keys)
        FILE_ARGS=(-f "creator-keys/src/curve_subscriptions_swaps.rs")
        ;;
    reserve)
        PKG_ARGS=(-p holder-insurance-pool -p creator-keys)
        FILE_ARGS=(
            -f "holder-insurance-pool/src/lib.rs"
            -f "creator-keys/src/multi_asset_payment.rs"
        )
        ;;
    vesting)
        PKG_ARGS=(-p creator-keys)
        FILE_ARGS=(-f "creator-keys/src/vesting.rs")
        ;;
    auth|authorization)
        PKG_ARGS=(-p creator-keys -p common -p multisig-admin)
        FILE_ARGS=(
            -f "creator-keys/src/acl_roles.rs"
            -f "creator-keys/src/badge_access_control.rs"
            -f "common/src/guard.rs"
            -f "multisig-admin/src/lib.rs"
        )
        ;;
    *)
        if [[ ! -f "${TARGET}" ]]; then
            echo "Error: unknown module or missing file '${TARGET}'." >&2
            echo "Run 'scripts/mutants.sh --help' for valid modules." >&2
            exit 2
        fi
        FILE_ARGS=(-f "${TARGET}")
        CRATE_DIR="${TARGET%%/*}"
        if [[ -f "${CRATE_DIR}/Cargo.toml" ]]; then
            PKG_ARGS=(-p "${CRATE_DIR}")
        fi
        ;;
esac

CMD=(cargo mutants)
if [[ ${#PKG_ARGS[@]} -gt 0 ]]; then
    CMD+=("${PKG_ARGS[@]}")
fi
if [[ ${#FILE_ARGS[@]} -gt 0 ]]; then
    CMD+=("${FILE_ARGS[@]}")
fi
if [[ -n "${IN_DIFF}" ]]; then
    CMD+=(--in-diff "${IN_DIFF}")
fi
if [[ -n "${JOBS}" ]]; then
    CMD+=(-j "${JOBS}")
fi
if [[ ${LIST_ONLY} -eq 1 ]]; then
    CMD+=(--list)
    echo "Listing mutants for target '${TARGET}':"
    "${CMD[@]}" "${EXTRA_ARGS[@]}"
    exit 0
fi

mkdir -p "${OUTPUT_DIR}"
CMD+=(-o "${OUTPUT_DIR}")
if [[ ${#EXTRA_ARGS[@]} -gt 0 ]]; then
    CMD+=("${EXTRA_ARGS[@]}")
fi

echo "Running mutation tests for target '${TARGET}'..."
echo "Command: ${CMD[*]}"

set +e
"${CMD[@]}"
MUTANTS_EXIT=$?
set -e

RESULTS_DIR="${OUTPUT_DIR}/mutants.out"
if [[ ! -d "${RESULTS_DIR}" ]]; then
    RESULTS_DIR="${OUTPUT_DIR}"
fi

count_lines() {
    local file="$1"
    if [[ -f "${file}" ]]; then
        grep -c '[^[:space:]]' "${file}" 2>/dev/null || true
    else
        echo 0
    fi
}

CAUGHT=$(count_lines "${RESULTS_DIR}/caught.txt")
MISSED=$(count_lines "${RESULTS_DIR}/missed.txt")
TIMEOUT=$(count_lines "${RESULTS_DIR}/timeout.txt")
UNVIABLE=$(count_lines "${RESULTS_DIR}/unviable.txt")
TESTED=$((CAUGHT + MISSED + TIMEOUT))

echo ""
echo "=== Mutation Testing Summary (${TARGET}) ==="
echo "  Caught:   ${CAUGHT}"
echo "  Missed:   ${MISSED}"
echo "  Timeout:  ${TIMEOUT}"
echo "  Unviable: ${UNVIABLE}"
echo "  Total:    $((TESTED + UNVIABLE))"

if [[ ${MISSED} -gt 0 && -f "${RESULTS_DIR}/missed.txt" ]]; then
    echo ""
    echo "Surviving (missed) mutants:"
    sed 's/^/  - /' "${RESULTS_DIR}/missed.txt"
fi

if [[ ${NO_FAIL} -eq 1 ]]; then
    exit 0
fi

exit "${MUTANTS_EXIT}"
