#!/usr/bin/env bash
# Wipes the local environment and rebuilds it from scratch.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

log() { printf '[dev-reset] %s\n' "$*"; }

log "stopping and removing quickstart container and volumes"
docker compose down --volumes --remove-orphans || true

log "removing generated environment file"
rm -f "$ROOT_DIR/.env.local"

log "removing local stellar CLI identities and network config"
for name in access-layer-dev seed-creator-alice seed-creator-bob seed-holder-1 seed-holder-2 seed-buyer seed-proposer; do
  stellar keys rm "$name" >/dev/null 2>&1 || true
done
stellar network rm local >/dev/null 2>&1 || true

log "removing contract build artifacts"
rm -rf "$ROOT_DIR/target/wasm32v1-none"

log "bringing environment back up"
"$ROOT_DIR/scripts/dev-up.sh"

log "reseeding"
"$ROOT_DIR/scripts/seed.sh"

log "reset complete"
