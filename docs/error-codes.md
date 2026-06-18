# Contract Error Codes

This document maps `ContractError` enum values to their meanings, likely causes, and expected caller behavior.

Error codes are defined in [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs) as a `#[contracterror]` enum with numeric values for on-chain serialization.

## Error Reference

| Code | Name | Numeric | Likely Cause | Expected Caller Behavior |
|------|------|---------|--------------|--------------------------|
| 1 | `AlreadyRegistered` | 1 | Attempt to register a creator address that is already registered in the contract | Retrieve existing creator profile or contact creator to select a different address |
| 2 | `NotRegistered` | 2 | Attempt to query or transact with a creator address that has not been registered | Register the creator first or verify the creator address is correct |
| 3 | `Overflow` | 3 | Integer arithmetic would overflow or underflow (e.g., supply > u32::MAX or fee math exceeds i128 bounds) | Check input amounts are within safe ranges or wait for supply to decrease; for quotes, verify key price is not near i128::MAX |
| 4 | `InsufficientPayment` | 4 | Payment amount is less than the current key price for a buy operation | Provide payment at least equal to the key price; call `get_buy_quote` to check current price and fees |
| 5 | `KeyPriceNotSet` | 5 | Attempt to use pricing operations (buy, sell, quote) before an admin has set a key price | Contact protocol admin to set a key price via `set_key_price` |
| 6 | `NotPositiveAmount` | 6 | Amount is zero or negative where a positive value is required (e.g., `set_key_price`, `buy_key` payment) | Use a positive amount; for buy/sell, check user input for negative or zero values |
| 7 | `FeeConfigNotSet` | 7 | Attempt to compute or query fees before an admin has set a protocol fee configuration | Contact protocol admin to set fee config via `set_fee_config` |
| 8 | `InvalidFeeConfig` | 8 | Proposed fee configuration violates constraints: `creator_bps + protocol_bps != 10000` | Ensure basis points sum to 10,000; validate client-side before sending |
| 9 | `InsufficientBalance` | 9 | Holder has no keys for the given creator (sell attempt with zero balance) | Verify holder owns keys before attempting a sell; call `get_key_balance` to check holdings |
| 10 | `SellUnderflow` | 10 | Sell operation would underflow supply/balance (internal invariant violation) or sell quote would result in a negative net amount | Report if encountered; for quotes, check fee configuration relative to key price |
| 11 | `ProtocolFeeExceedsCap` | 11 | Proposed fee configuration exceeds the maximum allowed protocol share (`protocol_bps > 5000`) | Reduce protocol share to 50% or lower; see `fee::PROTOCOL_BPS_MAX` |
| 12 | `HandleTooShort` | 12 | Creator handle length is below minimum bound (`< 3`) during registration | Provide a handle with at least 3 characters |
| 13 | `HandleTooLong` | 13 | Creator handle length exceeds maximum bound (`> 32`) during registration | Provide a handle with at most 32 characters |
| 14 | `InvalidHandleCharacter` | 14 | Creator handle contains unsupported characters (allowed set: lowercase `a-z`, digits `0-9`, underscore `_`) | Normalize handle to allowed characters before calling `register_creator` |

## Integration Notes

### Authorization and Registration

- Code 1 (`AlreadyRegistered`) is raised only if the same address calls `register_creator` twice. This is a guard against accidental re-registration; intended behavior should call `get_creator` first or handle registration state off-chain.
- Code 2 (`NotRegistered`) applies to both reads and writes. Callers must always register a creator before buy/sell/quote operations.
- Codes 12 (`HandleTooShort`), 13 (`HandleTooLong`), and 14 (`InvalidHandleCharacter`) are deterministic registration validation failures. Validate handles client-side with the same bounds and character set before submitting transactions.

### Pricing and Fees

- Code 5 (`KeyPriceNotSet`) and Code 7 (`FeeConfigNotSet`) are initialization gates. Clients should detect these and display a message like "Pricing has not been configured yet" rather than retrying immediately.
- Code 8 (`InvalidFeeConfig`) is raised when basis points are invalid. Always validate client-side before sending transactions: `creator_bps + protocol_bps == 10000`.
- Code 11 (`ProtocolFeeExceedsCap`) is raised when the protocol share exceeds the cap: `protocol_bps <= 5000`.

### Buy and Sell

- Code 4 (`InsufficientPayment`) applies only to buy operations. Sellers use code 9 (`InsufficientBalance`).
- Code 6 (`NotPositiveAmount`) can be raised by `set_key_price` (amount must be > 0) or `buy_key` (payment must be > 0).

### Overflow Handling

- Code 3 (`Overflow`) should be rare in typical use. It can arise if:
  - Protocol supply exceeds ~4 billion keys (unlikely in practice).
  - A key price approaches i128::MAX and fee math tries to add fees on top (call `get_buy_quote` first to validate).
  - Arithmetic during division or subtraction overflows (internal invariant violation; report if encountered).

## Event Emission

The contract does not emit error events. Errors are returned to the caller synchronously. Clients should log and handle error codes as part of transaction response validation.

## Version Stability

Error codes are stable and will not change. New error codes may be added in future contract versions without breaking existing error handling. Consumers should handle unexpected error codes gracefully (e.g., log and retry or display a generic error message).
