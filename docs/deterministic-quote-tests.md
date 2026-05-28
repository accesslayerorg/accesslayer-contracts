# Deterministic Quote Tests - Contributor Guide

This document provides guidance for contributors writing tests for quote operations (buy and sell quotes) in Access Layer contracts.

## Overview

Quote tests verify that the contract returns consistent, predictable pricing and fee information. These tests are critical for ensuring users see accurate prices and that the pricing model behaves correctly under various conditions.

## Why Deterministic Tests Matter

**Determinism** means a test produces the same result every time it runs, regardless of:

- Test execution order
- System time or random values
- Previous test state

For quote operations, determinism ensures:

- Users see consistent prices for the same market conditions
- Fee calculations are reproducible
- Pricing doesn't drift due to rounding errors
- Integration tests catch regressions reliably

## Quote Test Structure

### Basic Test Template

```rust
#[test]
fn test_quote_behavior_description() {
    // 1. Setup: Create test environment
    let env = test_env_with_auths();
    let (client, creator) = setup_with_fees(&env, price);

    // 2. Execute: Get quote or perform operation
    let quote = client.get_buy_quote(&creator);

    // 3. Assert: Verify expected behavior
    assert_eq!(quote.price, expected_price);
    assert_eq!(quote.total_amount, expected_total);
}
```

### Test Naming Convention

Use descriptive names that explain what is being tested:

**Good names**:

- `test_buy_quote_is_identical_across_consecutive_calls`
- `test_buy_quote_monotonic_with_zero_protocol_fee`
- `test_buy_quote_stable_across_50_sequential_purchases`

**Bad names**:

- `test_quote` (too vague)
- `test_1` (meaningless)
- `test_buy` (unclear what aspect is tested)

## Fixed Price Model

Access Layer uses a **fixed price model**:

- **Global price**: All creators share the same key price (stored in `KeyPrice` storage key)
- **Fixed per key**: Each key costs the same amount, regardless of supply
- **Fees on top**: Fees are calculated as a percentage of the price and added to the total

### Implications for Tests

1. **Price stability**: The price should never change unless an admin explicitly calls `set_key_price`
2. **Quote consistency**: Multiple calls to `get_buy_quote` should return identical results
3. **Supply independence**: Buying or selling keys should not affect the quote price
4. **Creator independence**: All creators have the same price (but independent supplies)

### Sell Quote Monotonicity (Incremental Sells)

For the fixed price model, sell quotes are expected to be stable across incremental sells until the holder runs out of keys:

- Calling `get_sell_quote` repeatedly for the same `(creator, holder)` should return the same `QuoteResponse` as long as `get_key_balance(creator, holder) > 0`.
- Once the holder balance reaches `0`, `get_sell_quote` should reject with `ContractError::InsufficientBalance`.
- If fee configuration and price would imply a negative sell payout (fees exceed price), `get_sell_quote` should reject with `ContractError::SellUnderflow` rather than returning an invalid negative amount.

## Fee Configuration Rules

Understanding fee rules is essential for writing correct tests:

### Fee Validation Rules

From [`fee::validate_fee_bps`](../creator-keys/src/lib.rs):

1. **Sum must equal 10,000**: `creator_bps + protocol_bps == 10000` (representing 100%)
2. **Protocol max is 50%**: `protocol_bps <= 5000` (5,000 basis points)

### Valid Fee Configurations

| creator_bps | protocol_bps | Valid? | Notes                      |
| ----------- | ------------ | ------ | -------------------------- |
| 9000        | 1000         | ✅     | 90% creator, 10% protocol  |
| 5000        | 5000         | ✅     | 50/50 split (max protocol) |
| 10000       | 0            | ✅     | 100% creator, 0% protocol  |
| 0           | 10000        | ❌     | Protocol exceeds 50% max   |
| 9000        | 0            | ❌     | Sum is 9000, not 10000     |
| 8000        | 2000         | ✅     | 80% creator, 20% protocol  |

