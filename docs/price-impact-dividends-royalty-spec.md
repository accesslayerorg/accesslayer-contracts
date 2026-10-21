# Specification: Bonding Curve Price Impact Limits, Dividend Pools, Buy Caps & Secondary Transfer Royalties

## 1. Price Impact Limit (#928)
- Calculates price impact percentage for single buy/sell orders before execution.
- Rejects trades that exceed configured maximum basis points with `SlippageExceeded`.
- Emits `PriceImpactConfigured` and `PriceImpactExceeded` events.

## 2. On-Chain Pro-Rata Dividend Pool (#954)
- Accumulates trading fee revenue into an on-chain dividend pool.
- Disperses funds pro-rata based on holder snapshot balances.
- Prevents double claims within the same cycle with `AlreadyClaimed`.
- Emits `DividendPoolFunded` and `DividendClaimed` events.

## 3. Transaction Buy Cap (#955)
- Creator-defined per-transaction purchase cap.
- Enforces strict compliance on both standard and batch purchase paths.
- Emits `BuyCapSet` event.

## 4. Secondary Transfer Royalty with Creator Routing (#956)
- Deducts configured basis points on secondary market transfers.
- Atomically routes royalty to the creator wallet alongside the transfer execution.
- Emits `SecondaryRoyaltyPaid` event.
