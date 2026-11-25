.PHONY: fmt fmt-check clippy test check ci wasm dev-up dev-reset seed

## Code formatting targets
fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

## Linting target
clippy:
	cargo clippy --workspace --all-targets -- -D warnings

## Test target
test:
	cargo test --workspace

## Basic compilation check
check:
	cargo check --workspace

## Refresh the committed WASM test fixture used by the upgrade tests.
##
## The fixture is checked in so the test suite compiles in CI without a WASM
## toolchain. That is what makes it go stale: after any change to the contract's
## code, re-run this and commit the refreshed fixture alongside the code change.
## The upgrade tests assert against the real build, so a stale fixture silently
## tests yesterday's contract.
WASM_TARGET := wasm32v1-none

wasm:
	rustup target add $(WASM_TARGET)
	cargo build --release --target $(WASM_TARGET) -p creator-keys
	mkdir -p creator-keys/test_wasm
	cp target/$(WASM_TARGET)/release/creator_keys.wasm creator-keys/test_wasm/contract.wasm
	@echo "Refreshed test fixture - commit it with your contract changes."
	@echo "Note: creator-keys-factory/test_wasm/ is NOT refreshed here. Despite living"
	@echo "under the factory, that fixture stands in for the creator-keys logic the"
	@echo "factory deploys, not the factory itself. The factory tests only assert on"
	@echo "registry addresses and never invoke the deployed contract, so a small"
	@echo "placeholder is sufficient there."

## Local development sandbox: start the quickstart network with Soroban RPC,
## build and deploy contracts, and write addresses to .env.local.
dev-up:
	docker compose up -d --wait
	./scripts/dev-up.sh

## Seed the running local environment with creators, holders, trades,
## an escrow and a pending proposal.
seed:
	./scripts/seed.sh

## Wipe and rebuild the local development environment from a clean state.
dev-reset:
	./scripts/dev-reset.sh

## Full CI workflow: runs format check, lint, tests, and compilation
## This is the standard check sequence to run before pushing to a branch
ci: fmt-check clippy test check
	@echo "✓ All CI checks passed"

## Convenience alias for running format checks and fixes
fmt-fix:
	cargo fmt --all

## Run all checks in sequence (alias for ci)
all-checks: ci