### Testing Edge Cases

Always test fee edge cases:

```rust
#[test]
fn test_buy_quote_with_max_protocol_fee() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    // 50/50 split is the maximum allowed protocol fee
    set_pricing_and_fees(&env, &client, 1000, 5000, 5000);
    let creator = register_test_creator(&env, &client, "alice");

    let quote = client.get_buy_quote(&creator);
    assert_eq!(quote.creator_fee, quote.protocol_fee, "fees should be equal");
}

#[test]
fn test_buy_quote_with_zero_protocol_fee() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    // 100% creator, 0% protocol
    set_pricing_and_fees(&env, &client, 1000, 10000, 0);
    let creator = register_test_creator(&env, &client, "bob");

    let quote = client.get_buy_quote(&creator);
    assert_eq!(quote.protocol_fee, 0, "protocol fee should be zero");
}
```

## Quote Response Structure

The `QuoteResponse` struct contains:

```rust
pub struct QuoteResponse {
    pub price: i128,           // Base key price
    pub creator_fee: i128,     // Fee going to creator
    pub protocol_fee: i128,    // Fee going to protocol
    pub total_amount: i128,    // Total cost (buy) or net proceeds (sell)
}
```

### Buy Quote Formula

```
total_amount = price + creator_fee + protocol_fee
```

Where:

- `creator_fee = floor(price * creator_bps / 10000)`
- `protocol_fee = floor(price * protocol_bps / 10000)`
- Remainder from rounding goes to creator

### Sell Quote Formula

```
total_amount = price - creator_fee - protocol_fee
```

Seller receives `total_amount` (price minus fees).

## Common Test Patterns

### Pattern 1: Quote Stability After Operations

Verify that quotes remain unchanged after buy/sell operations:

```rust
#[test]
fn test_buy_quote_unchanged_after_purchase() {
    let env = test_env_with_auths();
    let (client, creator) = setup_with_fees(&env, 1000);

    let quote_before = client.get_buy_quote(&creator);

    // Perform a buy operation
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &quote_before.total_amount);

    let quote_after = client.get_buy_quote(&creator);

    // Quote should be identical
    assert_eq!(quote_before.price, quote_after.price);
    assert_eq!(quote_before.total_amount, quote_after.total_amount);
}
```

### Pattern 2: Fee Conservation

Verify that fees sum correctly:

```rust
#[test]
fn test_buy_quote_fees_sum_correctly() {
    let env = test_env_with_auths();
    let (client, creator) = setup_with_fees(&env, 1000);

    let quote = client.get_buy_quote(&creator);

    // For buy quotes: total = price + fees
    assert_eq!(
        quote.total_amount,
        quote.price + quote.creator_fee + quote.protocol_fee,
        "total must equal price plus all fees"
    );

    // Fees must be non-negative
    assert!(quote.creator_fee >= 0);
    assert!(quote.protocol_fee >= 0);
}
```

### Pattern 3: Monotonicity Across Volume

Verify that quotes remain stable across multiple purchases:

```rust
#[test]
fn test_buy_quote_stable_across_multiple_purchases() {
    let env = test_env_with_auths();
    let (client, creator) = setup_with_fees(&env, 500);

    let initial_quote = client.get_buy_quote(&creator);

    // Perform multiple purchases
    for i in 0..10 {
        let buyer = Address::generate(&env);
        client.buy_key(&creator, &buyer, &initial_quote.total_amount);

        let current_quote = client.get_buy_quote(&creator);
        assert_eq!(
            current_quote.price,
            initial_quote.price,
            "price must remain constant after {} purchases",
            i + 1
        );
    }
}
```

### Pattern 4: Multi-Creator Independence

Verify that creators maintain independent state despite sharing global price:

