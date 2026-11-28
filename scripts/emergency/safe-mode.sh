#!/usr/bin/env bash

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

usage() {
  cat <<'EOF'
Usage:
  safe-mode.sh pause --network NETWORK --contract CONTRACT_ID --source KEY_ALIAS --admin ADMIN_ADDRESS --reason TEXT --duration-seconds SECONDS [--execute]
  safe-mode.sh resume --network NETWORK --contract CONTRACT_ID --source KEY_ALIAS --admin ADMIN_ADDRESS [--execute]
  safe-mode.sh unpause --network NETWORK --contract CONTRACT_ID --source KEY_ALIAS --admin ADMIN_ADDRESS [--execute]

Dry-run is the default. This manages CreatorKeysContract's bounded pause_all /
resume_all controls and the protocol-wide unpause entry point; it does not
invoke the separate SafeModeContract.
The source must be a Stellar CLI key alias, not a secret.
EOF
}

ACTION="${1:-}"
if [[ -z "$ACTION" ]]; then
  usage >&2
  exit 2
fi
if [[ "$ACTION" == -h || "$ACTION" == --help ]]; then
  usage
  exit 0
fi
shift
if [[ "$ACTION" != pause && "$ACTION" != resume && "$ACTION" != unpause ]]; then
  printf 'Action must be pause, resume, or unpause.\n' >&2
  usage >&2
  exit 2
fi

NETWORK=""
CONTRACT=""
SOURCE=""
ADMIN=""
REASON=""
DURATION=""
EXECUTE=false

while (($#)); do
  case "$1" in
    --network) NETWORK="${2:?Missing value for --network}"; shift 2 ;;
    --contract) CONTRACT="${2:?Missing value for --contract}"; shift 2 ;;
    --source) SOURCE="${2:?Missing value for --source}"; shift 2 ;;
    --admin) ADMIN="${2:?Missing value for --admin}"; shift 2 ;;
    --reason) REASON="${2:?Missing value for --reason}"; shift 2 ;;
    --duration-seconds) DURATION="${2:?Missing value for --duration-seconds}"; shift 2 ;;
    --execute) EXECUTE=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -z "$NETWORK" || -z "$CONTRACT" || -z "$SOURCE" || -z "$ADMIN" ]]; then
  usage >&2
  exit 2
fi
if [[ "$ACTION" == pause ]]; then
  if [[ -z "$REASON" || ${#REASON} -gt 512 || ! "$DURATION" =~ ^[0-9]+$ ||
    ${#DURATION} -gt 6 ]]; then
    printf 'Pause requires a 1-512 character reason and duration from 1 to 604800 seconds.\n' >&2
    exit 2
  fi
  DURATION=$((10#$DURATION))
  if (( DURATION < 1 || DURATION > 604800 )); then
    printf 'Pause requires a 1-512 character reason and duration from 1 to 604800 seconds.\n' >&2
    exit 2
  fi
fi
validate_source_alias "$SOURCE"
reject_secret_literal "$CONTRACT"
reject_secret_literal "$ADMIN"
reject_secret_literal "$REASON"
init_incident_log

if [[ "$ACTION" == unpause ]]; then
  READ_CMD=(stellar contract invoke --network "$NETWORK" --source "$SOURCE" --id "$CONTRACT" --send=no -- get_is_paused)
  MUTATION_CMD=(stellar contract invoke --network "$NETWORK" --source "$SOURCE" --id "$CONTRACT" --send=yes -- unpause --admin "$ADMIN")
  EXPECTED="UNPAUSE $NETWORK $CONTRACT"
  CONTROL=protocol
  TARGET_STATE=false
else
  READ_CMD=(stellar contract invoke --network "$NETWORK" --source "$SOURCE" --id "$CONTRACT" --send=no -- get_platform_pause)
  if [[ "$ACTION" == pause ]]; then
    MUTATION_CMD=(stellar contract invoke --network "$NETWORK" --source "$SOURCE" --id "$CONTRACT" --send=yes -- pause_all --admin "$ADMIN" --reason "$REASON" --max_duration_seconds "$DURATION")
    EXPECTED="PAUSE $NETWORK $CONTRACT"
    TARGET_STATE=true
  else
    MUTATION_CMD=(stellar contract invoke --network "$NETWORK" --source "$SOURCE" --id "$CONTRACT" --send=yes -- resume_all --admin "$ADMIN")
    EXPECTED="RESUME $NETWORK $CONTRACT"
    TARGET_STATE=false
  fi
  CONTROL=timed
fi

if [[ "$EXECUTE" != true ]]; then
  log_plan "${READ_CMD[@]}"
  log_plan "${MUTATION_CMD[@]}"
  log_plan "${READ_CMD[@]}"
  printf 'Dry run only. Re-run with --execute to submit after confirmation.\n'
  exit 0
fi

require_stellar_cli
confirm_submission "$EXPECTED"
current_state="$(capture_logged "${READ_CMD[@]}")"
if [[ "$CONTROL" == protocol ]]; then
  assert_bool_result "$current_state" true
elif [[ "$ACTION" == pause ]]; then
  assert_timed_pause_active "$current_state" false
else
  assert_timed_pause_active "$current_state" true
fi
run_logged "${MUTATION_CMD[@]}"
updated_state="$(capture_logged "${READ_CMD[@]}")"
if [[ "$CONTROL" == protocol ]]; then
  assert_bool_result "$updated_state" false
elif [[ "$TARGET_STATE" == true ]]; then
  assert_timed_pause_active "$updated_state" true
else
  assert_timed_pause_active "$updated_state" false
fi
if [[ "$CONTROL" == protocol ]]; then
  printf 'Protocol unpause verified for %s.\n' "$CONTRACT"
else
  printf 'Timed pause %s verified for %s.\n' "$ACTION" "$CONTRACT"
fi
