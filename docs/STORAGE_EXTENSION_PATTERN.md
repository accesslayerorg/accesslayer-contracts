# Safe Storage Extension Pattern for Deployed Contracts

When adding new storage fields to an already-deployed contract, care must be taken to avoid breaking existing state reads. This document describes the recommended patterns for extending storage safely.

## The Problem

Once a contract is deployed and has accumulated state on-chain, adding new storage fields naively can cause data corruption:

1. **Direct field reordering breaks struct deserialization**: If you add a new field in the middle of a struct, existing stored values will deserialize incorrectly because the binary layout has changed.
2. **New optional fields in middle positions**: Adding a non-optional field where an optional field was expected causes type mismatch errors.
3. **Storage key collisions**: Reusing storage keys for different purposes corrupts both old and new data.

## Safe Patterns

### Pattern 1: Optional Fields (Preferred for Simple Additions)

Wrap new fields in `Option<T>`. Existing stored values will deserialize as `None`, allowing the contract to handle the absence gracefully.

**Safe because**: `Option<T>` adds no breaking changes to existing binary layouts. Old values deserialize to `None` automatically.

**Example**:

```rust
#[contracttype]
#[derive(Clone)]
pub struct Account {
    pub balance: i128,
    pub last_activity: u64,        // existing field
    pub reputation_score: Option<u32>, // new field, wrapped in Option
}

// Reading an old Account without reputation_score:
// The new field will be None
let account: Account = env.storage().persistent().get(&key).unwrap();
if let Some(score) = account.reputation_score {
    // Handle accounts with reputation
} else {
    // Handle accounts created before reputation was added
}
```

### Pattern 2: Versioned Storage Keys (For Complex Changes)

Instead of modifying existing structs, maintain separate storage keys for different versions of the same logical data.

**Safe because**: Versioned keys isolate old and new data completely. The contract checks both versions and merges or migrates on-demand.

**Example**:

```rust
#[contracttype]
#[derive(Clone)]
pub enum DataKeyV1 {
    Account(Address),
}

#[contracttype]
#[derive(Clone)]
pub enum DataKeyV2 {
    AccountV2(Address), // new structure with additional fields
}

pub fn get_account(env: &Env, owner: &Address) -> Account {
    // Try the new key first
    if let Some(account_v2) = env.storage().persistent().get::<DataKeyV2, AccountV2>(&DataKeyV2::AccountV2(owner.clone())) {
        return account_v2.to_v1(); // downgrade or use directly
    }
    
    // Fall back to the old key for existing accounts
    env.storage()
        .persistent()
        .get::<DataKeyV1, Account>(&DataKeyV1::Account(owner.clone()))
        .unwrap_or_default()
}
```

### Pattern 3: Lazy Migration on Read (For Structural Changes)

When deployed, keep the old storage layout. On the first read-write cycle, upgrade the data to the new layout and store it under the new key.

**Safe because**: Old data is never modified until explicitly accessed and upgraded. Parallel reads/writes remain safe because the upgrade is transactional per key.

**Example**:

```rust
pub fn get_account_safe(env: &Env, owner: &Address) -> NewAccount {
    // Try reading new layout
    if let Some(new) = env.storage().persistent().get::<_, NewAccount>(&new_key(owner)) {
        return new;
    }
    
    // Fall back to old layout and upgrade
    if let Some(old) = env.storage().persistent().get::<_, OldAccount>(&old_key(owner)) {
        let upgraded = OldAccount::upgrade_to_new(old);
        // Store under new key for next time
        env.storage().persistent().set(&new_key(owner), &upgraded);
        return upgraded;
    }
    
    NewAccount::default()
}
```

## Unsafe Patterns to Avoid

### ❌ Modifying Struct Field Order

```rust
// UNSAFE: This breaks deserialization of existing stored values
#[contracttype]
#[derive(Clone)]
pub struct Old {
    pub balance: i128,
    pub name: String,
}

#[contracttype]
#[derive(Clone)]
pub struct New {
    pub name: String,    // moved; existing bytes now decode incorrectly
    pub balance: i128,
    pub reputation: u32, // new field
}
```

Existing bytes stored under the old layout will decode with swapped values.

### ❌ Removing Fields Without Fallback

```rust
// UNSAFE: Removing a field breaks deserialization of old data
#[contracttype]
#[derive(Clone)]
pub struct Old {
    pub balance: i128,
    pub legacy_flag: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct New {
    pub balance: i128,
    // legacy_flag removed
}
```

Existing stored values include `legacy_flag`; the decoder expects it and panics if absent.

### ❌ Reusing the Same Storage Key for Different Data

```rust
// UNSAFE: Overwrites old data with incompatible new data
const OLD_KEY: &str = "account";
const NEW_KEY: &str = "account"; // same key!

// Old data:
env.storage().persistent().set(&OLD_KEY, &old_account);

// New code:
env.storage().persistent().set(&NEW_KEY, &new_account); // overwrites old

// Old reads now get corrupted new data
```

## Checklist for Safe Storage Extensions

Before deploying a storage change to a live contract:

- [ ] Will existing serialized values deserialize under the new struct layout?
- [ ] Are new fields wrapped in `Option` or behind versioned keys?
- [ ] Are storage keys unique between old and new layouts?
- [ ] Have I verified deserialization with a test using stale binary data?
- [ ] If removing or reordering fields, is there a migration plan (versioned key or lazy upgrade)?
- [ ] Can old and new code coexist during rollout?

## References

- Soroban SDK: https://soroban.stellar.org/docs
- Versioned Storage Pattern: Commonly used in database migrations
