#!/usr/bin/env bash

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

usage() {
  cat <<'EOF'
Usage: snapshot.sh --network NETWORK --contract CONTRACT_ID --source KEY_ALIAS --output FILE [--records TSV] [--execute]

Dry-run is the default. --execute performs read-only contract simulations and
exports JSON only if all observations fit one ledger. STELLAR_RPC_URL must be
the same RPC endpoint configured for the selected Stellar CLI network.
The optional TSV supports: balance CREATOR WALLET, payment_reserve TOKEN,
escape_reserve KEY_ID, claimable_dividend CREATOR WALLET,
auction_pending CREATOR, and vault_pending_rewards CREATOR WALLET.
EOF
}

NETWORK=""
CONTRACT=""
SOURCE=""
OUTPUT=""
RECORDS=""
EXECUTE=false

while (($#)); do
  case "$1" in
    --network) NETWORK="${2:?Missing value for --network}"; shift 2 ;;
    --contract) CONTRACT="${2:?Missing value for --contract}"; shift 2 ;;
    --source) SOURCE="${2:?Missing value for --source}"; shift 2 ;;
    --output) OUTPUT="${2:?Missing value for --output}"; shift 2 ;;
    --records) RECORDS="${2:?Missing value for --records}"; shift 2 ;;
    --execute) EXECUTE=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -z "$NETWORK" || -z "$CONTRACT" || -z "$SOURCE" || -z "$OUTPUT" ]]; then
  usage >&2
  exit 2
fi
if [[ -n "$RECORDS" && ! -r "$RECORDS" ]]; then
  printf 'Cannot read records file: %s\n' "$RECORDS" >&2
  exit 2
fi
if [[ "$EXECUTE" == true && -e "$OUTPUT" ]]; then
  printf 'Refusing to overwrite existing snapshot: %s\n' "$OUTPUT" >&2
  exit 2
fi
validate_source_alias "$SOURCE"
reject_secret_literal "$CONTRACT"
init_incident_log

RPC_URL="${STELLAR_RPC_URL:-}"
if [[ "$EXECUTE" == true && -z "$RPC_URL" ]]; then
  printf 'Set STELLAR_RPC_URL to the selected CLI network RPC endpoint.\n' >&2
  exit 2
fi

READ_BASE=(stellar contract invoke --network "$NETWORK" --source "$SOURCE" --id "$CONTRACT" --send=no --)
CORE_METHODS=(
  get_is_paused
  get_global_trading_paused
  is_paused
  get_treasury_balance
  get_reward_pool_balance
  get_payment_base_reserve
)

plan_record() {
  local kind="$1"
  local first="$2"
  local second="${3:-}"
  local extra="${4:-}"
  reject_secret_literal "$first"
  reject_secret_literal "$second"
  reject_secret_literal "$extra"
  if [[ -n "$extra" || -z "$first" ]]; then
    printf 'Invalid TSV record: %s %s %s %s\n' "$kind" "$first" "$second" "$extra" >&2
    return 2
  fi
  case "$kind" in
    balance)
      [[ -n "$second" ]] || { printf 'balance requires CREATOR and WALLET\n' >&2; return 2; }
      log_plan "${READ_BASE[@]}" get_key_balance --creator "$first" --wallet "$second"
      ;;
    payment_reserve)
      [[ -z "$second" ]] || { printf 'payment_reserve takes one TOKEN\n' >&2; return 2; }
      log_plan "${READ_BASE[@]}" get_payment_asset_reserve --token "$first"
      ;;
    escape_reserve)
      [[ -z "$second" ]] || { printf 'escape_reserve takes one KEY_ID\n' >&2; return 2; }
      log_plan "${READ_BASE[@]}" get_escape_reserve --key_id "$first"
      ;;
    claimable_dividend)
      [[ -n "$second" ]] || { printf 'claimable_dividend requires CREATOR and WALLET\n' >&2; return 2; }
      log_plan "${READ_BASE[@]}" get_claimable_dividend --creator "$first" --wallet "$second"
      ;;
    auction_pending)
      [[ -z "$second" ]] || { printf 'auction_pending takes one CREATOR\n' >&2; return 2; }
      log_plan "${READ_BASE[@]}" is_auction_pending --creator "$first"
      ;;
    vault_pending_rewards)
      [[ -n "$second" ]] || { printf 'vault_pending_rewards requires CREATOR and WALLET\n' >&2; return 2; }
      log_plan "${READ_BASE[@]}" get_vault_pending_rewards --creator "$first" --holder "$second"
      ;;
    *)
      printf 'Unsupported record type: %s\n' "$kind" >&2
      return 2
      ;;
  esac
}

