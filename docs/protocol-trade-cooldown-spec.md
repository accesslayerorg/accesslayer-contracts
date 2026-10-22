# Specification: Protocol-Wide Per-Wallet Trade Cooldown (#1175)

## 1. Purpose

The protocol admin configures a cooldown window, in seconds, that applies to
**every** bonding-curve buy and sell. While the window is open, the same wallet
cannot trade again, so a wallet cannot round-trip keys
(buy → sell → buy → sell) inside the window.

This is the protocol-level counterpart of the creator-scoped cooldown of
issue #974:

| | Creator-scoped (#974) | Protocol-wide (#1175) |
|---|---|---|
| Configured by | the key's creator | the protocol admin |
| Scope | one creator's key | every creator's key |
| Stored per | `(creator, wallet)` | `wallet` |
| Setter | `set_cooldown(key_id, duration_secs)` | `set_trade_cooldown(admin, seconds)` |
| Max window | `MAX_TRADE_COOLDOWN_SECS` (24 h) | `MAX_TRADE_COOLDOWN_SECS` (24 h) |

The two cooldowns are independent and both are enforced: a trade must pass the
creator-scoped window **and** the protocol-wide window.

## 2. Storage

Defined in [`creator-keys/src/protocol_trade_cooldown.rs`](../creator-keys/src/protocol_trade_cooldown.rs)
as `TradeCooldownDataKey`. Both entries are persistent and extended to the full
`CREATOR_TTL_LEDGERS` window on write.

| Key | Value | Meaning |
|---|---|---|
| `CooldownSecs` | `u64` | The configured window in seconds. `0` (or absent) disables the cooldown. |
| `LastTrade(wallet)` | `u64` | Unix timestamp of the wallet's most recent successful trade. Absent = never traded. |

## 3. Entrypoints

| Entrypoint | Authorization | Description |
|---|---|---|
| `set_trade_cooldown(admin, seconds)` | protocol admin | Sets the window. `0` disables it. `seconds > MAX_TRADE_COOLDOWN_SECS` returns `TradeCooldownError::DurationTooLong`; a non-admin returns `TradeCooldownError::Unauthorized`. |
| `get_trade_cooldown() → u64` | none | The configured window in seconds (`0` = disabled). |
| `get_trade_cooldown_remaining(wallet) → u64` | none | Seconds `wallet` must still wait. `0` when the cooldown is disabled, the wallet never traded, or the window elapsed. |
| `get_wallet_last_trade_timestamp(wallet) → u64` | none | Timestamp of the wallet's last trade, `0` when it never traded. |

## 4. Enforcement

The check runs **before** any price, balance or supply change, so a blocked
trade is a complete no-op. It reverts with `ContractError::CooldownActive`
(36) and publishes a `TradeCooldownBlockedEvent` carrying the exact
`seconds_remaining`.

The timestamp is written **after** the trade settles, so a reverted trade never
consumes a window.

### 4.1 One logical trade = one window

A multi-key purchase or a batch is a single logical trade, so the check runs
once for the whole operation and the timestamp is written once:

| Operation | Check | Timestamp write |
|---|---|---|
| `buy_key`, `buy_key_with_referrer`, `forward_buy` | `execute_buy` | `execute_buy` |
| `buy_keys`, `buy_keys_with_ref_and_slip` | `buy_keys_core` | `buy_keys_with_ref_and_slip` |
| `bundle_buy` | `bundle_buy` and `buy_keys_core` | `bundle_buy` |
| `sell_key`, `sell_key_with_slippage` | `sell_key_with_slippage` | `sell_key_with_slippage` |
| `batch_buy`, `batch_buy_v2` | entrypoint | entrypoint |
| `batch_sell`, `batch_sell_v2` | entrypoint | entrypoint |
| `swap` | covered by `buy_keys` / `batch_sell` | covered by `buy_keys` / `batch_sell` |

`buy_keys_core` runs once per item of a bundle, so it only *checks*; the
timestamp write stays at the entrypoint level, otherwise the second item of a
bundle would find the wallet inside the window the first item had just opened.

### 4.2 Out of scope

The following are not market trades and are not gated:

- `atomic_swap` — a peer-to-peer key transfer; no supply change, no price.
- `transfer_keys`, `transfer_from` — key transfers.
- `airdrop_keys`, `claim_locked_allocation` — creator-side allocations.
- `buyback` — the creator burning their own keys.
- `reinvest_dividend` — settling already-claimed dividends into keys; no
  external funds move and it does not reset the window, so a sell right after
  is still gated.

## 5. Events

| Event | Topics | Data |
|---|---|---|
| `tcd_blk` | `(Symbol("tcd_blk"), wallet)` | `wallet`, `expires_at`, `seconds_remaining` |
| `tcd_set` | `(Symbol("tcd_set"), admin)` | `admin`, `cooldown_secs`, `ledger` |

## 6. Reporting the remaining seconds

A Soroban `#[contracterror]` enum can only carry unit variants, so the error
itself cannot carry the remaining seconds. The value is reported by

- the `TradeCooldownBlockedEvent` published with the rejection
  (`seconds_remaining`), and
- the `get_trade_cooldown_remaining(wallet)` view, which a client can call
  before attempting the trade.

Both report the same exact value: `cooldown_secs - (now - last_trade_timestamp)`.
