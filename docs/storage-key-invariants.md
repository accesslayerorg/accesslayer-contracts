# Storage Key Invariants

This document describes the storage model, key structure, and invariants that must be maintained across all contract operations in Access Layer contracts.

## Overview

Access Layer contracts use Soroban's persistent storage to maintain creator profiles, key balances, and protocol configuration. Understanding these storage invariants is critical for contributors working on contract logic.

## Storage Keys

Storage keys are defined in the `DataKey` enum in [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs):

```rust
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Creator(Address),
    FeeConfig,
    KeyPrice,
    KeyBalance(Address, Address),
    TreasuryAddress,
    AdminAddress,
    ProtocolFeeRecipient,
}
```

### Key Types

| Key                            | Type               | Value Type       | Purpose                                                    |
| ------------------------------ | ------------------ | ---------------- | ---------------------------------------------------------- |
| `Creator(Address)`             | Per-creator        | `CreatorProfile` | Stores creator registration data, supply, and holder count |
| `FeeConfig`                    | Global             | `FeeConfig`      | Stores protocol-wide fee split configuration               |
| `KeyPrice`                     | Global             | `i128`           | Stores the fixed price for all keys across all creators    |
| `KeyBalance(Address, Address)` | Per-creator-holder | `u32`            | Stores how many keys a holder owns for a specific creator  |
| `TreasuryAddress`              | Global             | `Address`        | Protocol treasury address for fee routing                  |
| `AdminAddress`                 | Global             | `Address`        | Protocol admin address for configuration                   |
| `ProtocolFeeRecipient`         | Global             | `Address`        | Protocol fee recipient address                             |

## Storage Invariants

These invariants must hold true after every contract operation:

### 1. Creator Profile Invariants

**Invariant**: A creator exists if and only if `DataKey::Creator(address)` is present in storage.

**Implications**:

- `register_creator` must set this key
- `get_creator` must check for key presence
- Buy/sell operations must verify creator exists before proceeding

**Invariant**: For any registered creator, `profile.supply == sum of all KeyBalance(creator, *)`.

**Implications**:

- When buying a key, increment both `profile.supply` and `KeyBalance(creator, buyer)`
- When selling a key, decrement both `profile.supply` and `KeyBalance(creator, seller)`
- Never modify supply without updating the corresponding balance

**Invariant**: For any registered creator, `profile.holder_count == count of non-zero KeyBalance(creator, *)`.

**Implications**:

- When a holder's balance goes from 0 to 1, increment `holder_count`
- When a holder's balance goes from 1 to 0, decrement `holder_count`
- Never increment/decrement holder_count for balance changes that don't cross the zero boundary

**Example**:

```rust
// Buying first key for a holder
let current_balance = 0;
let new_balance = 1;
// Must increment holder_count because balance crossed from 0 to non-zero

// Buying second key for same holder
let current_balance = 1;
let new_balance = 2;
// Must NOT increment holder_count because balance was already non-zero
```

### 2. Key Balance Invariants

**Invariant**: `KeyBalance(creator, holder)` is only stored if the balance is non-zero.

**Implications**:

- When balance reaches 0, the key should be removed from storage (or left as 0)
- `get_key_balance` must return 0 for missing keys
- Storage reads use `.get().unwrap_or(0)` pattern

**Invariant**: Key balances are always non-negative (`u32` type enforces this).

**Implications**:

- Selling when balance is 0 must return `InsufficientBalance` error
- Use `checked_sub` to detect underflow attempts

### 3. Global Configuration Invariants

**Invariant**: `KeyPrice` must be positive when set.

**Implications**:

- `set_key_price` must reject zero or negative values
- Quote operations must check if price is set before computing

**Invariant**: `FeeConfig` must satisfy `creator_bps + protocol_bps == 10000` and `protocol_bps <= 5000`.

**Implications**:

- `set_fee_config` must validate before storing
- Fee computation can assume valid config if present
- See [docs/fee-assumptions.md](./fee-assumptions.md) for details

**Invariant**: Global config keys are optional until explicitly set by admin.

**Implications**:

- Operations requiring config must check for presence and return appropriate errors
- Read-only views should return `None` or default values for unset config

### 4. Supply and Balance Conservation

**Invariant**: Total supply across all creators equals the sum of all key balances.

**Mathematical representation**:

```
∀ creator: creator.supply = Σ(KeyBalance(creator, holder)) for all holders
```

**Implications**:

- Supply and balances must be updated atomically in the same transaction
- Never update supply without updating a corresponding balance
- Use checked arithmetic to prevent overflow

**Invariant**: Holder count equals the number of holders with non-zero balance.

**Mathematical representation**:

```
∀ creator: creator.holder_count = count({holder | KeyBalance(creator, holder) > 0})
```

**Implications**:

- Holder count must be updated when balance crosses zero boundary
- Holder count should never exceed supply (each holder has at least 1 key)

## Storage Access Patterns

### Reading Creator Profiles

**Helper function**: `read_creator_profile(env, creator) -> Option<CreatorProfile>`

**Usage**:

```rust
// For operations that tolerate missing creators
let profile = read_creator_profile(&env, &creator);
if profile.is_none() {
    // Handle unregistered creator
}

// For operations that require a creator
let profile = read_registered_creator_profile(&env, &creator)?;
```

