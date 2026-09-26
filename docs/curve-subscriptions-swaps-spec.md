# Specification: Curve Migration, Key Subscriptions, Atomic Swaps & Multi-Currency Payments

## 1. Bonding Curve Migration (#941)
- Timelocked governance upgrade for bonding curve parameters without supply reset.
- Validates proposal parameters and enforces minimum delay before execution.
- Emits `CurveMigrationExecuted` with new slope and milestone parameters.

## 2. On-Chain Key Subscriptions (#942)
- Grants recurring content access based on holding minimum key balance thresholds.
- Enforces validity duration and allows extensions/renewals while balance maintained.
- Emits `SubscriptionGranted` and `SubscriptionRenewed`.

## 3. Cross-Key Atomic Swaps (#945)
- Direct key-to-key trading without curve routing in an atomic dual-sided transfer.
- Requires authorization from both parties and routes protocol swap fees.
- Emits `AtomicSwapExecuted`.

## 4. Multi-Currency Payment with Oracle Conversion (#932)
- Whitelists Stellar assets with oracle exchange rates to XLM.
- Enforces slippage protection on oracle conversion and curve execution.
- Emits `PaymentAssetConfigured`.
