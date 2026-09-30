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

### `get_buyback_quote(creator: Address, amount: u32) → Result<i128, ContractError>`

Returns the total creator buyback cost for burning `amount` keys from the creator's own held balance.

Semantics:

- `base_price = KEY_PRICE * amount`
- `protocol_fee = floor(base_price * protocol_bps / 10000)`
- `total_cost = base_price + protocol_fee`

The creator fee is waived on the buyback path, so only the protocol fee is added on top of the gross price.

**Edge cases:**
- Returns `Err(ContractError::NotPositiveAmount)` if `amount == 0`.
- Returns `Err(ContractError::NotRegistered)` if `creator` is not registered.
- Returns `Err(ContractError::KeyPriceNotSet)` if no key price has been stored.
- Returns `Err(ContractError::FeeConfigNotSet)` if no fee config has been stored.
- Returns `Err(ContractError::InsufficientSupply)` if `amount > get_total_key_supply(creator)`.
- The quote does not validate the creator wallet's current held balance; execution-time balance checks occur in `buyback`.

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

## Quote Invariants

The following invariants are guaranteed for successful quote responses:

### Input Invariants
- **Registration**: The `creator` address must be registered via `register_creator`.
- **Pricing**: A global key price must be set via `set_key_price`. `price` is always `> 0`.
- **Fees**: A global fee configuration must be set via `set_fee_config`.

### Output Invariants
- **Non-negativity**: All fee fields (`creator_fee`, `protocol_fee`) and `price` are always `≥ 0`.
- **Total Amount Consistency**:
    - **Buy**: `total_amount = price + creator_fee + protocol_fee`.
    - **Sell**: `total_amount = price - (creator_fee + protocol_fee)`.
- **Net Receivables**: On a sell quote, `total_amount` is guaranteed to be non-negative. The contract returns `Err(ContractError::SellUnderflow)` if fees would result in a negative payout.
- **Rounding**:
    - `protocol_fee` is calculated as `floor(price * protocol_bps / 10000)`.
    - `creator_fee` receives any remainder from integer division to ensure `creator_fee + protocol_fee` exactly matches the intended total fee application when `creator_bps + protocol_bps = 10000`.

### Units and Precision
- All monetary values are raw `i128` integers in the base unit (stroops or protocol-defined).
- No on-chain decimal scaling is performed. Callers should use `get_key_decimals()` for display formatting.

---

## Aggregated key stats

### `get_key_stats(key_id: Address) → Result<KeyStatsView, ContractError>`

Returns a single-call snapshot of all key-level fields for a registered creator. Server sync and admin snapshot endpoints can use this instead of multiple individual reads to reduce RPC round trips.

Bumps the TTL on every storage entry it reads so that a cold read keeps all state alive without a separate `refresh_ttl` call.

| Field | Type | Semantics |
|---|---|---|
| `current_price` | `i128` | Next-purchase price in stroops: the fixed auction price when an auction is active (`supply < auction_supply`), otherwise the bonding-curve price at the current supply. `0` if no key price has been set. |
| `circulating_supply` | `u32` | Keys currently in circulation (does not include locked/unclaimed allocations). |
| `holder_count` | `u32` | Number of distinct wallets holding at least one key. |
| `trading_paused` | `bool` | `true` if either the per-key pause or the global emergency pause is active. |
| `supply_cap` | `u32` | Hard supply ceiling set by the creator. `0` means uncapped. |
| `holder_cap_bps` | `u32` | Per-wallet holding cap in basis points (e.g. `1000` = 10% of supply). `0` means uncapped. |
| `circuit_breaker_threshold_bps` | `u32` | Price-jump threshold that triggers the circuit breaker. Defaults to `30` when never explicitly configured. |
| `lockup_duration_seconds` | `u64` | Sell lockup window in seconds; `0` means no lockup is configured. |
| `launch_penalty_bps` | `u32` | Basis points applied as an early-sell penalty inside the launch window; `0` means no penalty. |
| `buy_cooldown_ledgers` | `u32` | Per-wallet buy cooldown in ledgers; `0` means no cooldown. |
| `max_buy_quantity` | `u32` | Per-transaction buy quantity cap; `0` means no limit. |
| `has_auction` | `bool` | `true` when a pre-launch auction is currently configured. |
| `auction_price` | `i128` | Fixed auction price per key in stroops. `0` when `has_auction` is `false`. |
| `auction_supply` | `u32` | Total keys available at the fixed auction price. `0` when `has_auction` is `false`. |
| `auction_sold` | `u32` | Keys already sold through the auction. `0` when `has_auction` is `false`. |

**TTL behaviour:** Every storage entry read by this function has its TTL extended to at least `TTL_MIN_EXTENSION_LEDGERS` (~30 days). Optional entries that have never been written to storage are skipped (guarded with `.has()`) to avoid a `MissingValue` panic.