if [[ "$EXECUTE" != true ]]; then
  for method in "${CORE_METHODS[@]}"; do
    log_plan "${READ_BASE[@]}" "$method"
  done
  if [[ -n "$RECORDS" ]]; then
    while IFS=$'\t' read -r kind first second extra || [[ -n "${kind:-}" ]]; do
      [[ -z "${kind:-}" || "$kind" == \#* ]] && continue
      reject_secret_literal "${first:-}"
      reject_secret_literal "${second:-}"
      reject_secret_literal "${extra:-}"
      plan_record "$kind" "${first:-}" "${second:-}" "${extra:-}"
    done < "$RECORDS"
  fi
  printf 'Dry run only. Re-run with --execute to export a ledger-consistent snapshot.\n'
  exit 0
fi

require_stellar_cli
if ! command -v curl >/dev/null 2>&1; then
  printf 'curl is required to pin the snapshot to an RPC ledger.\n' >&2
  exit 127
fi
if ! command -v python3 >/dev/null 2>&1; then
  printf 'python3 is required to write the snapshot JSON.\n' >&2
  exit 127
fi

output_dir="$(dirname "$OUTPUT")"
mkdir -p "$output_dir"
rows_file="$(mktemp "$output_dir/.incident-snapshot-rows.XXXXXX")"
json_file="$(mktemp "$output_dir/.incident-snapshot-json.XXXXXX")"
trap 'rm -f "$rows_file" "$json_file"' EXIT

latest_ledger() {
  printf '[%s] $ curl -fsS <STELLAR_RPC_URL> -H Content-Type:application/json -d getLatestLedger\n' \
    "$(date -u +%FT%TZ)" | tee -a "$INCIDENT_LOG" >&2
  local response
  response="$(curl -fsS "$RPC_URL" \
    -H 'Content-Type: application/json' \
    --data '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger","params":{}}')"
  python3 -c '
import json, sys
result = json.load(sys.stdin)["result"]
print(str(result["sequence"]) + "\t" + result["id"])
' <<<"$response"
}

ledger_before="$(latest_ledger)"
ledger_sequence="${ledger_before%%$'\t'*}"
ledger_hash="${ledger_before#*$'\t'}"

record_result() {
  local method="$1"
  shift
  local output
  output="$(capture_logged "${READ_BASE[@]}" "$method" "$@")"
  python3 -c '
import json, sys
print(json.dumps({"method": sys.argv[1], "arguments": sys.argv[2:], "result": sys.stdin.read()}))
' "$method" "$@" <<<"$output" >> "$rows_file"
}

for method in "${CORE_METHODS[@]}"; do
  record_result "$method"
done

if [[ -n "$RECORDS" ]]; then
  while IFS=$'\t' read -r kind first second extra || [[ -n "${kind:-}" ]]; do
    [[ -z "${kind:-}" || "$kind" == \#* ]] && continue
    if [[ -n "${extra:-}" || -z "${first:-}" ]]; then
      printf 'Invalid TSV record: %s %s %s %s\n' "$kind" "$first" "${second:-}" "${extra:-}" >&2
      exit 2
    fi
    case "$kind" in
      balance)
        [[ -n "${second:-}" ]] || { printf 'balance requires CREATOR and WALLET\n' >&2; exit 2; }
        record_result get_key_balance --creator "$first" --wallet "$second"
        ;;
      payment_reserve)
        [[ -n "${second:-}" ]] && { printf 'payment_reserve takes one TOKEN\n' >&2; exit 2; }
        record_result get_payment_asset_reserve --token "$first"
        ;;
      escape_reserve)
        [[ -n "${second:-}" ]] && { printf 'escape_reserve takes one KEY_ID\n' >&2; exit 2; }
        record_result get_escape_reserve --key_id "$first"
        ;;
      claimable_dividend)
        [[ -n "${second:-}" ]] || { printf 'claimable_dividend requires CREATOR and WALLET\n' >&2; exit 2; }
        record_result get_claimable_dividend --creator "$first" --wallet "$second"
        ;;
      auction_pending)
        [[ -n "${second:-}" ]] && { printf 'auction_pending takes one CREATOR\n' >&2; exit 2; }
        record_result is_auction_pending --creator "$first"
        ;;
      vault_pending_rewards)
        [[ -n "${second:-}" ]] || { printf 'vault_pending_rewards requires CREATOR and WALLET\n' >&2; exit 2; }
        record_result get_vault_pending_rewards --creator "$first" --holder "$second"
        ;;
      *)
        printf 'Unsupported record type: %s\n' "$kind" >&2
        exit 2
        ;;
    esac
  done < "$RECORDS"
fi

ledger_after="$(latest_ledger)"
if [[ "$ledger_after" != "$ledger_before" ]]; then
  printf 'Ledger advanced during collection (%s -> %s); no snapshot was written. Retry.\n' \
    "$ledger_sequence" "${ledger_after%%$'\t'*}" >&2
  exit 1
fi

python3 - "$NETWORK" "$CONTRACT" "$ledger_sequence" "$ledger_hash" "$rows_file" > "$json_file" <<'PY'
import json
import sys
from datetime import datetime, timezone

network, contract, sequence, ledger_hash, rows_path = sys.argv[1:]
with open(rows_path, encoding="utf-8") as rows_file:
    observations = [json.loads(line) for line in rows_file if line.strip()]

json.dump({
    "network": network,
    "contract": contract,
    "ledger": {"sequence": int(sequence), "hash": ledger_hash},
    "captured_at": datetime.now(timezone.utc).isoformat(),
    "observations": observations,
}, sys.stdout, indent=2)
sys.stdout.write("\n")
PY
mv "$json_file" "$OUTPUT"
printf 'Snapshot written to %s at ledger %s (%s).\n' "$OUTPUT" "$ledger_sequence" "$ledger_hash"
