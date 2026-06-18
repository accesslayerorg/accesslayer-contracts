# Creator Keys — Test Helper Reference

A focused reference for every utility in `contract_test_env/mod.rs`.
Each helper is documented with its exact signature, what it does, and a minimal
`#[test]` block you can copy-paste directly into a new test file.

The examples follow a **zero-setup philosophy**: spin up only what the specific
capability under test actually needs. No unrelated fee configs, no extra
registrations, no end-to-end orchestration unless the helper itself requires it.

---

## Table of Contents

1. [Environment Initializers](#1-environment-initializers)
   - [`test_env_with_auths`](#test_env_with_auths)
   - [`set_test_timestamp`](#set_test_timestamp)
   - [`register_creator_keys`](#register_creator_keys)
2. [Pricing and Fee Configuration](#2-pricing-and-fee-configuration)
   - [`set_key_price_for_tests`](#set_key_price_for_tests)
   - [`set_protocol_fee_bps`](#set_protocol_fee_bps)
   - [`set_pricing_and_fees`](#set_pricing_and_fees)
3. [Creator Registration](#3-creator-registration)
   - [`register_test_creator`](#register_test_creator)
   - [`register_test_creator_with_fee_config`](#register_test_creator_with_fee_config)
4. [Storage Manipulation](#4-storage-manipulation)
   - [`set_stored_key_price`](#set_stored_key_price)
5. [Price and Fee Math Helpers](#5-price-and-fee-math-helpers)
   - [`compute_expected_buy_price`](#compute_expected_buy_price)
   - [`compute_expected_sell_price`](#compute_expected_sell_price)
   - [`compute_expected_protocol_fee`](#compute_expected_protocol_fee)
   - [`compute_expected_creator_fee`](#compute_expected_creator_fee)
6. [Unit Conversion](#6-unit-conversion)
   - [`stroops_to_display_units`](#stroops_to_display_units)
7. [State Snapshot and Immutability Assertions](#7-state-snapshot-and-immutability-assertions)
   - [`capture_snapshot`](#capture_snapshot)
   - [`ContractStateSnapshot::assert_unchanged`](#contractstatesnapshotassert_unchanged)
8. [Trade Sequence Helpers](#8-trade-sequence-helpers)
   - [`TradeOperation`](#tradeoperation)
   - [`compute_expected_balance_after_trades`](#compute_expected_balance_after_trades)
9. [Constants Reference](#9-constants-reference)

---

## 1. Environment Initializers

These three helpers are the foundation of every test. They create the Soroban
`Env`, optionally pin the ledger clock, and deploy the contract — in that order.

---

### `test_env_with_auths`

```rust
pub fn test_env_with_auths() -> Env
```

Creates a default Soroban `Env` with `mock_all_auths()` enabled. This bypasses
`require_auth` checks on every entrypoint so tests can call admin and
user-gated methods without constructing real signers.

Use this as the first line of any test that calls a state-mutating entrypoint
(`register_creator`, `buy_key`, `set_fee_config`, etc.).

**When to use the raw `Env::default()` instead:** when you are testing a
read-only method that never calls `require_auth` and you want to be explicit
that no auth is involved.

#### Minimal example

```rust
#[test]
fn test_env_mocks_auth_for_admin_calls() {
    // Arrange: env with mocked auth — no real signer needed
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = soroban_sdk::Address::generate(&env);

    // Act: set_key_price requires admin auth; mock_all_auths satisfies it
    client.set_key_price(&admin, &100_i128);

    // Assert: price is accepted — no auth error
    // (a subsequent get_buy_quote would confirm the price is stored)
}
```

---

### `set_test_timestamp`

```rust
pub fn set_test_timestamp(env: &Env, timestamp: u64)
```

Pins the ledger timestamp to a deterministic value. Useful when a test
produces a snapshot file or when time-dependent logic must be reproducible
across machines and CI runs.

The module also exports the constant `DEFAULT_TEST_TIMESTAMP: u64 = 1_700_000_000`
for use when any stable value will do.

#### Minimal example

```rust
#[test]
fn test_snapshot_timestamp_is_deterministic() {
    let env = test_env_with_auths();

    // Pin the clock before any contract interaction
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP);

    // The ledger timestamp is now fixed — snapshot files produced in this
    // test will always carry the same timestamp regardless of wall-clock time.
    assert_eq!(env.ledger().timestamp(), DEFAULT_TEST_TIMESTAMP);
}
```

---

### `register_creator_keys`

```rust
pub fn register_creator_keys<'a>(env: &'a Env) -> (CreatorKeysContractClient<'a>, Address)
```

Deploys `CreatorKeysContract` into `env` and returns a typed client together
with the contract's `Address`. The `'a` lifetime ties the client to the `Env`
it was created from — both must stay in scope for the duration of the test.

This is the only deployment step needed. There is no separate initializer call;
the contract has no single `initialize` entrypoint.

#### Minimal example

```rust
#[test]
fn test_contract_deploys_cleanly() {
    let env = test_env_with_auths();

    // Deploy and unpack the client and contract id
    let (client, contract_id) = register_creator_keys(&env);

    // The contract id is a valid Address
    // A read-only call with no prior state returns a safe default
    let version = client.get_protocol_state_version();
    assert_eq!(version, 1); // PROTOCOL_STATE_VERSION_INITIAL
    let _ = contract_id; // contract_id available for direct storage access
}
```

---

## 2. Pricing and Fee Configuration

---

### `set_key_price_for_tests`

```rust
pub fn set_key_price_for_tests(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    key_price: i128,
) -> Address
```

Generates a fresh admin address, calls `set_key_price` with it, and returns
the admin. Use this when a test needs a price set but does not care about the
admin identity.

#### Minimal example

```rust
#[test]
fn test_buy_key_rejects_payment_below_price() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Set price — admin identity is irrelevant to this assertion
    set_key_price_for_tests(&env, &client, 500_i128);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    // Payment of 499 is one stroop below the required 500
    let result = client.try_buy_key(&creator, &buyer, &499_i128);
    assert_eq!(result, Err(Ok(ContractError::InsufficientPayment)));
}
```

---

### `set_protocol_fee_bps`

```rust
pub fn set_protocol_fee_bps(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    creator_bps: u32,
    protocol_bps: u32,
) -> Address
```

Generates a fresh admin, calls `set_fee_config`, and returns the admin.
`creator_bps + protocol_bps` must equal `10_000` and `protocol_bps` must not
exceed `5_000`; the contract enforces both constraints and will panic in tests
that pass invalid values.

#### Minimal example

```rust
#[test]
fn test_fee_config_is_stored_correctly() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Configure a 90 / 10 split — no price or creator needed for this assertion
    set_protocol_fee_bps(&env, &client, 9_000, 1_000);

    let config = client.get_fee_config().unwrap();
    assert_eq!(config.creator_bps, 9_000);
    assert_eq!(config.protocol_bps, 1_000);
}
```

---

### `set_pricing_and_fees`

```rust
pub fn set_pricing_and_fees(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    key_price: i128,
    creator_bps: u32,
    protocol_bps: u32,
) -> Address
```

Sets both the key price and the fee config in a single call using the same
generated admin. Use this for quote tests and fee-split assertions where both
values must be consistent.

#### Minimal example

```rust
#[test]
fn test_buy_quote_total_equals_price_plus_fees() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // One call configures everything the quote path needs
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);
    let creator = register_test_creator(&env, &client, "alice");

    let quote = client.get_buy_quote(&creator).unwrap();

    // total_amount must equal price + creator_fee + protocol_fee
    assert_eq!(quote.total_amount, quote.price + quote.creator_fee + quote.protocol_fee);
}
```

---

## 3. Creator Registration

---

### `register_test_creator`

```rust
pub fn register_test_creator(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    handle: &str,
) -> Address
```

Generates a fresh `Address`, calls `register_creator` with the given handle,
and returns the creator address. The handle must be 3–32 characters of
lowercase ASCII letters, digits, or underscores.

#### Minimal example

```rust
#[test]
fn test_registered_creator_is_visible() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = register_test_creator(&env, &client, "alice");

    assert!(client.is_creator_registered(&creator));
    let profile = client.get_creator(&creator).unwrap();
    assert_eq!(profile.supply, 0);
    assert_eq!(profile.holder_count, 0);
}
```

---

### `register_test_creator_with_fee_config`

```rust
pub fn register_test_creator_with_fee_config(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    handle: &str,
    creator_bps: u32,
    protocol_bps: u32,
) -> Address
```

Sets the global fee config and registers a new creator in one step. Use this
when a test needs a creator whose fee split is non-default and you want to
avoid the two-step boilerplate of calling `set_protocol_fee_bps` then
`register_test_creator` separately.

The module also exports two named constants for the canonical split:

```rust
pub const DEFAULT_CREATOR_BPS: u32  = 9_000;
pub const DEFAULT_PROTOCOL_BPS: u32 = 1_000;
```

#### Minimal example

```rust
#[test]
fn test_creator_fee_balance_accrues_on_buy() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Price + fee config + creator registration in one call
    set_key_price_for_tests(&env, &client, 1_000_i128);
    let creator = register_test_creator_with_fee_config(
        &env, &client, "alice",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let buyer = soroban_sdk::Address::generate(&env);

    client.buy_key(&creator, &buyer, &1_000_i128);

    // Creator fee recipient balance should reflect the 90% creator share
    let balance = client.get_creator_fee_balance(&creator).unwrap();
    let expected = compute_expected_creator_fee(1_000, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS);
    assert_eq!(balance, expected);
}
```

---

## 4. Storage Manipulation

---

### `set_stored_key_price`

```rust
pub fn set_stored_key_price(env: &Env, contract_id: &Address, price: i128)
```

Writes the key price directly into persistent storage via `env.as_contract`,
bypassing the `set_key_price` entrypoint and its `require_auth` / positive-value
guards. Use this exclusively for edge-case tests that need to inject values the
public API would reject (e.g., a zero price, a negative price, or a price that
was never set).

> **Do not use this in happy-path tests.** It couples the test to internal
> storage key names. Prefer `set_key_price_for_tests` for all normal cases.

#### Minimal example

```rust
#[test]
fn test_buy_quote_with_zero_price_returns_zero_quote() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);

    // Inject a zero price that the public API would reject
    set_stored_key_price(&env, &contract_id, 0);

    let creator = register_test_creator(&env, &client, "alice");

    // A zero price normalizes to a zero-value quote response
    let quote = client.get_buy_quote(&creator).unwrap();
    assert_eq!(quote.price, 0);
    assert_eq!(quote.total_amount, 0);
}
```

---

## 5. Price and Fee Math Helpers

These pure functions mirror the contract's internal math so test assertions
stay aligned when the pricing model or fee formula changes. They contain no
Soroban SDK calls and require no `Env`.

---

### `compute_expected_buy_price`

```rust
pub fn compute_expected_buy_price(_supply: u32, base_price: i128) -> i128
```

Returns the expected gross price for a buy at the given supply level. The
current model is a flat fixed price, so `_supply` is unused — but the
parameter is kept so call sites remain readable and the signature will not
break if a bonding curve is introduced later.

#### Minimal example

```rust
#[test]
fn test_buy_price_is_independent_of_supply() {
    // No Env needed — pure math
    let price_at_zero   = compute_expected_buy_price(0,   500_i128);
    let price_at_ten    = compute_expected_buy_price(10,  500_i128);
    let price_at_large  = compute_expected_buy_price(999, 500_i128);

    assert_eq!(price_at_zero,  500);
    assert_eq!(price_at_ten,   500);
    assert_eq!(price_at_large, 500);
}
```

---

### `compute_expected_sell_price`

```rust
pub fn compute_expected_sell_price(_supply: u32, base_price: i128) -> i128
```

Returns the expected gross price for a sell at the given supply level.
Mirrors `compute_expected_buy_price` — the gross sell price equals the base
key price. The seller's net payout is `price - creator_fee - protocol_fee`,
computed separately via the fee helpers below.

#### Minimal example

```rust
#[test]
fn test_sell_quote_gross_price_matches_base_price() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer   = soroban_sdk::Address::generate(&env);
    client.buy_key(&creator, &buyer, &1_000_i128);

    let quote = client.get_sell_quote(&creator, &buyer).unwrap();

    // Gross price field must equal the base price regardless of supply
    let expected_gross = compute_expected_sell_price(1, 1_000_i128);
    assert_eq!(quote.price, expected_gross);
}
```

---

### `compute_expected_protocol_fee`

```rust
pub fn compute_expected_protocol_fee(price: i128, protocol_bps: u32) -> i128
```

Computes `price * protocol_bps / 10_000` using floor division. Use this to
derive the expected `protocol_fee` field in a `QuoteResponse` without
duplicating the formula inline.

#### Minimal example

```rust
#[test]
fn test_protocol_fee_is_floor_divided() {
    // 999 * 1000 / 10_000 = 99.9 → floor → 99
    let fee = compute_expected_protocol_fee(999, 1_000);
    assert_eq!(fee, 99);

    // Dust: 1 * 1000 / 10_000 = 0.1 → floor → 0
    let dust_fee = compute_expected_protocol_fee(1, 1_000);
    assert_eq!(dust_fee, 0);
}
```

---

### `compute_expected_creator_fee`

```rust
pub fn compute_expected_creator_fee(price: i128, creator_bps: u32, protocol_bps: u32) -> i128
```

Delegates to `creator_keys::fee::compute_fee_split` and returns the creator
component. The remainder from integer division is assigned to the creator, so
`creator_fee + protocol_fee == price` always holds.

#### Minimal example

```rust
#[test]
fn test_creator_fee_absorbs_rounding_remainder() {
    // 999 * 1000 / 10_000 = 99 protocol; creator gets 900 (remainder included)
    let creator_fee  = compute_expected_creator_fee(999, 9_000, 1_000);
    let protocol_fee = compute_expected_protocol_fee(999, 1_000);

    assert_eq!(creator_fee,  900);
    assert_eq!(protocol_fee,  99);
    assert_eq!(creator_fee + protocol_fee, 999); // conservation holds
}
```

---

## 6. Unit Conversion

---

### `stroops_to_display_units`

```rust
pub const STROOPS_PER_DISPLAY_UNIT: i128 = 10_000_000;

pub fn stroops_to_display_units(stroops: i128) -> i128
```

Converts a raw stroop amount to whole display units by dividing by
`10_000_000` (7 decimal places, matching `KEY_DECIMALS`). Use this in
assertions that compare human-readable amounts rather than raw stroops.

#### Minimal example

```rust
#[test]
fn test_stroop_conversion_to_display_units() {
    // No Env needed — pure arithmetic
    assert_eq!(stroops_to_display_units(10_000_000), 1);  // 1.0000000
    assert_eq!(stroops_to_display_units(25_000_000), 2);  // 2.5000000 → 2 (floor)
    assert_eq!(stroops_to_display_units(9_999_999),  0);  // < 1 display unit
}
```

---

## 7. State Snapshot and Immutability Assertions

Use these helpers to prove that a read-only method does not mutate contract
state. The pattern is: capture → call the method under test → capture again →
assert unchanged.

---

### `capture_snapshot`

```rust
pub fn capture_snapshot(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    holder: &Address,
) -> ContractStateSnapshot
```

Reads `supply`, `holder_count`, and `key_balance` for the given
`(creator, holder)` pair and bundles them into a `ContractStateSnapshot`.

```rust
pub struct ContractStateSnapshot {
    pub supply:       u32,
    pub holder_count: u32,
    pub key_balance:  u32,
}
```

---

### `ContractStateSnapshot::assert_unchanged`

```rust
impl ContractStateSnapshot {
    pub fn assert_unchanged(&self, after: &ContractStateSnapshot)
}
```

Asserts that `self` and `after` are identical. On failure it prints both
snapshots so the diff is immediately visible in the test output.

#### Minimal example — proving a view method is read-only

```rust
#[test]
fn test_get_creator_details_does_not_mutate_state() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");

    // Use a sentinel holder that holds no keys — balance stays 0
    let sentinel = soroban_sdk::Address::generate(&env);

    let before = capture_snapshot(&client, &creator, &sentinel);

    // Call the view method multiple times
    client.get_creator_details(&creator);
    client.get_creator_details(&creator);

    let after = capture_snapshot(&client, &creator, &sentinel);
    before.assert_unchanged(&after);
}
```

---

## 8. Trade Sequence Helpers

---

### `TradeOperation`

```rust
#[derive(Debug, Clone, Copy)]
pub enum TradeOperation {
    Buy,   // increases holder balance by 1
    Sell,  // decreases holder balance by 1 (floor 0)
}
```

A lightweight enum that represents a single buy or sell in a sequence. Pass a
slice of `TradeOperation` values to `compute_expected_balance_after_trades` to
derive the expected final balance without duplicating the counting logic inline.

---

### `compute_expected_balance_after_trades`

```rust
pub fn compute_expected_balance_after_trades(
    initial_balance: u32,
    trades: &[TradeOperation],
) -> u32
```

Applies a sequence of `TradeOperation` values to `initial_balance` and returns
the expected final balance. Each `Buy` increments by 1; each `Sell` decrements
by 1 with a floor of 0. Mirrors the contract's balance tracking logic exactly.

#### Minimal example — verifying a mixed trade sequence

```rust
#[test]
fn test_balance_tracks_buy_sell_sequence() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100_i128);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer   = soroban_sdk::Address::generate(&env);

    // Declare the sequence once — used for both the helper and the actual calls
    let trades = [
        TradeOperation::Buy,
        TradeOperation::Buy,
        TradeOperation::Sell,
        TradeOperation::Buy,
    ];

    let expected = compute_expected_balance_after_trades(0, &trades);
    assert_eq!(expected, 2); // 0 + 1 + 1 - 1 + 1 = 2

    // Execute the same sequence against the contract
    client.buy_key(&creator, &buyer, &100_i128);
    client.buy_key(&creator, &buyer, &100_i128);
    client.sell_key(&creator, &buyer);
    client.buy_key(&creator, &buyer, &100_i128);

    assert_eq!(client.get_key_balance(&creator, &buyer), expected);
}
```

#### Minimal example — floor-at-zero behavior

```rust
#[test]
fn test_balance_never_goes_below_zero() {
    // No Env needed — pure helper
    let trades = [
        TradeOperation::Buy,
        TradeOperation::Sell,
        TradeOperation::Sell, // would go to -1 without the floor
        TradeOperation::Sell,
    ];

    let result = compute_expected_balance_after_trades(0, &trades);
    assert_eq!(result, 0); // floored at 0, never negative
}
```

---

## 9. Constants Reference

| Constant | Type | Value | Purpose |
|---|---|---|---|
| `DEFAULT_TEST_TIMESTAMP` | `u64` | `1_700_000_000` | Stable ledger timestamp for reproducible snapshots |
| `DEFAULT_CREATOR_BPS` | `u32` | `9_000` | Canonical creator share for fixture helpers |
| `DEFAULT_PROTOCOL_BPS` | `u32` | `1_000` | Canonical protocol share for fixture helpers |
| `STROOPS_PER_DISPLAY_UNIT` | `i128` | `10_000_000` | Conversion factor for 7-decimal key amounts |

---

## Composing Helpers: A Practical Cheat-Sheet

The table below maps common test goals to the minimal set of helpers required.

| Goal | Helpers needed |
|---|---|
| Verify a read-only view returns a safe default | `test_env_with_auths` → `register_creator_keys` |
| Test a buy or sell entrypoint | + `set_key_price_for_tests` → `register_test_creator` |
| Test quote math with fees | + `set_pricing_and_fees` (replaces the two above) |
| Assert a view method is read-only | + `capture_snapshot` → `assert_unchanged` |
| Test fee accrual on a creator | `register_test_creator_with_fee_config` (combines fee config + registration) |
| Test a mixed buy/sell sequence | `TradeOperation` slice → `compute_expected_balance_after_trades` |
| Inject an invalid storage state | `set_stored_key_price` (bypass the public API guard) |
| Pin the clock for snapshot tests | `set_test_timestamp` with `DEFAULT_TEST_TIMESTAMP` |

---

## Using This Reference in a New Test File

Every integration test binary in `creator-keys/tests/` declares the shared
module with:

```rust
mod contract_test_env;
```

Then imports only what it needs:

```rust
use contract_test_env::{
    register_creator_keys,
    register_test_creator,
    set_key_price_for_tests,
    test_env_with_auths,
};
```

A complete minimal test file looks like this:

```rust
mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator,
    set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::testutils::Address as _;

#[test]
fn test_buy_key_unregistered_creator_fails() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100_i128);

    let unregistered = soroban_sdk::Address::generate(&env);
    let buyer        = soroban_sdk::Address::generate(&env);

    let result = client.try_buy_key(&unregistered, &buyer, &100_i128);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}
```
