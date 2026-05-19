# Creator Metadata Storage Evolution

This document formalizes how creator metadata and contract state can evolve in Access Layer contracts without breaking existing reads, storage invariants, or downstream indexing.

## Current Storage Layout

The primary storage unit for creator metadata is the `CreatorProfile` struct, stored using the `DataKey::Creator(Address)` key in persistent storage.

### `CreatorProfile` Struct (v1)
```rust
pub struct CreatorProfile {
    pub creator: Address,      // Primary identity of the creator
    pub handle: String,       // User-facing handle (e.g. "@alice")
    pub supply: u32,          // Total supply of creator keys
    pub holder_count: u32,    // Count of unique key holders
    pub fee_recipient: Address, // Target for creator fee distributions
}
```

## Evolution Strategy

To maintain backward compatibility while allowing the metadata to grow, contributors must follow these rules:

### 1. Additive Changes
New fields must be added to the **end** of the `CreatorProfile` struct. 
- Use `Option<T>` for new fields to ensure that existing records (which lack these fields) can still be deserialized correctly by the Soroban SDK.
- The contract logic must handle the `None` case gracefully, typically by providing a sensible default or treating the field as "not yet set".

### 2. View Stability
Public views (structs returned by `get_` methods) are considered stable interfaces for indexers and clients.
- **Never delete or rename fields** in view structs (e.g., `CreatorDetailsView`).
- If a field becomes obsolete, keep it in the struct but document it as `@deprecated`.
- Always provide a scalar default (e.g., `0`, `""`, or `false`) for new fields when reading older state to keep indexer behavior predictable.

### 3. State Versioning
The contract exposes a `PROTOCOL_STATE_VERSION` constant (currently `1`).
- **Increment this value** whenever the semantics of the stored data change significantly, or when a breaking layout change is introduced.
- Clients and indexers can use `get_protocol_state_version()` to detect when they need to update their internal mapping logic.

## Compatibility Rules

| Change Type | Compatibility | Strategy |
|-------------|---------------|----------|
| **Add Optional Field** | Backward | Append `Option<T>` to struct end. |
| **Add Required Field** | Breaking | Requires state migration or incrementing `PROTOCOL_STATE_VERSION`. |
| **Rename Field** | Breaking | Create new field; keep old field as deprecated if possible. |
| **Change DataKey Type** | Breaking | Requires a dual-read strategy or full state migration. |

## Migration Considerations

When introducing changes that affect existing records:

1. **Lazy Migration**: Prefer updating records only when they are next modified by a user action (e.g., during a `register_creator` update or a fee recipient change).
2. **Schema Diffs**: PRs that change `contracttype` structs must include a clear description of the "Before" and "After" state.
3. **Indexer Alignment**: Coordinate with the `accesslayer-server` team to ensure indexing logic is updated before the new contract version is deployed to mainnet.

## Storage Keys and Invariants

- **`DataKey::Creator(Address)`**: Unique per creator address. Must always map to a valid `CreatorProfile`.
- **`DataKey::KeyBalance(Creator, Holder)`**: Stores the `u32` balance of keys. Must never be negative.
- **`DataKey::FeeConfig`**: Global protocol setting. Affects all fee calculations contract-wide.