```rust
#[test]
fn test_quotes_independent_across_creators() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let price = 1000;
    set_pricing_and_fees(&env, &client, price, 8000, 2000);

    let creator_alice = register_test_creator(&env, &client, "alice");
    let creator_bob = register_test_creator(&env, &client, "bob");

    // Both should have same quote (global price)
    let quote_alice = client.get_buy_quote(&creator_alice);
    let quote_bob = client.get_buy_quote(&creator_bob);
    assert_eq!(quote_alice.price, quote_bob.price);

    // Buy from alice
    let buyer = Address::generate(&env);
    client.buy_key(&creator_alice, &buyer, &quote_alice.total_amount);

    // Bob's quote should be unchanged
    let quote_bob_after = client.get_buy_quote(&creator_bob);
    assert_eq!(quote_bob.price, quote_bob_after.price);

    // But supplies should be independent
    assert_eq!(client.get_total_key_supply(&creator_alice), 1);
    assert_eq!(client.get_total_key_supply(&creator_bob), 0);
}
```

## Using Test Helpers

The `contract_test_env` module provides helpers to reduce boilerplate:

### Available Helpers

```rust
// Create test environment with mocked auth
let env = test_env_with_auths();

// Override the default deterministic timestamp when a test needs a specific moment
set_test_timestamp(&env, 1_700_000_123);

// Register contract and get client
let (client, contract_id) = register_creator_keys(&env);

// Set key price only
let admin = set_key_price_for_tests(&env, &client, 1000);

// Set fee config only
let admin = set_protocol_fee_bps(&env, &client, 9000, 1000);

// Set both price and fees (common pattern)
let admin = set_pricing_and_fees(&env, &client, 1000, 9000, 1000);

// Register a test creator
let creator = register_test_creator(&env, &client, "alice");
```

Use `set_test_timestamp` in tests that need a deterministic ledger time (for
example snapshot assertions or time-sensitive setup). A recommended default is
`DEFAULT_TEST_TIMESTAMP` (`1700000000`).

### Custom Setup Function

For quote tests, use this pattern:

```rust
fn setup_with_fees<'a>(env: &'a Env, price: i128) -> (CreatorKeysContractClient<'a>, Address) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, price, 9000, 1000);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}
```

## Payment Amount in Tests

**Critical**: When calling `buy_key`, you must provide the `total_amount`, not just the `price`:

```rust
// ❌ WRONG: Using price directly
let quote = client.get_buy_quote(&creator);
client.buy_key(&creator, &buyer, &quote.price); // Will fail with InsufficientPayment!

// ✅ CORRECT: Using total_amount
let quote = client.get_buy_quote(&creator);
client.buy_key(&creator, &buyer, &quote.total_amount); // Includes price + fees
```

## Test Snapshots

Quote tests generate snapshot files in `creator-keys/test_snapshots/`:

### What Are Snapshots?

Snapshots capture the complete test execution state, including:

- All contract calls and their parameters
- Storage changes
- Events emitted
- Return values

### When to Update Snapshots

Update snapshots when:

- You intentionally change contract behavior
- You add new tests
- Fee calculations change

**Do not** update snapshots to "fix" failing tests without understanding why they changed.

### Reviewing Snapshot Changes

When snapshots change in a PR:

1. Review the diff carefully
2. Verify the changes match your intended behavior
3. Explain the changes in the PR description

## Common Mistakes and Fixes

### Mistake 1: Invalid Fee Configuration

```rust
// ❌ WRONG: Fees don't sum to 10,000
set_pricing_and_fees(&env, &client, 1000, 9000, 0); // Sum is 9000

// ✅ CORRECT: Fees sum to 10,000
set_pricing_and_fees(&env, &client, 1000, 10000, 0); // Sum is 10,000
```

### Mistake 2: Protocol Fee Exceeds Maximum

