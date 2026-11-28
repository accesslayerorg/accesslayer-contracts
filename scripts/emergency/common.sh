#!/usr/bin/env bash

set -euo pipefail

init_incident_log() {
  umask 077
  local log_dir
  INCIDENT_LOG="${INCIDENT_LOG:-${HOME:?HOME must be set}/.local/state/accesslayer/incident-commands.log}"
  log_dir="$(dirname "$INCIDENT_LOG")"
  mkdir -p "$log_dir"
  touch "$INCIDENT_LOG"
  chmod 600 "$INCIDENT_LOG"
}

validate_source_alias() {
  local source="$1"
  if [[ "$source" =~ ^S[A-Z2-7]{55}$ ]]; then
    printf 'Pass a Stellar CLI key alias, never a secret key.\n' >&2
    return 2
  fi
}

reject_secret_literal() {
  local value="$1"
  if [[ "$value" =~ ^S[A-Z2-7]{55}$ ]]; then
    printf 'A secret-key-shaped value is not accepted by emergency scripts.\n' >&2
    return 1
  fi
}

log_command() {
  local rendered
  printf -v rendered '%q ' "$@"
  printf '[%s] $ %s\n' "$(date -u +%FT%TZ)" "${rendered% }" | tee -a "$INCIDENT_LOG" >&2
}

log_plan() {
  local rendered
  printf -v rendered '%q ' "$@"
  printf 'DRY RUN [%s] $ %s\n' "$(date -u +%FT%TZ)" "${rendered% }" | tee -a "$INCIDENT_LOG"
}

run_logged() {
  log_command "$@"
  local command_status=0
  "$@" 2>&1 | tee -a "$INCIDENT_LOG" || command_status="${PIPESTATUS[0]}"
  if (( command_status != 0 )); then
    printf '[%s] command failed with exit code %s\n' \
      "$(date -u +%FT%TZ)" "$command_status" | tee -a "$INCIDENT_LOG" >&2
  fi
  return "$command_status"
}

capture_logged() {
  log_command "$@"
  local output
  if ! output="$("$@" 2>&1)"; then
    printf '%s\n' "$output" | tee -a "$INCIDENT_LOG" >&2
    printf '[%s] command failed\n' "$(date -u +%FT%TZ)" | tee -a "$INCIDENT_LOG" >&2
    return 1
  fi
  printf '%s\n' "$output" | tee -a "$INCIDENT_LOG" >&2
  printf '%s' "$output"
}

confirm_submission() {
  local expected="$1"
  local response
  if [[ ! -t 0 ]]; then
    printf 'Submission requires an interactive terminal.\n' >&2
    return 2
  fi
  printf 'Type "%s" to submit: ' "$expected" >&2
  read -r response
  if [[ "$response" != "$expected" ]]; then
    printf 'Confirmation did not match; no transaction was submitted.\n' >&2
    return 2
  fi
}

assert_bool_result() {
  local output="$1"
  local expected="$2"
  local normalized
  normalized="$(printf '%s' "$output" | tr -d '[:space:]"')"
  if [[ "$normalized" != "$expected" ]]; then
    printf 'Expected contract view to return %s; received: %s\n' "$expected" "$output" >&2
    return 1
  fi
}

assert_timed_pause_active() {
  local output="$1"
  local expected="$2"
  python3 - "$expected" "$output" <<'PY'
import json
import sys

expected = sys.argv[1] == "true"
raw = sys.argv[2].strip()
try:
    value = json.loads(raw)
except json.JSONDecodeError as error:
    raise SystemExit(f"Could not parse get_platform_pause output as JSON: {error}")

if value is None:
    active = False
elif isinstance(value, dict) and isinstance(value.get("active"), bool):
    active = value["active"]
else:
    raise SystemExit(f"Unexpected get_platform_pause result: {raw}")

if active != expected:
    raise SystemExit(f"Expected timed pause active={expected}, received active={active}")
PY
}

require_stellar_cli() {
  if ! command -v stellar >/dev/null 2>&1; then
    printf 'The Stellar CLI ("stellar") is required for execution.\n' >&2
    return 127
  fi
}
