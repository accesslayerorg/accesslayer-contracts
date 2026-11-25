.PHONY: fmt fmt-check clippy test check ci wasm simulate simulate-nightly

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

## Economic attack simulation harness (sandwich, wash trading, reward farming,
## governance buy-and-vote, drain-the-reserve). Runs randomized multi-agent
## sequences with a fixed, printed seed and fails if any adversary profits
## beyond the documented tolerance or reserve solvency is violated.
##
## Override SIM_SEED to reproduce a specific run, and SIM_ITERATIONS to scale
## the number of randomized sequences (the nightly job uses a larger count).
SIM_SEED ?= 42
SIM_ITERATIONS ?= 200
SIM_REPORT ?= target/simulation/report.md

simulate:
	mkdir -p $(dir $(SIM_REPORT))
	SIM_SEED=$(SIM_SEED) SIM_ITERATIONS=$(SIM_ITERATIONS) SIM_REPORT=$(SIM_REPORT) \
	cargo test --workspace simulation -- --nocapture
	@echo "Simulation report written to $(SIM_REPORT)"

## Larger iteration count for the nightly CI job.
simulate-nightly:
	$(MAKE) simulate SIM_ITERATIONS=10000 SIM_SEED=$${SIM_SEED:-$$(date +%Y%m%d)}

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

## Full CI workflow: runs format check, lint, tests, and compilation
## This is the standard check sequence to run before pushing to a branch
ci: fmt-check clippy test check
	@echo "✓ All CI checks passed"

## Convenience alias for running format checks and fixes
fmt-fix:
	cargo fmt --all

## Run all checks in sequence (alias for ci)
all-checks: ci
