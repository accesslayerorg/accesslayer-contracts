# Dividend distribution: caller permissions and protocol fee behavior

This document explains how the `distribute_dividend` entrypoint works, including who can call it and how fees are applied.

---

## Open caller model

The `distribute_dividend` function is callable by **any address**, not just the creator. This is intentional and enables several use cases:

- **Fans can distribute**: A fan who holds keys can distribute dividends to other holders of the same creator's keys.
- **Integrators can distribute**: Third-party applications can distribute dividends on behalf of users or as part of reward programs.
- **Creators can distribute**: The creator themselves can distribute dividends to their key holders.

There is no authorization check on the caller's address. The only requirements are:

1. The creator must be registered in the contract.
2. There must be at least one key holder (supply > 0).
3. The fee configuration must be set.
4. The protocol must not be paused.
5. The distribution amount must be positive.

---

## Protocol fee deduction

When a dividend is distributed, the **protocol fee is deducted before** the remaining amount is split proportionally among key holders.

### Fee calculation

```
protocol_fee = floor(distribution_amount * protocol_bps / 10_000)
net_amount   = distribution_amount - protocol_fee
```

The `protocol_bps` value is set by the protocol admin via `set_fee_config` and represents the protocol's share of all transactions, including dividend distributions.

### Immediate payment to fee recipient

The protocol fee is transferred to the protocol fee recipient **immediately at distribution time**. The fee recipient does not need to claim this amount separately — it is credited to their balance when the distribution occurs.

---

## Worked example

**Setup:**
- Distribution amount: 1000 XLM
- Protocol fee: 2% (protocol_bps = 200)
- Key holders: Alice (3 keys), Bob (2 keys), Charlie (1 key)
- Total supply: 6 keys

**Step 1: Calculate protocol fee**
```
protocol_fee = floor(1000 * 200 / 10_000) = floor(20.0) = 20 XLM
```

**Step 2: Calculate net distribution amount**
```
net_amount = 1000 - 20 = 980 XLM
```

**Step 3: Calculate per-key share**
```
per_key = floor(980 / 6) = floor(163.333...) = 163 XLM
```

**Step 4: Calculate each holder's claimable amount**
```
Alice:   163 * 3 = 489 XLM
Bob:     163 * 2 = 326 XLM
Charlie: 163 * 1 = 163 XLM
```

**Total distributed to holders:** 489 + 326 + 163 = 978 XLM

Note: 2 XLM remains undistributed due to integer division rounding. This dust stays in the contract and is not lost — it simply is not allocated to any holder.

**Step 5: Protocol fee recipient**
```
Protocol fee recipient balance increases by 20 XLM immediately.
```

---

## Rounding behavior

The per-key share is calculated using **floor integer division**:

```
per_key = floor(net_amount / total_supply)
```

The remainder from this division is not distributed to any holder. This means:

- The total distributed to holders may be slightly less than `net_amount`.
- The dust amount is negligible for large distributions.
- The contract does not track or expose the undistributed dust.

---

## Summary

| Aspect | Behavior |
|---|---|
| **Caller** | Any address (open caller model) |
| **Fee deduction** | Protocol fee deducted before splitting |
| **Fee timing** | Immediate at distribution time |
| **Fee recipient** | Receives deducted amount immediately |
| **Per-key calculation** | Floor integer division |
| **Rounding dust** | Remainder stays in contract, not distributed |

---

For dividend accumulator math and settlement details, see [proportional dividend share tests](../creator-keys/tests/proportional_dividend_share.rs).
