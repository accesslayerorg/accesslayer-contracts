# Time comparison rules

Contracts must compare values in the same unit and use the shared helpers in
`common::time` instead of open-coded boundary checks.

| Rule | Unit | Boundary | Helper |
| --- | --- | --- | --- |
| Timelock action becomes executable | ledger timestamp (seconds) | inclusive at `eta` | `is_unlocked(now, eta)` |
| Vesting cliff becomes claimable | ledger timestamp (seconds) | inclusive at `start + cliff` | `is_unlocked(now, cliff)` |
| Vesting schedule completes | ledger timestamp (seconds) | inclusive at `start + duration` | `is_unlocked(now, end)` |
| Pending vesting-admin proposal expires | ledger sequence | exclusive after `expiry_ledger` | `is_expired(sequence, expiry)` |
| Vesting accrual elapsed time | ledger timestamp (seconds) | saturating subtraction | `elapsed_since(now, started_at)` |

`is_unlocked` is true at the exact boundary. `is_expired` remains false at the
exact deadline and becomes true on the next unit. `elapsed_since` returns zero
when the current value precedes the start value, so large gaps and clock skew
cannot underflow.
