# CI Contract Checks

This document describes the continuous integration (CI) checks that run on every pull request and push to the main branch for Access Layer contracts.

## Overview

The CI pipeline ensures code quality, correctness, and consistency across the contract codebase. All checks must pass before a pull request can be merged.

## CI Workflow

The CI workflow is defined in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) and runs three primary checks:

### 1. Format Check

**Command**: `cargo fmt --all -- --check`

**Purpose**: Ensures all Rust code follows consistent formatting rules defined by `rustfmt`.

**What it checks**:

- Indentation and spacing
- Line length limits
- Import ordering
- Code structure formatting

**How to fix failures**:

```bash
# Auto-format all code
cargo fmt --all

# Or use the Makefile helper
make fmt
```

**Local verification**:

```bash
# Check formatting without modifying files
cargo fmt --all -- --check

# Or use the Makefile helper
make fmt-check
```

### 2. Clippy Lints

**Command**: `cargo clippy --workspace --all-targets -- -D warnings`

**Purpose**: Catches common mistakes, anti-patterns, and potential bugs using Rust's official linter.

**What it checks**:

- Unused variables and imports
- Inefficient code patterns
- Potential logic errors
- Type conversion issues
- Idiomatic Rust violations

**How to fix failures**:

```bash
# Run clippy and see warnings
cargo clippy --workspace --all-targets

# Or use the Makefile helper
make clippy
```

**Common issues**:

- Unused imports: Remove them or use `#[allow(unused_imports)]` if needed for conditional compilation
- Unnecessary clones: Clippy suggests when cloning can be avoided
- Complex boolean expressions: Simplify or add comments explaining the logic

### 3. Test Suite

**Command**: `cargo test --workspace`

**Purpose**: Runs all unit tests and integration tests to verify contract behavior.

**What it checks**:

- Unit tests in `src/` modules (marked with `#[test]`)
- Integration tests in `tests/` directory
- Doc tests in code comments
- Test snapshots for deterministic behavior

**How to fix failures**:

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific package
cargo test -p creator-keys

# Run a specific test file
cargo test --test buy_quote_monotonicity

# Run a specific test
cargo test test_buy_quote_is_identical_across_consecutive_calls

# Or use the Makefile helper
make test
```

**Test categories**:

- **Unit tests**: Test individual functions and modules in isolation
- **Integration tests**: Test contract behavior end-to-end with mocked Soroban environment
- **Snapshot tests**: Verify deterministic output by comparing against stored snapshots

## Running All Checks Locally

Before pushing code, run all CI checks locally:

```bash
# Run all checks individually
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Or use the Makefile to run all checks
make verify
```

## CI Environment

**Platform**: Ubuntu Latest (GitHub Actions runner)

**Rust toolchain**: Stable channel (specified in `rust-toolchain.toml`)

**Components**: `rustfmt`, `clippy` (installed automatically)

## Troubleshooting CI Failures

### Format Check Failures

**Symptom**: CI shows formatting differences

**Solution**:

1. Run `cargo fmt --all` locally
2. Commit the formatting changes
3. Push the updated code

### Clippy Failures

**Symptom**: CI shows clippy warnings treated as errors

**Solution**:

1. Run `cargo clippy --workspace --all-targets` locally
2. Fix the reported issues
3. If a warning is intentional, add `#[allow(clippy::lint_name)]` with a comment explaining why
4. Commit and push the fixes

**Note**: Never use `#[allow(clippy::all)]` or disable all warnings. Address each warning individually.

### Test Failures

**Symptom**: One or more tests fail in CI

**Solution**:

1. Run the failing test locally: `cargo test <test_name>`
2. Check the test output for assertion failures or panics
3. Review recent changes that might have affected the test
4. Fix the code or update the test if behavior intentionally changed
5. For snapshot tests, regenerate snapshots if the new behavior is correct
6. Commit and push the fixes

**Common causes**:

- Logic errors in contract code
- Incorrect test assertions
- Missing test setup (e.g., fee config not set)
- Outdated test snapshots

## Adding New Tests

When adding new tests, ensure they:

1. **Are deterministic**: Tests should produce the same result every run
2. **Are isolated**: Tests should not depend on execution order
3. **Have clear names**: Use descriptive test function names like `test_buy_quote_monotonic_with_zero_protocol_fee`
4. **Include assertions**: Every test should verify expected behavior with `assert!`, `assert_eq!`, or `assert_ne!`
5. **Use test helpers**: Import from `contract_test_env` to reduce boilerplate

See [docs/deterministic-quote-tests.md](./deterministic-quote-tests.md) for guidance on writing quote tests.

## CI Performance

**Typical run time**: 2-3 minutes for all checks

**Optimization tips**:

- CI uses caching for Rust dependencies
- Tests run in parallel by default
- Format and clippy checks are fast (<30 seconds each)

## CI Badge

The CI status badge is displayed in the README:

```markdown
![CI Status](https://github.com/your-org/accesslayer-contracts/workflows/Contracts%20CI/badge.svg)
```

A green badge indicates all checks are passing.

## Future CI Enhancements

Potential additions to the CI pipeline:

- **Code coverage**: Track test coverage percentage
- **Security audits**: Run `cargo audit` to check for vulnerable dependencies
- **Benchmark tests**: Detect performance regressions
- **Contract size checks**: Ensure WASM output stays within size limits
- **Gas/instruction cost analysis**: Monitor computational costs

## Questions

If CI checks are failing and you're unsure how to fix them:

1. Read the CI log output carefully
2. Run the failing check locally to reproduce
3. Check recent commits for changes that might have caused the failure
4. Ask in pull request comments for help from maintainers
