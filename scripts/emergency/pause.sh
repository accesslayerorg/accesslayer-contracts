#!/usr/bin/env bash

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

usage() {
  cat <<'EOF'
Usage: pause.sh --network NETWORK --contract CONTRACT_ID --source KEY_ALIAS --admin ADMIN_ADDRESS [--execute]

Dry-run is the default. --execute submits the protocol-wide pause after a
confirmation prompt. The source must be a Stellar CLI key alias, not a secret.
Set INCIDENT_LOG to choose a private command log path.
EOF
}

NETWORK=""
CONTRACT=""
SOURCE=""
ADMIN=""
EXECUTE=false

while (($#)); do
  case "$1" in
    --network) NETWORK="${2:?Missing value for --network}"; shift 2 ;;
    --contract) CONTRACT="${2:?Missing value for --contract}"; shift 2 ;;
    --source) SOURCE="${2:?Missing value for --source}"; shift 2 ;;
    --admin) ADMIN="${2:?Missing value for --admin}"; shift 2 ;;
    --execute) EXECUTE=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -z "$NETWORK" || -z "$CONTRACT" || -z "$SOURCE" || -z "$ADMIN" ]]; then
  usage >&2
  exit 2
fi
validate_source_alias "$SOURCE"
reject_secret_literal "$CONTRACT"
reject_secret_literal "$ADMIN"
init_incident_log

READ_CMD=(stellar contract invoke --network "$NETWORK" --source "$SOURCE" --id "$CONTRACT" --send=no -- get_is_paused)
PAUSE_CMD=(stellar contract invoke --network "$NETWORK" --source "$SOURCE" --id "$CONTRACT" --send=yes -- pause --admin "$ADMIN")

if [[ "$EXECUTE" != true ]]; then
  log_plan "${READ_CMD[@]}"
  log_plan "${PAUSE_CMD[@]}"
  log_plan "${READ_CMD[@]}"
  printf 'Dry run only. Re-run with --execute to submit after confirmation.\n'
  exit 0
fi

require_stellar_cli
confirm_submission "PAUSE $NETWORK $CONTRACT"
current_state="$(capture_logged "${READ_CMD[@]}")"
assert_bool_result "$current_state" false
run_logged "${PAUSE_CMD[@]}"
paused_state="$(capture_logged "${READ_CMD[@]}")"
assert_bool_result "$paused_state" true
printf 'Protocol pause verified for %s.\n' "$CONTRACT"
