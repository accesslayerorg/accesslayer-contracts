#!/usr/bin/env bash

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

usage() {
  cat <<'EOF'
Usage: testnet-drill.sh --contract CONTRACT_ID --source KEY_ALIAS --admin ADMIN_ADDRESS [--execute]

Runs the legacy protocol pause -> verify -> unpause -> verify flow on Stellar
testnet only. Dry-run is the default. The source must be a Stellar CLI key
alias, not a secret.
EOF
}

CONTRACT=""
SOURCE=""
ADMIN=""
EXECUTE=false

while (($#)); do
  case "$1" in
    --contract) CONTRACT="${2:?Missing value for --contract}"; shift 2 ;;
    --source) SOURCE="${2:?Missing value for --source}"; shift 2 ;;
    --admin) ADMIN="${2:?Missing value for --admin}"; shift 2 ;;
    --execute) EXECUTE=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -z "$CONTRACT" || -z "$SOURCE" || -z "$ADMIN" ]]; then
  usage >&2
  exit 2
fi
validate_source_alias "$SOURCE"
reject_secret_literal "$CONTRACT"
reject_secret_literal "$ADMIN"
init_incident_log

READ_CMD=(stellar contract invoke --network testnet --source "$SOURCE" --id "$CONTRACT" --send=no -- get_is_paused)
PAUSE_CMD=(stellar contract invoke --network testnet --source "$SOURCE" --id "$CONTRACT" --send=yes -- pause --admin "$ADMIN")
UNPAUSE_CMD=(stellar contract invoke --network testnet --source "$SOURCE" --id "$CONTRACT" --send=yes -- unpause --admin "$ADMIN")

if [[ "$EXECUTE" != true ]]; then
  log_plan "${READ_CMD[@]}"
  log_plan "${PAUSE_CMD[@]}"
  log_plan "${READ_CMD[@]}"
  log_plan "${UNPAUSE_CMD[@]}"
  log_plan "${READ_CMD[@]}"
  printf 'Testnet dry run only. Re-run with --execute to run the drill after confirmation.\n'
  exit 0
fi

require_stellar_cli
confirm_submission "DRILL $CONTRACT"
initial_state="$(capture_logged "${READ_CMD[@]}")"
assert_bool_result "$initial_state" false
run_logged "${PAUSE_CMD[@]}"
paused_state="$(capture_logged "${READ_CMD[@]}")"
assert_bool_result "$paused_state" true
run_logged "${UNPAUSE_CMD[@]}"
resumed_state="$(capture_logged "${READ_CMD[@]}")"
assert_bool_result "$resumed_state" false
printf 'Testnet pause and recovery drill passed for %s.\n' "$CONTRACT"
