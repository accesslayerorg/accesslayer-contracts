# Creator Metadata Versioning and Storage Evolution

This document describes how creator metadata (the `CreatorProfile` struct) can safely evolve and receive new fields without breaking existing state reads or causing deserialization failures.

For general storage extension patterns, see [Safe Storage Extension Pattern for Deployed Contracts](./STORAGE_EXTENSION_PATTERN.md).

## Overview

The `CreatorProfile` is stored at key `DataKey::Creator(address)` in the contract's persistent storage. Once deployed on-chain, existing serialized creator records must continue to deserialize correctly as the schema evolves.

```rust
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
    pub holder_count: u32,
    pub fee_recipient: Address,
    pub registered_at: u32,  // added as last field for safe extension
}
```

## Safe Extension Patterns

### Pattern 1: Append New Fields at the End (Recommended for Simple Additions)

**When to use**: Adding optional or default-valued metadata that does not affect core creator functionality.

**Why it works**: Soroban's persistent storage layer reads structs by field index. Appending a new field after existing fields preserves backward compatibility—existing records that lack the new field will deserialize successfully and the new field will take its default value.

**Example**: Adding a `metadata_version` field
```rust
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
    pub holder_count: u32,
    pub fee_recipient: Address,
    pub registered_at: u32,
    pub metadata_version: u32,  // ✅ Safe: appended at end with default value
}
```

**Implementation notes**:
- Ensure new fields have sensible default values (e.g., `u32` defaults to `0`, `bool` defaults to `false`).
- Document the field semantics clearly so future readers understand its purpose and initial state.
- Update all `register_creator` call sites to set the new field consistently.

### Pattern 2: Use `Option<T>` for Optional Metadata (For Gradual Rollout)

**When to use**: Adding metadata that is not applicable to all creators or should be retrofitted gradually.

**Why it works**: Wrapping new fields in `Option<T>` makes them explicitly nullable. Existing records deserialize as `None` without error.

**Example**: Adding creator badges or profile metadata
```rust
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
    pub holder_count: u32,
    pub fee_recipient: Address,
    pub registered_at: u32,
    pub profile_url: Option<String>,  // ✅ Safe: new creators can have this, old ones are None
}
```

**Implementation notes**:
- Place `Option<T>` fields at the end of the struct, after all non-optional fields.
- In read paths (e.g., `get_creator`), provide sensible defaults or error messaging if the field is not set.
- Document whether the field is immutable or can be updated post-registration.

### Pattern 3: Versioned Storage Keys (For Breaking Structural Changes)

**When to use**: Adding fields that fundamentally change the meaning of existing fields, or restructuring the profile layout entirely.

**Why it works**: Versioned keys isolate old and new data completely. The contract can handle both versions on read, maintaining full backward compatibility while still allowing structural evolution.

**Example**: If the `handle` field changes from a name-based string to a numeric ID:
```rust
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    CreatorV1(Address),  // legacy: handle is String
    CreatorV2(Address),  // new: handle is structured differently
}

pub fn get_creator_profile(env: &Env, creator: &Address) -> Result<CreatorProfile, ContractError> {
    // Try the new key first
    if let Some(profile_v2) = env.storage()
        .persistent()
        .get::<DataKey, CreatorProfileV2>(&DataKey::CreatorV2(creator.clone())) {
        return Ok(profile_v2.into_v1());  // downgrade or interpret as needed
    }
    
    // Fall back to the legacy key
    env.storage()
        .persistent()
        .get::<DataKey, CreatorProfile>(&DataKey::CreatorV1(creator.clone()))
        .ok_or(ContractError::NotRegistered)
}
```

**Implementation notes**:
- Use separate enum variants in `DataKey` (e.g., `CreatorV1`, `CreatorV2`).
- Implement conversion logic between versions (e.g., `into_v1()` or `from_v2_to_v1()`).
- Document the migration path in code comments.
- Migration can happen lazily (upgrade on first read) or eagerly (batch migration in admin function).

## Compatibility Rules

### ✅ Safe Changes

- **Appending new non-optional fields** with sensible defaults
- **Adding new `Option<T>` fields** at the end of the struct
- **Updating documentation** about field semantics
- **Adding validation** to existing field reads (as long as old values remain valid)

### ⚠️ Risky Changes (Requires Versioned Storage Keys)