```rust
// ❌ WRONG: Protocol fee exceeds 50%
set_pricing_and_fees(&env, &client, 1000, 0, 10000); // Protocol is 100%

// ✅ CORRECT: Protocol fee at maximum
set_pricing_and_fees(&env, &client, 1000, 5000, 5000); // Protocol is 50%
```

### Mistake 3: Using Price Instead of Total Amount

```rust
// ❌ WRONG: Insufficient payment
let quote = client.get_buy_quote(&creator);
client.buy_key(&creator, &buyer, &quote.price);

// ✅ CORRECT: Full payment including fees
let quote = client.get_buy_quote(&creator);
client.buy_key(&creator, &buyer, &quote.total_amount);
```

### Mistake 4: Expecting Different Prices Per Creator

```rust
// ❌ WRONG: Expecting different prices
set_pricing_and_fees(&env, &client, 500, 8000, 2000);
let creator_alice = register_test_creator(&env, &client, "alice");

set_pricing_and_fees(&env, &client, 1000, 8000, 2000); // Changes global price!
let creator_bob = register_test_creator(&env, &client, "bob");

// Both creators now have price 1000 (global price was updated)

// ✅ CORRECT: Understanding global price model
set_pricing_and_fees(&env, &client, 1000, 8000, 2000);
let creator_alice = register_test_creator(&env, &client, "alice");
let creator_bob = register_test_creator(&env, &client, "bob");

// Both creators share the same global price
```

## Testing Checklist

Before submitting a quote test PR, verify:

- [ ] Test name clearly describes what is being tested
- [ ] Fee configuration sums to 10,000 and protocol ≤ 5,000
- [ ] Using `total_amount` for `buy_key` calls, not `price`
- [ ] Assertions include descriptive failure messages
- [ ] Test is deterministic (no random values, no time dependencies)
- [ ] Test uses helpers from `contract_test_env`
- [ ] Test verifies the specific behavior mentioned in the name
- [ ] Snapshot files are reviewed and understood

## Example: Complete Quote Test

Here's a complete example demonstrating best practices:

```rust
#[test]
fn test_buy_quote_stable_across_50_sequential_purchases() {
    // Setup: Create environment with fixed price and fees
    let env = test_env_with_auths();
    let price = 750_i128;
    let (client, creator) = setup_with_fees(&env, price);

    // Get initial quote before any purchases
    let initial_quote = client.get_buy_quote(&creator);

    // Execute: Perform 50 sequential purchases
    for i in 0..50_u32 {
        let buyer = Address::generate(&env);
        client.buy_key(&creator, &buyer, &initial_quote.total_amount);

        // Assert: Verify quote remains stable after each purchase
        let current_quote = client.get_buy_quote(&creator);
        assert_eq!(
            current_quote.price,
            initial_quote.price,
            "price must remain constant after {} purchases",
            i + 1
        );
        assert_eq!(
            current_quote.total_amount,
            initial_quote.total_amount,
            "total_amount must remain constant after {} purchases",
            i + 1
        );
        assert_eq!(
            current_quote.creator_fee,
            initial_quote.creator_fee,
            "creator_fee must remain constant after {} purchases",
            i + 1
        );
        assert_eq!(
            current_quote.protocol_fee,
            initial_quote.protocol_fee,
            "protocol_fee must remain constant after {} purchases",
            i + 1
        );
    }
}
```

## Running Quote Tests

```bash
# Run all quote monotonicity tests
cargo test --test buy_quote_monotonicity

# Run a specific test
cargo test test_buy_quote_stable_across_50_sequential_purchases

# Run with output
cargo test test_buy_quote_stable_across_50_sequential_purchases -- --nocapture
```

## Questions

For questions about writing quote tests:

1. Review existing tests in `creator-keys/tests/buy_quote_monotonicity.rs`
2. Check [docs/fee-assumptions.md](./fee-assumptions.md) for fee calculation details
3. See [docs/error-codes.md](./error-codes.md) for error handling
4. Ask in pull request comments or discussions