**Invariant**: Always use helper functions instead of direct storage access to maintain consistency.

### Reading Key Balances

**Helper function**: `read_key_balance(env, creator) -> u32`

**Usage**:

```rust
// Returns 0 for unregistered creators
let supply = read_key_balance(&env, &creator);

// For holder balances
let balance_key = constants::storage::key_balance(&creator, &holder);
let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
```

**Invariant**: Always use `.unwrap_or(0)` when reading balances to handle missing keys.

### Writing Storage

**Pattern**: Always update related storage keys atomically.

**Example** (from `buy_key`):

```rust
// 1. Update holder balance
let new_balance = current_balance.checked_add(1)?;
env.storage().persistent().set(&balance_key, &new_balance);

// 2. Update holder count if needed
if current_balance == 0 {
    profile.holder_count = profile.holder_count.checked_add(1)?;
}

// 3. Update supply
profile.supply = profile.supply.checked_add(1)?;

// 4. Write updated profile
env.storage().persistent().set(&creator_key, &profile);
```

**Invariant**: Never return early between related storage updates. Use `?` operator carefully.

## Storage Consistency Checks

When modifying contract logic, verify these consistency checks:

### Before Committing Code

1. **Supply matches balances**: After buy/sell, verify `profile.supply` was updated correctly
2. **Holder count matches non-zero balances**: Verify holder_count increments/decrements only when crossing zero
3. **No orphaned balances**: Ensure balance updates always have corresponding supply updates
4. **Overflow protection**: Use `checked_add`, `checked_sub` for all arithmetic

### In Tests

Add assertions to verify invariants:

```rust
#[test]
fn test_buy_key_maintains_invariants() {
    // ... setup and buy operation ...

    let profile = client.get_creator(&creator).unwrap();
    let buyer_balance = client.get_key_balance(&creator, &buyer);

    // Verify supply invariant
    assert_eq!(profile.supply, buyer_balance, "supply must equal sum of balances");

    // Verify holder count invariant
    assert_eq!(profile.holder_count, 1, "holder_count must be 1 for single holder");
}
```

## Common Pitfalls

### 1. Forgetting to Update Holder Count

**Wrong**:

```rust
// Buying first key
profile.supply += 1;
// Forgot to increment holder_count!
```

**Right**:

```rust
// Buying first key
if current_balance == 0 {
    profile.holder_count = profile.holder_count.checked_add(1)?;
}
profile.supply = profile.supply.checked_add(1)?;
```

### 2. Updating Supply Without Balance

**Wrong**:

```rust
// Only updating supply
profile.supply += 1;
env.storage().persistent().set(&creator_key, &profile);
// Forgot to update KeyBalance!
```

**Right**:

```rust
// Update balance first
let new_balance = current_balance.checked_add(1)?;
env.storage().persistent().set(&balance_key, &new_balance);

// Then update supply
profile.supply = profile.supply.checked_add(1)?;
env.storage().persistent().set(&creator_key, &profile);
```

### 3. Not Using Checked Arithmetic

**Wrong**:

```rust
profile.supply = profile.supply + 1; // Can panic on overflow
```

**Right**:

```rust
profile.supply = profile.supply.checked_add(1).ok_or(ContractError::Overflow)?;
```

### 4. Returning Early Between Updates

**Wrong**:

```rust
// Update balance
env.storage().persistent().set(&balance_key, &new_balance);

// Some operation that might fail
some_operation()?; // If this fails, balance is updated but supply is not!

// Update supply
profile.supply += 1;
```

**Right**:

```rust
// Validate everything first
some_operation()?;

// Then update all storage atomically
env.storage().persistent().set(&balance_key, &new_balance);
profile.supply = profile.supply.checked_add(1)?;
env.storage().persistent().set(&creator_key, &profile);
```

## Storage Migration

If storage structure changes in a future version:

1. **Document the migration**: Describe old and new formats
2. **Provide migration function**: Write a function to convert old data to new format
3. **Test migration**: Verify all invariants hold after migration
4. **Version the storage**: Consider adding a version field to detect old data

## Debugging Storage Issues

### Viewing Storage in Tests

```rust
#[test]
fn debug_storage_state() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);

    // ... perform operations ...

    // Read storage directly
    let profile: Option<CreatorProfile> = env.as_contract(&contract_id, || {
        env.storage().persistent().get(&DataKey::Creator(creator.clone()))
    });

    println!("Profile: {:?}", profile);
}
```

### Common Storage Errors

| Error                 | Likely Cause                             | Solution                               |
| --------------------- | ---------------------------------------- | -------------------------------------- |
| `NotRegistered`       | Creator key not in storage               | Call `register_creator` first          |
| `Overflow`            | Supply or holder_count exceeded u32::MAX | Check arithmetic operations            |
| `InsufficientBalance` | KeyBalance is 0 or missing               | Verify holder owns keys before selling |

## Questions

For questions about storage invariants or debugging storage issues:

1. Review this document and [docs/error-codes.md](./error-codes.md)
2. Check existing tests for examples of correct storage usage
3. Ask in pull request comments or discussions