- **Reordering existing fields** — breaks binary deserialization
- **Changing field types** (e.g., `String` → `u32`, `u32` → `i128`) — breaks deserialization
- **Removing fields** — old records cannot deserialize
- **Inserting fields in the middle** — shifts all downstream field offsets

### ❌ Unsafe Changes

- **Changing the meaning of an existing field** without a migration path
- **Storing different logical data** under the same `DataKey` — causes data corruption
- **Modifying existing field constraints** retroactively without migration

## Migration Considerations for Contributors

### When Adding a New Field

1. **Decide on the pattern**:
   - Is this a simple append with a default? Use **Pattern 1** (append at end).
   - Is this metadata that may not apply to all creators? Use **Pattern 2** (wrap in `Option<T>`).
   - Does this break the structure? Use **Pattern 3** (versioned keys).

2. **Update `register_creator`**:
   - Ensure the new field is set when creating a new profile.
   - Document what value new creators get.

3. **Update read paths**:
   - `get_creator` and any other read-only views must handle the new field gracefully.
   - Provide sensible defaults or clear error messages if a field is missing.

4. **Add tests**:
   - Test that **old profiles (without the new field) deserialize and read correctly**.
   - Test that **new profiles (with the new field set) read correctly**.
   - Test edge cases (e.g., `None` values for optional fields).

5. **Document the change**:
   - Update this file if the new field represents a structural shift.
   - Add comments in code explaining why the field was added and where it is used.

### Example Workflow: Adding `metadata_uri` Field

```rust
// 1. Update CreatorProfile (append at end)
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
    pub holder_count: u32,
    pub fee_recipient: Address,
    pub registered_at: u32,
    pub metadata_uri: Option<String>,  // new field
}

// 2. Update register_creator
pub fn register_creator(
    env: &Env,
    creator: &Address,
    handle: &String,
    metadata_uri: &Option<String>,  // new parameter
) -> Result<(), ContractError> {
    // ... validation ...
    let profile = CreatorProfile {
        creator: creator.clone(),
        handle: handle.clone(),
        supply: /* ... */,
        holder_count: 0,
        fee_recipient: /* ... */,
        registered_at: env.ledger().sequence(),
        metadata_uri: metadata_uri.clone(),  // set the new field
    };
    // ... store profile ...
}

// 3. Add test for backward compatibility
#[test]
fn test_old_profile_reads_with_new_field_optional() {
    // Simulate an old profile without metadata_uri
    // Verify it deserializes and get_creator returns None for metadata_uri
}
```

### When the Binary Layout Changes

If you find that a required field must change type or position:

1. Create a new version of the struct (e.g., `CreatorProfileV2`).
2. Add a new `DataKey` variant (e.g., `DataKey::CreatorV2(Address)`).
3. Implement a migration function that reads from the old key and writes to the new key (or handles both on read).
4. Test that both old and new profiles coexist and read correctly.

## Storage Key Invariant

The storage invariant (documented in [storage-key-invariants.md](./storage-key-invariants.md)) requires:

> **Invariant**: A creator exists if and only if `DataKey::Creator(address)` is present in storage.

When adding new metadata fields:
- **Do not create separate storage keys** for individual profile fields.
- **All profile metadata must remain in the single `CreatorProfile` struct** at `DataKey::Creator(address)`.
- **Versions of the struct** (if needed) must be handled through versioned keys (e.g., `DataKey::CreatorV2`) or `Option<T>` wrapper fields.

## Event and ABI Stability

Unlike the `ContractError` enum (which has strict ordering guarantees), the `CreatorProfile` struct does not have a fixed numeric ABI. However, all changes must maintain binary serialization compatibility.

- **Do not reorder or remove fields** from `CreatorProfile`.
- **Append new fields** to the end of the struct.
- **Wrap optional additions** in `Option<T>` for clarity.
- **Document additions** so indexers and off-chain clients can adapt.

## References

- [STORAGE_EXTENSION_PATTERN.md](./STORAGE_EXTENSION_PATTERN.md) — General patterns for safe storage extension
- [storage-key-invariants.md](./storage-key-invariants.md) — Storage keys and invariants for the contract
- [creator-keys/src/lib.rs](../creator-keys/src/lib.rs) — `CreatorProfile` definition and usage
