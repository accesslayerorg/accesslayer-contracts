# Testing Utilities

Shared test utilities for accesslayer-contracts providing mock contracts, test environment setup, and assertion helpers.

## Crate

`test-utils` - Shared utilities for all contract test suites.

## Setup

Add `test-utils` as a dev-dependency in your test files:

```toml
# Cargo.toml
[dependencies]
test-utils = { workspace = true, features = [...] }

[dev-dependencies]
test-utils = { workspace = true }
```

## MockContracts

### MockToken

A mock ERC20-like token for testing.

```rust
use test_utils::MockToken;

// Mint 1000 tokens to an address
test_utils::MockToken::mint(&env, &address, 1000);

// Transfer tokens
test_utils::MockToken::transfer(&env, &from, &to, 100);

// Check balance
let bal = test_utils::MockToken::balance(&env, &address);
```

### MockOracle

A mock price oracle for testing.

```rust
use test_utils::MockOracle;

// Set price for an asset
test_utils::MockOracle::set_price(&env, "ETH", 3000);

// Get current price
let price = test_utils::MockOracle::get_price(&env, "ETH");
```

### MockReceiver

A mock receiver for testing token transfers.

```rust
use test_utils::MockReceiver;

// Notify receiver of incoming tokens
test_utils::MockReceiver::receive(&env, &sender, 100, vec![0x12, 0x34]);
```

## Helpers

### advance_ledgers(env, n)

Advance the ledger timestamp by `n` seconds.

```rust
test_utils::advance_ledgers(&env, 3600); // 1 hour
```

### set_timestamp(env, ts)

Set the ledger timestamp to a specific value.

```rust
test_utils::set_timestamp(&env, 1_700_000_000);
```

### fund(env, account, amount)

Fund an account with mock tokens.

```rust
test_utils::fund(&env, &account, 1000);
```

## TestEnv Builder

A complete test environment with mock contracts and default configuration.

```rust
use test_utils::TestEnv;

let env = TestEnv::default();

// Access deployed contracts
let admin = env.admin;
let treasury = env.treasury;

// Advance ledger
env.advance_ledgers(3600);

// Set timestamp
env.set_timestamp(1_700_000_000);

// Fund account
env.fund(&some_account, 500);

// Assert events
env.assert_event(symbol_short!("Transfer"), &[0x12, 0x34]);
```

### TestEnv Methods

| Method | Description |
|--------|-------------|
| `advance_ledgers(n)` | Advance ledger by n seconds |
| `set_timestamp(ts)` | Set ledger timestamp |
| `fund(account, amount)` | Fund account with mock tokens |
| `assert_event(topic, data)` | Assert event was emitted |

## Assertion Helpers

### assert_event(env, topic, data)

Assert that an event with the given topic and data was emitted.

```rust
use soroban_sdk::symbol_short;

env.assert_event(
    symbol_short!("Transfer"),
    &[0x12, 0x34, 0x56, 0x78]
);
```

### assert_event_with_diff(env, topic, expected_data)

Assert event with readable diff on failure.

```rust
test_utils::assert_event_with_diff(&env, symbol_short!("Deposit"), &[0x01, 0x02, 0x03]);
```