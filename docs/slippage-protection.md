# Slippage protection for trading clients

Creator Keys trading operations support price bounds so a transaction can fail safely when the on-chain quote moves before it is executed.

## Buy protection

When submitting a buy, supply a maximum acceptable total price. The contract evaluates the current quote and rejects the operation with `SlippageExceeded` when the required price is higher than that bound. A bound equal to the quoted amount is accepted.

## Sell protection

When submitting a sell, supply a minimum acceptable proceeds value. The contract rejects the operation with `SlippageExceeded` when the current proceeds are lower than that bound. A bound equal to the quoted proceeds is accepted.

## Client workflow

1. Read a fresh quote immediately before building the transaction.
2. Derive a limit from that quote and the user-selected tolerance.
3. Submit the limit with the trade invocation.
4. On `SlippageExceeded`, discard the stale quote and present a refreshed quote for explicit confirmation.

Price bounds are evaluated as part of contract execution. They are not a substitute for refreshing quotes in a client interface, and a client must never silently retry a rejected trade with a wider tolerance.