**Edge cases:**
- Returns `Err(ContractError::NotRegistered)` for any `key_id` that has never been registered — the 404-equivalent for callers.
- All optional numeric fields (`supply_cap`, `holder_cap_bps`, `launch_penalty_bps`, `buy_cooldown_ledgers`, `max_buy_quantity`) return `0` when not configured.
- `circuit_breaker_threshold_bps` returns the default value `30` when the threshold has never been explicitly set.
- `current_price` is `0` when `KEY_PRICE` has not been written to storage (contract not fully initialised).
- Never panics regardless of which optional per-key settings are absent.

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

Returns the creator-facing share of the protocol fee in basis points.

- Unit: basis points (1 bps = 1/10000). Example: `9000` = 90%.
- Valid range: `0–10000`. When a fee config is stored, `creator_bps + protocol_bps == 10000` always holds.
- Returns `Err(ContractError::NotRegistered)` for unregistered creators.
- Returns `Err(ContractError::FeeConfigNotSet)` if no protocol fee config exists.

---

### `get_creator_treasury_share(creator: Address) → Result<u32, ContractError>`

Alias for `get_creator_fee_bps`. Returns the same creator-facing bps value from the global fee config.

- Unit and range: identical to `get_creator_fee_bps` (basis points, `0–10000`).
- Same error conditions as `get_creator_fee_bps`.

---

### `get_protocol_treasury_share_bps(env: Env) → Result<u32, ContractError>`

Returns the protocol treasury share from the global fee configuration in basis points.

- Unit: basis points (1 bps = 1/10000). Example: `500` = 5%.
- Valid range: `0–5000`. The protocol share is capped at `PROTOCOL_BPS_MAX` (`5000`) by `set_fee_config`.
- When a fee config is stored, `creator_bps + protocol_bps == 10000` always holds, so `protocol_bps` is at most half the total.
- Returns `Err(ContractError::FeeConfigNotSet)` if no protocol fee config has been stored.
- Does not require a creator argument — reads directly from the global protocol config.

---

### `get_creator_fee_recipient(creator: Address) → Result<Address, ContractError>`

Returns the fee recipient address for `creator` (defaults to the creator's own address at registration).

- Returns `Err(ContractError::NotRegistered)` for unregistered creators.

---

### `get_creator_fee_balance(creator: Address) → Result<i128, ContractError>`

Returns accrued creator fee balance for the creator's fee recipient after buy executions.

- Returns `0` when no buy has accrued fees yet.
- Returns `Err(ContractError::NotRegistered)` for unregistered creators.
- Amount unit matches key price and quote fee fields (raw `i128`).

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

### `get_reputation(env: Env, creator: Address) → ReputationView`

Returns a creator's reputation score together with the per-reason counters that
produced it (`keys_launched`, `milestones_reached`, `governance_participation`,
`trade_activity`, `deprecations`) and a signed per-reason breakdown.

The score saturates at zero: a penalty that would push it below zero floors at
zero rather than reverting, so `new_score` is authoritative and the breakdown
does not always re-sum to the score once flooring has occurred.

---

### `get_allowance(env: Env, owner: Address, spender: Address, key_id: Address) → u32`

Returns the remaining transfer allowance `owner` granted to `spender` for
`key_id`, or `0` when none exists. `transfer_from` decrements this in the same
call that moves the keys, and removes the entry once it reaches zero.

---

### `get_sell_tax_bps(env: Env, creator: Address) → u32`

Returns a creator's configured sell tax in basis points, or `0` when unset.
Capped at `MAX_SELL_TAX_BPS` when set through `set_sell_tax_bps`.

---

### `get_buyback_pool_balance(env: Env) → (i128, Option<Address>)`

Returns the accumulated buyback balance in the contract's internal ledger and
the configured pool address. The address is `None` until an admin calls
`set_buyback_pool_address`; until then collected tax is still accounted in the
first element.

---

### `get_escalation_config(env: Env) → Option<EscalationConfig>`

Returns the active poll quorum-escalation policy, or `None` when the protocol
admin has never configured one. A config with `max_extensions == 0` reads back
as `Some` but is treated as disabled.

---

### `get_escalation_status(env: Env, creator: Address, poll_id: u32) → EscalationView`

Returns a proposal's deadline, extensions consumed and allowed, remaining
ledgers, current participation in basis points of circulating supply, the
creator's `quorum_bps`, whether an extension would currently qualify
(`eligible`), and whether the budget is spent (`exhausted`).

`eligible` requires a configured quorum the proposal has not already reached
plus participation of at least `threshold_bps` of that requirement.
`exhausted` is `false` when escalation is disabled, which distinguishes "no
configured budget" from "a spent budget".

---

## Precision and units

All monetary values (`price`, `creator_fee`, `protocol_fee`, `total_amount`) are raw `i128` integers in the same unit as the stored key price. No decimal conversion is performed on-chain. Off-chain callers should divide by `10^get_key_decimals()` for human-readable display.

Basis points fields (`creator_bps`, `protocol_bps`) are in units of `1/10000` (e.g., `1000` = 10%). The sum `creator_bps + protocol_bps` always equals `10000` when a valid fee config is stored.
