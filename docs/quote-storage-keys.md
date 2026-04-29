# Quote Storage Keys and Invariants

This document defines the storage keys involved in quote computation for `get_buy_quote` and `get_sell_quote` in the `creator-keys` contract.

Quotes are derived views. No `QuoteResponse` values are persisted in storage.

## Scope

Quote input validation and quote math are implemented in:

- `resolve_quote_inputs`
- `compute_fees_for_payment`
- `get_buy_quote`
- `get_sell_quote`

## Storage Keys Used By Quotes

| Storage Key | Value Type | Used By | Purpose |
| --- | --- | --- | --- |
| `DataKey::KeyPrice` | `i128` | buy + sell quotes | Base quote price shared by all creators |
| `DataKey::Creator(creator)` | `CreatorProfile` | buy + sell quotes | Registration gate; quotes require creator to exist |
| `DataKey::FeeConfig` | `FeeConfig` | buy + sell quotes | Fee split source (`creator_bps`, `protocol_bps`) |
| `DataKey::KeyBalance(creator, holder)` | `u32` | sell quotes only | Holder balance gate; sell quotes require balance > 0 |

## Key Invariants

### `DataKey::KeyPrice`

- Must exist before quote methods can return non-error values.
- Missing key must yield `ContractError::KeyPriceNotSet`.
- Negative values are invalid for quotes and must yield `ContractError::NotPositiveAmount`.
- Zero is treated as a no-op quote input and returns a zeroed `QuoteResponse`.

### `DataKey::Creator(creator)`

- Creator must be registered for both buy and sell quotes.
- Missing creator must yield `ContractError::NotRegistered`.
- Quote methods only read this key; they never mutate creator state.

### `DataKey::FeeConfig`

- Must exist for quote fee computation.
- Missing key must yield `ContractError::FeeConfigNotSet`.
- Stored config must satisfy protocol fee constraints established by `set_fee_config`:
  - `creator_bps + protocol_bps == 10_000`
  - `protocol_bps <= 5_000`
- Quote math assumes this invariant and computes fees with checked arithmetic.

### `DataKey::KeyBalance(creator, holder)`

- Read only for sell quote eligibility.
- Missing key is interpreted as `0`.
- Sell quote requires effective balance > 0, otherwise returns `ContractError::InsufficientBalance`.
- Quote path does not mutate holder balances.

## Update Expectations

Quote methods are read-only with respect to contract storage.

Expected quote-path storage behavior:

- `get_buy_quote`: reads `KeyPrice`, `Creator(creator)`, `FeeConfig`; performs no writes.
- `get_sell_quote`: reads `KeyPrice`, `Creator(creator)`, `KeyBalance(creator, holder)`, `FeeConfig`; performs no writes.

State-changing methods (`set_key_price`, `set_fee_config`, `register_creator`, `buy_key`, `sell_key`) may change future quote outputs by updating those underlying keys.

## Formula and Consistency Expectations

For price `p`, creator fee `c`, and protocol fee `f`:

- Buy quote: `total_amount = p + c + f`
- Sell quote: `total_amount = p - c - f`

Additional expectations:

- Fees are derived from `FeeConfig` with checked arithmetic.
- Overflow in fee/total computations must return contract errors (`Overflow` or `SellUnderflow`).
- Quote values are deterministic for unchanged storage state.
