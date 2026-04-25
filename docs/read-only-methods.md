# Read-only contract methods: return value semantics

This document covers every read-only (`get_*` / `is_*`) entrypoint in the `creator-keys` contract. For each method it describes the return type, unit and precision, and what callers should expect on edge-case inputs.

For fee split math, see [fee-assumptions.md](./fee-assumptions.md). For integration boundaries, see [contract-consumer-boundaries.md](./contract-consumer-boundaries.md).

---

## Quote methods

### `get_buy_quote(creator: Address) → Result<QuoteResponse, ContractError>`

Returns the current quote for purchasing one key for a creator.

| Field | Type | Semantics |
|---|---|---|
| `price` | `i128` | Raw key price as stored (stroops or protocol-defined unit). |
| `creator_fee` | `i128` | Creator's share of the fee at the stored fee config bps. Always `≥ 0`. |
| `protocol_fee` | `i128` | Protocol share. Always `≥ 0`. |
| `total_amount` | `i128` | `price + creator_fee + protocol_fee` — the amount the buyer must supply. |

**Edge cases:**
- Returns `Err(ContractError::NotRegistered)` if `creator` is not registered.
- Returns `Err(ContractError::KeyPriceNotSet)` if no key price has been stored.
- Returns `Err(ContractError::FeeConfigNotSet)` if no fee config has been stored.
- A zero key price is not storable (enforced by `set_key_price`), so `price` is always `> 0` for a successful quote.
- `total_amount ≥ price` always holds because fees are additive on the buy path.

---

### `get_sell_quote(creator: Address, holder: Address) → Result<QuoteResponse, ContractError>`

Returns the current quote for selling one key held by `holder` for `creator`.

| Field | Type | Semantics |
|---|---|---|
| `price` | `i128` | Raw key price (same source as buy). |
| `creator_fee` | `i128` | Creator's share deducted from the sell proceeds. |
| `protocol_fee` | `i128` | Protocol share deducted from the sell proceeds. |
| `total_amount` | `i128` | `price - creator_fee - protocol_fee` — the net amount the seller receives. |

**Edge cases:**
- Returns `Err(ContractError::InsufficientBalance)` if `holder` holds zero keys for `creator`.
- Returns `Err(ContractError::NotRegistered)` if `creator` is not registered.
- Returns `Err(ContractError::KeyPriceNotSet)` / `Err(ContractError::FeeConfigNotSet)` if configuration is absent.
- `total_amount` may be `0` when fees equal the full price (e.g., 100% protocol fee, allowed within config constraints).
- `total_amount` is never negative: `checked_format_quote_response` returns `Err(ContractError::Overflow)` rather than allow underflow.

---

## Supply and balance methods

### `get_total_key_supply(creator: Address) → u32`

Returns the current total supply of keys for `creator`.

- Returns `0` for unregistered creators (no panic).
- Precision: integer key count; no decimals at the supply level.
- Equivalent to `read_key_balance` helper output.

---

### `get_creator_supply(creator: Address) → Result<u32, ContractError>`

Returns the supply for a registered creator.

- Returns `Err(ContractError::NotRegistered)` for unknown creators — use `get_total_key_supply` if you need a zero-safe fallback.

---

### `get_key_balance(creator: Address, wallet: Address) → u32`

Returns the number of keys `wallet` holds for `creator`.

- Returns `0` if either `creator` is unregistered or `wallet` has never bought a key.
- No error variant; always returns a `u32`.

---

### `get_holder_key_count(creator: Address, holder: Address) → HolderKeyCountView`

Returns a struct view of a holder's key count.

| Field | Type | Semantics |
|---|---|---|
| `creator` | `Address` | Echo of the input creator address. |
| `holder` | `Address` | Echo of the input holder address. |
| `key_count` | `u32` | Number of keys held; `0` when creator is unregistered or holder has no keys. |
| `creator_exists` | `bool` | `true` if the creator is registered. |

**Edge cases:** Never panics. When `creator_exists` is `false`, `key_count` is always `0` regardless of any storage state.

---

### `get_creator_holder_count(creator: Address) → u32`

Returns the number of distinct addresses holding at least one key for `creator`.

- Returns `0` for unregistered creators.
- Decrements when a holder sells their last key.

---

## Creator profile methods

### `get_creator(creator: Address) → Result<CreatorProfile, ContractError>`

Returns the full profile for a registered creator.

- Returns `Err(ContractError::NotRegistered)` for unregistered creators.
- `CreatorProfile.supply` equals `get_total_key_supply` output.

---

### `get_creator_details(creator: Address) → CreatorDetailsView`

Returns a non-optional creator details snapshot.

