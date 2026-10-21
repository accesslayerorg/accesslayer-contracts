# Test Fix Summary

## Issue Identified

The initial tests had incorrect expectations about how `sell_key` behaves:

### Problem
- `sell_key` sells **ONE key at a time** (not a batch)
- Original tests expected the **first** sell to fail when holder has 4 liquid keys
- This was incorrect - the first 4 sells should succeed, and the 5th should fail

### Root Cause of Test Failures

**Test 1: `test_sell_reverts_when_attempting_to_use_staked_keys`**
- Expected: First `sell_key` call to fail with `InsufficientBalance`
- Actual: First `sell_key` call succeeded (returned `Ok(9)` for new supply)
- **Why**: With 4 liquid keys available, selling 1 key should succeed

**Test 2: `test_staked_balance_unchanged_after_sell_attempts`**
- Expected: First `sell_key` to fail, then 4 more to succeed
- Actual: Tried to sell 5 keys total, which exceeded liquid balance
- **Why**: The test logic was backwards

## Solution Applied

### Fixed Test 1: `test_sell_reverts_when_attempting_to_use_staked_keys`

**Before:**
```rust
// Just tried to sell once and expected it to fail
let result = client.try_sell_key(&creator, &holder, &None);
assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
```

**After:**
```rust
// Sell 4 liquid keys successfully (one at a time)
for _ in 0..4 {
    let result = client.try_sell_key(&creator, &holder, &None);
    assert!(result.is_ok(), "Selling within liquid balance should succeed");
}

// Attempt to sell 5th key - should fail because only 4 were liquid
let result = client.try_sell_key(&creator, &holder, &None);
assert_eq!(
    result,
    Err(Ok(ContractError::InsufficientBalance)),
    "Selling more than liquid balance should fail"
);
```

### Fixed Test 2: `test_staked_balance_unchanged_after_sell_attempts`

**Before:**
```rust
let _ = client.try_sell_key(&creator, &holder, &None);  // Unclear intent
assert_eq!(client.get_staked_balance(&creator, &holder), 6);

for _ in 0..4 {
    client.sell_key(&creator, &holder, &None);  // Would fail on 5th total
}
```

**After:**
```rust
// Successfully sell 4 keys (one at a time)
for _ in 0..4 {
    client.sell_key(&creator, &holder, &None);
}

// Verify staked balance unchanged after successful sells
assert_eq!(client.get_staked_balance(&creator, &holder), 6);
assert_eq!(client.get_liquid_balance(&creator, &holder), 0);

// Attempt to sell when no liquid balance remains (should fail)
let result = client.try_sell_key(&creator, &holder, &None);
assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
```

## Test Behavior Now Correctly Verifies

### Scenario: 10 total keys, 6 staked, 4 liquid

| Action | Liquid Before | Expected Result | Liquid After | Staked |
|--------|--------------|-----------------|--------------|--------|
| Sell #1 | 4 | ✅ Success | 3 | 6 |
| Sell #2 | 3 | ✅ Success | 2 | 6 |
| Sell #3 | 2 | ✅ Success | 1 | 6 |
| Sell #4 | 1 | ✅ Success | 0 | 6 |
| Sell #5 | 0 | ❌ Fail (InsufficientBalance) | 0 | 6 |

## Acceptance Criteria Verification

✅ **Sell of 5 (total) reverts when only 4 liquid keys available**
- First 4 sells succeed
- 5th sell fails with `InsufficientBalance`

✅ **Sell of 4 succeeds using only liquid balance**
- All 4 liquid keys can be sold one at a time
- Staked keys remain untouched

✅ **Staked balance unchanged after both attempts**
- Remains at 6 after successful sells
- Remains at 6 after failed sell attempt
- Liquid balance correctly reaches 0 after 4 sells

## Implementation Correctness

The `sell_key` implementation is **correct**:

```rust
// Check liquid balance (total balance - staked balance)
let staked_balance_key = constants::storage::staked_balance(&creator, &seller);
let staked_balance: u32 = env
    .storage()
    .persistent()
    .get(&staked_balance_key)
    .unwrap_or(0);
let liquid_balance = current_balance.saturating_sub(staked_balance);

if liquid_balance == 0 {
    return Err(ContractError::InsufficientBalance);
}
```

This properly:
1. Calculates liquid balance as `total - staked`
2. Rejects sells when liquid balance is 0
3. Allows sells when liquid balance > 0 (for the single key being sold)

## Commit

```
18d975b fix: correct test expectations for sell_key liquid balance validation
```

## Summary

The implementation was correct all along. The tests had incorrect expectations about the behavior of `sell_key` which sells one key per call, not a batch. Tests now correctly verify that:

1. Multiple sells within liquid balance succeed
2. Sells beyond liquid balance fail
3. Staked balance remains unchanged throughout
