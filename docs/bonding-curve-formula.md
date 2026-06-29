# Bonding curve formula and price calculation reference

This document describes how key prices are calculated in the `creator-keys` contract. It covers the price model, fee application, and the full quote pipeline for both buy and sell paths.

For fee split math and rounding rules, see [fee-assumptions.md](./fee-assumptions.md). For quote return value semantics, see [read-only-methods.md](./read-only-methods.md).

---

## Current price model: flat fixed price

The contract currently uses a **flat (constant) bonding curve**. The price per key is the same regardless of supply.

```
price(supply) = KEY_PRICE
```

`KEY_PRICE` is a single `i128` value stored in persistent contract storage and set by the protocol admin via `set_key_price`. It does not change as keys are bought or sold.

This is the simplest bonding curve shape — a horizontal line. It is the intended starting model and may be replaced with a supply-sensitive curve in a future version.

---

## Inputs

| Symbol | Source | Description |
|---|---|---|
| `KEY_PRICE` | `storage::KEY_PRICE` | Fixed price per key in base units (stroops or protocol-defined unit). Always `> 0`. |
| `creator_bps` | `FeeConfig.creator_bps` | Creator fee share in basis points (`0–10000`). |
| `protocol_bps` | `FeeConfig.protocol_bps` | Protocol fee share in basis points (`0–5000`). |

Constraint: `creator_bps + protocol_bps == 10000` always holds for a valid stored config.

---

## Fee calculation

Fees are applied to `KEY_PRICE` using integer basis-point arithmetic:

```
protocol_fee = floor(KEY_PRICE * protocol_bps / 10000)
creator_fee  = KEY_PRICE - protocol_fee
```

The remainder from integer division goes to the creator, so `creator_fee + protocol_fee == KEY_PRICE` exactly.

---

## Buy quote

The buyer pays the key price plus all fees on top:

```
total_amount = KEY_PRICE + creator_fee + protocol_fee
             = KEY_PRICE + KEY_PRICE          (since creator_fee + protocol_fee == KEY_PRICE)
             = 2 * KEY_PRICE
```

`QuoteResponse` fields on the buy path:

| Field | Value |
|---|---|
| `price` | `KEY_PRICE` |
| `creator_fee` | `KEY_PRICE - floor(KEY_PRICE * protocol_bps / 10000)` |
| `protocol_fee` | `floor(KEY_PRICE * protocol_bps / 10000)` |
| `total_amount` | `price + creator_fee + protocol_fee` |

---

## Buyback quote

The current buyback path also follows the fixed-price model, but waives the creator fee:

```
base_price   = KEY_PRICE * amount
protocol_fee = floor(base_price * protocol_bps / 10000)
total_cost   = base_price + protocol_fee
```

Because `KEY_PRICE` is global and flat, buybacks do **not** currently change the gross price returned by later `get_buy_quote` calls. A future move to a supply-sensitive curve would be required for buybacks to move price.

---

## Sell quote

The seller receives the key price minus all fees:

```
total_amount = KEY_PRICE - creator_fee - protocol_fee
             = KEY_PRICE - KEY_PRICE          (since creator_fee + protocol_fee == KEY_PRICE)
             = 0
```

`QuoteResponse` fields on the sell path:

| Field | Value |
|---|---|
| `price` | `KEY_PRICE` |
| `creator_fee` | same as buy |
| `protocol_fee` | same as buy |
| `total_amount` | `price - creator_fee - protocol_fee` |

`total_amount` is guaranteed non-negative. The contract returns `Err(ContractError::SellUnderflow)` rather than allow a negative payout.

---

## Worked example

**Setup:**
- `KEY_PRICE = 1000`
- `creator_bps = 9000` (90%)
- `protocol_bps = 1000` (10%)

**Fee calculation:**
```
protocol_fee = floor(1000 * 1000 / 10000) = floor(100.0) = 100
creator_fee  = 1000 - 100 = 900
```

**Buy quote:**
```
price        = 1000
creator_fee  = 900
protocol_fee = 100
total_amount = 1000 + 900 + 100 = 2000
```

**Sell quote:**
```
price        = 1000
creator_fee  = 900
protocol_fee = 100
total_amount = 1000 - 900 - 100 = 0
```

The seller receives 0 net in this configuration because the combined fee equals the full key price. This is expected behavior — the fee config controls how proceeds are distributed.

---

## Rounding edge case

When `KEY_PRICE` is not evenly divisible by `10000 / protocol_bps`, the remainder goes to the creator:

**Example:** `KEY_PRICE = 999`, `protocol_bps = 1000`
```
protocol_fee = floor(999 * 1000 / 10000) = floor(99.9) = 99
creator_fee  = 999 - 99 = 900
```

`creator_fee + protocol_fee = 999 == KEY_PRICE`. No value is lost.

---

## Units

All monetary values are raw `i128` integers in the same unit as `KEY_PRICE`. No decimal scaling is performed on-chain. Use `get_key_decimals()` (returns `7`) to convert to a human-readable display value.
