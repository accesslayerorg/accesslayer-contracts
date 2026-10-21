# Stake position storage model

This document describes the `StakePosition` data model used by the key staking
feature, how stake IDs are assigned, and how individual stake positions should
be stored in Soroban persistent storage.

---

## `StakePosition` struct

```rust
pub struct StakePosition {
    pub stake_id: u32,
    pub amount: u32,
    pub unlock_ledger: u32,
}
```

| Field | Type | Description |
| --- | --- | --- |
| `stake_id` | `u32` | Unique position ID within a single `(creator, wallet)` pair. IDs start at `1` and increase by `1` for each new position created by that wallet for that creator. |
| `amount` | `u32` | Number of staked keys held by the position. |
| `unlock_ledger` | `u32` | Soroban ledger sequence number at which the position becomes unlockable. |

### Field notes

- `stake_id` is not global across the contract. It only needs to be unique for
  one wallet staking into one creator.
- `amount` should be treated as a positive quantity.
- `unlock_ledger` records the ledger boundary used by the staking flow when the
  position is later withdrawn or otherwise released.

---

## Stake ID assignment

Stake IDs are assigned sequentially per wallet per creator:

1. The first stake position a wallet opens for a creator gets `stake_id = 1`.
2. The next position for the same wallet and creator gets `stake_id = 2`.
3. The sequence continues monotonically for that wallet and creator pair.

This keeps stake IDs deterministic within a `(creator, wallet)` pair while
allowing other wallets to create their own independent sequences for the same
creator.

---

## Storage key structure

Stake positions should be stored under a composite Soroban storage key that
includes the creator address, the wallet address, and the stake ID.

A representative key shape is:

```rust
StakePosition(creator, wallet, stake_id)
```

The important invariant is that each position gets its own storage slot, so
multiple positions do not overwrite one another.

If the contract also stores the next stake ID for a wallet/creator pair, that
sequence counter should live under a separate composite key derived from the
same creator and wallet pair.

---

## Multiple concurrent positions

A wallet can hold multiple concurrent stake positions for the same creator.

For example, these positions can all exist at the same time:

- `StakePosition(creator, wallet, 1)`
- `StakePosition(creator, wallet, 2)`
- `StakePosition(creator, wallet, 3)`

That means consumers must treat stake positions as a collection keyed by
`(creator, wallet, stake_id)`, not as a single per-wallet balance entry.
