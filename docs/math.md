# Contract math and rounding rules

Checked arithmetic is required for monetary, balance, reserve, and time-derived
contract values. The shared `common::math` module returns `MathError` for
overflow and division by zero. `checked_mul_div` computes the product in
Soroban's 256-bit integer type before dividing, so a valid quotient does not
fail merely because the intermediate product exceeds `i128`.

## Rounding

`RoundingDirection::Down` means mathematical floor; `Up` means mathematical
ceiling. This distinction also applies to negative inputs. Use `Down` at
protocol-settlement boundaries unless an operation documents otherwise:
remainders stay undistributed rather than increasing a payment or liability.

| Operation | Rounding | Beneficiary / effect |
|---|---|---|
| Creator/protocol percentage fee | Down for the calculated fee | The fee recipient receives no more than its exact share; residual units stay with the primary recipient. |
| Pro-rata dividend claim | Down | Dust remains unallocated in the funded pool, favoring the protocol/remaining pool. |
| Royalty and price-impact basis-point calculation | Down | The calculated charge/impact never rounds above the exact fraction. |
| Linear vesting release | Down | Fractional units remain locked until a later release calculation. |
| LP reward and share calculations | Down | Fractional reward units remain in the pool. |
| Asset/oracle conversion | Down | Conversion never credits more value than the exact rate supports. |

`checked_add` returns `MathError::Overflow` rather than wrapping. Contract
entrypoints should map math errors to their stable contract-specific overflow
error. If a future operation needs a different rounding policy, the call site
must pass it explicitly and document who receives the remainder.
