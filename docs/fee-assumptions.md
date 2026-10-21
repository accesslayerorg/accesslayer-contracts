# Fee Split Assumptions

This document describes the assumptions, behavior, and integration points for creator and protocol fee split logic in Access Layer contracts.

## Fee Representation

- **Basis points (bps)**: Fee shares are expressed in basis points, where 10,000 = 100%.
- **Validation**: `creator_bps + protocol_bps` must equal 10,000. Any other sum is rejected.
- **Protocol cap**: `protocol_bps` is capped at 5,000 (50%) and `set_fee_config` rejects values above the cap with `ContractError::ProtocolFeeExceedsCap`.
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
- **Graduated milestone fee reduction (#1076)**: `set_milestones` configures an additional per-key scheduled fee (`MilestoneTier { supply, fee_bps }`). It does not change the global `FeeConfig` split; the milestone fee is tracked separately per key, applied post-swap by `check_and_apply_milestone`, only ever moves downward, and is readable via `get_current_milestone`.

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

## Per-Key Dynamic Swap Fee (#1056)

The separate protocol trade fee collected by `buy_key` / `sell_key` uses
per-key rolling gross trading volume. The key identifier is the registered
creator address, and trade volume uses the raw `i128` base units already used
for curve prices and fee calculations; no token-decimal conversion is applied.
Buys count the gross curve price executed, and sells count gross proceeds
before fees. Failed or reverted transactions record no volume because volume
writes share Soroban transaction atomicity with settlement.

The default brackets are `< 100` units = 150 bps, `100..=1,000` = 100 bps,
and `> 1,000` = 75 bps. Admins may replace these with strictly increasing
minimum-volume thresholds (first threshold must be zero), each carrying a fee
from 0 through 10,000 bps. Bracket updates preserve accumulated volume.

The fee for a trade is selected from volume immediately before that trade;
that trade's gross volume is recorded after fee collection. A trade that
crosses a boundary uses the old rate, and the next trade uses the new rate.
The bracket rate replaces the nonzero rate configured with `set_protocol_fee`;
that existing configuration is still required to activate collection and
provide a treasury destination. An explicitly configured rate of zero keeps
trade fee collection disabled.

Volume uses 5-minute timestamp buckets in per-key persistent storage. A bucket
is counted only when its start timestamp is within the exact 86,400-second
cutoff. This bounded model never includes activity older than 24 hours, but
can conservatively undercount activity in the oldest partially overlapping
bucket by up to five minutes. At exactly the 24-hour boundary it remains
eligible; once the cutoff passes its bucket start it is excluded.

`get_current_fee_bps(key_id)` and `get_key_rolling_volume(key_id)` are
read-only. `FeeAdjusted` is emitted when a later successful trade detects a
changed rate, including changes due to volume expiry or a bracket update. An
expiry event reports the rolling volume before the current trade is added; a
crossing event caused by that trade reports volume after it is added. A
contract cannot execute solely because time passed, so expiry events occur
only when a later successful trade observes the change. No event is emitted
when the fee remains unchanged.
