# Fee Split Assumptions

This document describes the assumptions, behavior, and integration points for creator and protocol fee split logic in Access Layer contracts.

## Fee Representation

- **Basis points (bps)**: Fee shares are expressed in basis points, where 10,000 = 100%.
- **Validation**: `creator_bps + protocol_bps` must equal 10,000. Any other sum is rejected.
- **Example**: 9,000 creator_bps + 1,000 protocol_bps = 90% creator, 10% protocol.

## Rounding and Remainder Handling

- **Protocol amount**: Computed as `floor(total * protocol_bps / 10000)`.
- **Creator amount**: `total - protocol_amount`, so the remainder from integer division goes to the creator.
- **Balance conservation**: `creator_amount + protocol_amount == total` for all valid inputs.
- **Zero and negative**: For `total <= 0`, returns `(0, 0)`.

### Examples

| total | creator_bps | protocol_bps | creator | protocol |
|-------|-------------|--------------|---------|----------|
| 1000  | 9000        | 1000         | 900     | 100      |
| 1000  | 10000       | 0            | 1000    | 0        |
| 1000  | 0           | 10000        | 0       | 1000     |
| 999   | 9000        | 1000         | 900     | 99       |
| 0     | 9000        | 1000         | 0       | 0        |
| 1     | 9000        | 1000         | 1       | 0        |

## Overflow and Precision Limits

- **Amount type**: `i128` (Soroban token amounts).
- **Overflow**: `total * protocol_bps` must fit in `i128`. For typical 7-decimal tokens, this is safe for amounts up to approximately 10^20 smallest units.
- **Dust**: Amounts of 1 or 2 units may round protocol share to 0; creator receives the full amount.

## Storage and Configuration

- **DataKey**: `FeeConfig` stores a global `FeeConfig` struct with `creator_bps` and `protocol_bps`.
- **Initialization**: Fee config must be set explicitly via `set_fee_config` before `compute_fees_for_payment` can be used.
- **Admin**: `set_fee_config` requires authorization of the `admin` address. Admin key management is out of scope for this module.

## Contract Interface

| Function | Purpose |
|----------|---------|
| `set_fee_config(env, admin, creator_bps, protocol_bps)` | Set fee split; requires admin auth. |
| `get_fee_config(env)` | Read current fee config. |
| `compute_fees_for_payment(env, total)` | Compute `(creator_amount, protocol_amount)` from stored config. |

## Integration with Payment Flow

The fee split logic is designed to be called from the payment handler (see `contracts-economics-02`):

1. Receive total payment amount (e.g., from bonding-curve buy).
2. Call `compute_fees_for_payment(env, total)` to get `(creator_amount, protocol_amount)`.
3. Transfer `creator_amount` to the creator.
4. Transfer `protocol_amount` to the protocol treasury.

Token transfers are not implemented in this module.

## Client and Server Implications

### Events

When buy/sell flows emit events, include `(creator_amount, protocol_amount)` in the payload so indexers and UIs can display the fee breakdown without recomputing. The payment handler (future work) should emit these values.

### Quote Flow

Clients can call `get_fee_config` and `compute_fees_for_payment` (or a view that wraps them) to show users a fee preview before they sign a transaction.

### Indexing

Server/indexer can rely on fee breakdown in events for analytics and display. No additional off-chain computation is required for fee display.
