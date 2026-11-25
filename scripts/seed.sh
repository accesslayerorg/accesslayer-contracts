#!/usr/bin/env bash
# Populates the local environment with creators, key holders, trades, an
# escrow and a pending proposal. Requires scripts/dev-up.sh to have run.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ENV_FILE="$ROOT_DIR/.env.local"
if [[ ! -f "$ENV_FILE" ]]; then
  echo "error: $ENV_FILE not found. Run scripts/dev-up.sh first." >&2
  exit 1
fi
# shellcheck disable=SC1090
set -a; source "$ENV_FILE"; set +a

log() { printf '[seed] %s\n' "$*"; }

invoke() {
  # invoke <contract-id> <fn> [args...]
  local contract="$1"; shift
  local fn="$1"; shift
  stellar contract invoke \
    --id "$contract" \
    --source "$DEV_IDENTITY" \
    --network "$SOROBAN_NETWORK" \
    -- "$fn" "$@"
}

new_identity() {
  local name="$1"
  if ! stellar keys address "$name" >/dev/null 2>&1; then
    stellar keys generate "$name" --network "$SOROBAN_NETWORK" --fund >/dev/null
  fi
  stellar keys address "$name"
}

log "creating seed identities"
CREATOR_A="$(new_identity seed-creator-alice)"
CREATOR_B="$(new_identity seed-creator-bob)"
HOLDER_1="$(new_identity seed-holder-1)"
HOLDER_2="$(new_identity seed-holder-2)"
BUYER="$(new_identity seed-buyer)"
PROPOSER="$(new_identity seed-proposer)"

log "registering creators"
invoke "$CREATOR_KEYS_CONTRACT_ID" register --creator "$CREATOR_A" >/dev/null
invoke "$CREATOR_KEYS_CONTRACT_ID" register --creator "$CREATOR_B" >/dev/null

log "minting creator keys to holders"
for holder in "$HOLDER_1" "$HOLDER_2"; do
  invoke "$CREATOR_KEYS_CONTRACT_ID" buy --creator "$CREATOR_A" --buyer "$holder" --amount 3 >/dev/null
done
invoke "$CREATOR_KEYS_CONTRACT_ID" buy --creator "$CREATOR_B" --buyer "$BUYER" --amount 2 >/dev/null

log "executing trades"
invoke "$CREATOR_KEYS_CONTRACT_ID" sell --creator "$CREATOR_A" --seller "$HOLDER_1" --amount 1 >/dev/null
invoke "$CREATOR_KEYS_CONTRACT_ID" buy --creator "$CREATOR_A" --buyer "$BUYER" --amount 1 >/dev/null

log "registering referral chain"
invoke "$REFERRAL_CONTRACT_ID" register_referral --referrer "$CREATOR_A" --referee "$HOLDER_2" >/dev/null

log "opening escrow"
invoke "$CREATOR_KEYS_CONTRACT_ID" create_escrow --creator "$CREATOR_B" --depositor "$BUYER" --amount 1 >/dev/null

log "submitting pending proposal"
invoke "$CREATOR_KEYS_CONTRACT_ID" create_proposal --creator "$CREATOR_A" --proposer "$PROPOSER" >/dev/null

cat >> "$ENV_FILE" <<EOF
SEED_CREATOR_ALICE=$CREATOR_A
SEED_CREATOR_BOB=$CREATOR_B
SEED_HOLDER_1=$HOLDER_1
SEED_HOLDER_2=$HOLDER_2
SEED_BUYER=$BUYER
SEED_PROPOSER=$PROPOSER
EOF

log "seed complete"
