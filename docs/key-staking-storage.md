# Key Staking Storage and Data Model

This document describes how stake positions are stored, how each position gets a unique ID, and what the data model looks like in Soroban storage for the key staking feature.

---

## `StakePosition` Struct

The `StakePosition` struct represents a single active or historical staking position for a wallet.

```rust
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct StakePosition {
    pub stake_id: u32,
    pub amount: u32,
    pub unlock_ledger: u32,
}
```

| Field | Type | Unit | Constraints | Description |
|---|---|---|---|---|
| `stake_id` | `u32` | Identifier | `> 0` | A unique, sequential identifier for the stake position per staker wallet per creator. |
| `amount` | `u32` | Keys (whole units) | `> 0` | The number of creator keys currently staked in this position. |
| `unlock_ledger` | `u32` | Soroban ledger sequence number | Strictly `>` current ledger sequence at staking time | The earliest ledger height at which the staked keys can be unstaked (unlocked). |

---

## Stake ID Assignment Logic

When a user stakes keys for a creator:
1. **Scope**: Stake IDs are scoped per staker (wallet) address and per creator address.
2. **Initial Value**: The first stake position created by a specific wallet for a specific creator is assigned `stake_id = 1`.
3. **Sequential Increment**: Each subsequent stake position created by the same wallet for the same creator increments the counter sequentially (e.g., `2`, `3`, `4`, etc.).
4. **Counter Storage**: The contract maintains a counter tracking the next available stake ID for each wallet/creator pair. This counter is initialized to `1` on first use and incremented after a position is successfully created.

---

## Storage Key Structure

Staking data is stored in Soroban's persistent storage using two primary storage key patterns under the `DataKey` enum:

### 1. `DataKey::StakePosition(Address, Address, u32)`

- **Key Format**: `DataKey::StakePosition(staker, creator, stake_id)`
  - `staker`: `Address` of the wallet staking the keys.
  - `creator`: `Address` of the creator whose keys are being staked.
  - `stake_id`: `u32` identifier assigned to this position.
- **Value Type**: `StakePosition` struct.
- **Purpose**: Persists the metadata (stake ID, amount, and unlock ledger) for a specific staking position.

### 2. `DataKey::NextStakeId(Address, Address)`

- **Key Format**: `DataKey::NextStakeId(staker, creator)`
  - `staker`: `Address` of the wallet.
  - `creator`: `Address` of the creator.
- **Value Type**: `u32`
- **Purpose**: Tracks the next sequential stake ID to assign for the given `(staker, creator)` pair. Defaults to `1` if the entry does not exist in storage.

---

## Multiple Concurrent Stake Positions

A key feature of the staking data model is support for **multiple concurrent stake positions**:
- **Concurrency**: A single wallet (staker) can hold multiple active stake positions for the same creator simultaneously.
- **Independence**: Since each position is stored under a unique composite key containing `stake_id` (e.g., `DataKey::StakePosition(staker, creator, stake_id)`), creating a new position does not overwrite or merge with existing positions.
- **Flexibility**: Each concurrent position behaves independently:
  - Each has its own distinct `unlock_ledger` lockup period.
  - Stakers can unlock or manage each position separately by reference to its unique `stake_id`.
