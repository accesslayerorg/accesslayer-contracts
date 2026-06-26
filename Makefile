.PHONY: fmt fmt-check clippy test check ci

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

## Full CI workflow: runs format check, lint, tests, and compilation
## This is the standard check sequence to run before pushing to a branch
ci: fmt-check clippy test check
	@echo "✓ All CI checks passed"

## Convenience alias for running format checks and fixes
fmt-fix:
	cargo fmt --all

## Run all checks in sequence (alias for ci)
all-checks: ci