| Field | Type | Semantics |
|---|---|---|
| `creator` | `Address` | Echo of input. |
| `handle` | `String` | Display handle; empty string when `is_registered` is `false`. |
| `supply` | `u32` | Current key supply; `0` when unregistered. |
| `is_registered` | `bool` | `true` if the creator is registered. |

**Edge cases:** Never panics. Prefer this over `get_creator` when you want a stable shape without `Result` branching.

---

### `get_key_name(creator: Address) → Result<String, ContractError>`

Returns the creator's handle as the key display name.

- Returns `Err(ContractError::NotRegistered)` for unregistered creators.

---

### `get_key_symbol(creator: Address) → Result<String, ContractError>`

Returns the creator's handle as the key ticker symbol.

- Returns `Err(ContractError::NotRegistered)` for unregistered creators.
- Returns the same string as `get_key_name` — both reflect the handle.

---

### `is_creator_registered(creator: Address) → bool`

Returns `true` if a creator profile exists for `creator`, `false` otherwise.

- Does not mutate state.
- Preferred over `get_creator` when you only need a boolean check.

---

## Fee configuration methods

### `get_protocol_fee_view(env: Env) → ProtocolFeeView`

Returns a non-optional protocol fee configuration snapshot.

| Field | Type | Semantics |
|---|---|---|
| `creator_bps` | `u32` | Creator share in basis points (`0–10000`). |
| `protocol_bps` | `u32` | Protocol share in basis points. |
| `is_configured` | `bool` | `true` once `set_fee_config` has been called. |

**Edge cases:** When `is_configured` is `false`, both bps fields are `0`. Never returns `None` — safe for indexers that require a stable schema.

---

### `get_fee_config(env: Env) → Option<FeeConfig>`

Returns the raw `FeeConfig` if set, `None` otherwise.

- Lower-level than `get_protocol_fee_view`; avoid in indexer code where `None` handling adds complexity.

---

### `get_creator_fee_config(creator: Address) → CreatorFeeView`

Returns the fee configuration view scoped to a creator.

| Field | Type | Semantics |
|---|---|---|
| `creator_bps` | `u32` | Creator share; `0` if unregistered or fee config absent. |
| `protocol_bps` | `u32` | Protocol share; `0` if unregistered or fee config absent. |
| `is_registered` | `bool` | `false` if the creator is not registered. |
| `is_configured` | `bool` | `false` if no global fee config has been set. |

**Edge cases:** Never panics. When `is_registered` is `false`, all numeric fields are `0` regardless of any stored fee config.

---

### `get_creator_fee_bps(creator: Address) → Result<u32, ContractError>`

Returns the creator-facing basis-point share for a registered creator.

- Returns `Err(ContractError::NotRegistered)` for unregistered creators.
- Returns `Err(ContractError::FeeConfigNotSet)` if no protocol fee config exists.

---

### `get_creator_treasury_share(creator: Address) → Result<u32, ContractError>`

Alias for `get_creator_fee_bps`. Returns the creator-facing bps from the global fee config.

- Same error conditions as `get_creator_fee_bps`.

---

### `get_creator_fee_recipient(creator: Address) → Result<Address, ContractError>`

Returns the fee recipient address for `creator` (defaults to the creator's own address at registration).

- Returns `Err(ContractError::NotRegistered)` for unregistered creators.

---

### `is_protocol_config_initialized(env: Env) → bool`

Returns `true` if a protocol fee configuration has been stored; `false` otherwise.

- Does not mutate state.

---

## Protocol-level read methods

### `get_protocol_state_version(_env: Env) → u32`

Returns the fixed `PROTOCOL_STATE_VERSION` constant (`1` currently).

- Does not read or mutate storage.
- Bump this value (in the source constant) when externally visible protocol semantics change.

---

### `get_key_decimals(_env: Env) → u32`

Returns `KEY_DECIMALS` (`7`), matching the standard Soroban token decimal convention.

- Does not read or mutate storage.
- Use this to format key amounts for display (divide raw value by `10^7`).

---

### `get_treasury_address(env: Env) → Option<Address>`

Returns the configured treasury address, or `None` if not yet set.

---

### `get_protocol_admin(env: Env) → Option<Address>`

Returns the configured protocol admin address, or `None` if not yet set.

---

### `get_protocol_fee_recipient(env: Env) → Option<Address>`

Returns the configured protocol fee recipient address, or `None` if not yet set.

---

## Precision and units

All monetary values (`price`, `creator_fee`, `protocol_fee`, `total_amount`) are raw `i128` integers in the same unit as the stored key price. No decimal conversion is performed on-chain. Off-chain callers should divide by `10^get_key_decimals()` for human-readable display.

Basis points fields (`creator_bps`, `protocol_bps`) are in units of `1/10000` (e.g., `1000` = 10%). The sum `creator_bps + protocol_bps` always equals `10000` when a valid fee config is stored.
