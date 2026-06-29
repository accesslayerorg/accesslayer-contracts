# Locked allocation: parameter format and unlock ledger requirements

This document covers the `LockedAllocation` struct used in `register_creator`, the
constraints on `unlock_ledger`, and what happens to bonding curve supply while keys
are locked.

---

## `LockedAllocation` struct

```rust
pub struct LockedAllocation {
    pub amount: u32,
    pub unlock_ledger: u32,
    pub claimed: bool,
}
```

| Field | Type | Unit | Constraints | Description |
|---|---|---|---|---|
| `amount` | `u32` | keys (whole units) | `> 0` | Number of keys to lock for the creator at registration. |
| `unlock_ledger` | `u32` | Soroban ledger sequence number | Strictly `> current ledger sequence at registration time` | The earliest ledger at which `claim_locked_allocation` may be called. |
| `claimed` | `bool` | — | Read-only after storage; always `false` at registration | Tracks whether the locked allocation has been claimed. Set to `true` by `claim_locked_allocation`. |

### Field constraints

- `amount` must be `> 0`. Passing `0` is rejected at registration time.
- `unlock_ledger` must be **strictly greater than** the ledger sequence number at the
  moment `register_creator` is called. Passing a value equal to or less than the
  current ledger returns `ContractError::AllocationLocked`.
- `claimed` must be `false` at registration. If already `true` the allocation is
  silently treated as already consumed.

---

## Passing `LockedAllocation` to `register_creator`

`register_creator` accepts the allocation as the third parameter, wrapped in `Option`:

```rust
client.register_creator(
    &creator,
    &handle,
    &Some(LockedAllocation {
        amount: 100,
        unlock_ledger: current_ledger + 500,
        claimed: false,
    }),
    &None, // max_supply (optional)
);
```

Passing `&None` skips locked allocation entirely.

---

## Supply and price impact at registration

When a `LockedAllocation` is included, the contract **immediately adds `amount` to the
creator's bonding curve supply** at registration time. The locked keys count as if they
were already bought, which means:

- `get_total_key_supply(creator)` returns `amount` right after registration, before any
  external buyer has purchased a key.
- `get_buy_quote` and `get_sell_quote` compute fees against the same fixed key price
  (`KEY_PRICE`) regardless of the locked allocation — the flat price model is
  supply-independent, so the locked keys do not change the price per key.
- The creator's own wallet is not credited with a key balance by locking. The balance
  entry is only written when `claim_locked_allocation` is called after the ledger
  constraint is satisfied.

**Implication for price**: Because the current model uses a flat (constant) bonding
curve, locking 100 keys does not raise or lower the unit price. If the contract is
upgraded to a supply-sensitive curve in the future, a locked allocation would push the
starting price up by the locked supply.

---

## `unlock_ledger` must exceed the registration ledger

The `unlock_ledger` field is compared against `env.ledger().sequence()` at registration:

```
if unlock_ledger <= current_ledger_at_registration {
    return Err(ContractError::AllocationLocked);
}
```

This means:

- Setting `unlock_ledger` to the current ledger number itself is rejected.
- Callers must always pass a value at least one ledger sequence ahead of the time of
  the `register_creator` call.
- There is no maximum value enforced. Very large values (e.g., `u32::MAX`) are
  accepted, locking the allocation indefinitely until that ledger is reached.

---

## Claiming after unlock

Once `env.ledger().sequence() >= unlock_ledger`, the creator may call
`claim_locked_allocation(creator)`:

- The contract sets `claimed = true` in storage so the allocation cannot be claimed
  a second time (`ContractError::AlreadyClaimed` is returned on a duplicate call).
- The creator's key balance is credited with `amount` keys.
- The supply in the creator profile is **not** changed again — it was already counted
  at registration.

---

## Worked example

**Setup:**
- Current ledger sequence: `1000`
- `KEY_PRICE = 500`
- Fee config: `creator_bps = 9000`, `protocol_bps = 1000`
- Locked allocation: `amount = 100`, `unlock_ledger = 1500`

**At registration (ledger 1000):**

```
get_total_key_supply(creator) → 100   // locked keys counted immediately
get_key_balance(creator, creator)  → 0    // creator wallet not credited yet
get_buy_quote(creator).price       → 500  // flat price unchanged
```

**Between ledger 1000 and 1499:**

```
claim_locked_allocation(creator) → Err(ContractError::AllocationLocked)
```

**At ledger 1500 or later:**

```
claim_locked_allocation(creator) → Ok(())
get_key_balance(creator, creator)  → 100  // creator now holds the keys
get_total_key_supply(creator) → 100   // supply unchanged (already counted)
```

**After a buyer purchases one key (supply becomes 101):**

```
get_buy_quote(creator).price → 500   // flat price model: supply has no effect on price
```
