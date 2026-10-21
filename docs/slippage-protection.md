# Slippage protection for trading clients

Creator Keys trading operations support price bounds so a transaction can fail safely when the on-chain quote moves before it is executed.

## Buy protection

When submitting a buy, supply a maximum acceptable total price. The contract evaluates the current quote and rejects the operation with `SlippageExceeded` when the required price is higher than that bound. A bound equal to the quoted amount is accepted.

## Sell protection

When submitting a sell, supply a minimum acceptable proceeds value. The contract rejects the operation with `SlippageExceeded` when the current proceeds are lower than that bound. A bound equal to the quoted proceeds is accepted.

## Bonding curve swap protection

`swap` accepts a `max_slippage_bps` tolerance alongside `amount` and a buy/sell `direction`. The contract computes the curve's spot price before and after the whole trade and reverts with `SlippageExceeded` when the impact in basis points is strictly above the tolerance; an impact exactly at the tolerance is accepted. The breach emits a `SwapSlippageExceeded` event carrying both the actual and maximum bps values. Passing `0` skips the check entirely for advanced callers. The `get_price_impact(amount, direction)` view returns the same bps impact without executing anything, so clients can derive a tolerance before building the transaction.

## Client workflow

1. Read a fresh quote immediately before building the transaction.
2. Derive a limit from that quote and the user-selected tolerance.
3. Submit the limit with the trade invocation.
4. On `SlippageExceeded`, discard the stale quote and present a refreshed quote for explicit confirmation.

Price bounds are evaluated as part of contract execution. They are not a substitute for refreshing quotes in a client interface, and a client must never silently retry a rejected trade with a wider tolerance.