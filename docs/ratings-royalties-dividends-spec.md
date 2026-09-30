# Specification: Key Ratings, Secondary Royalties, Holder Dividends & Dynamic Fees

## 1. Key Rating System (#946)
- Allows authenticated key holders holding at least 1 key to rate creators with scores from 1 to 5.
- Aggregates running sum and count atomically; overwriting previous rating from the same wallet.
- Emits `KeyRated` event with new score, updated running average (scaled by 100), and total rating count.

## 2. Configurable Creator Royalty (#950)
- Allows key creators to configure a secondary transfer royalty fee (up to 2500 bps = 25%).
- Computes and forwards royalty amount to the creator atomically on secondary transfers.
- Emits `RoyaltyPaid` event with creator, sender, and royalty amount.

## 3. Holder Dividend Distribution (#944)
- Revenue-sharing pool for trading fees distributed pro-rata to key holders.
- Tracked per cycle with duplicate claim protection within the same cycle.
- Emits `DividendDistributed` and `DividendClaimed` events.

## 4. Dynamic Volume-Based Fee Adjustment (#943)
- Protocol fee percentage dynamically adjusts based on 24-hour trading volume thresholds.
- Rewards high-volume trading with discounted fee tiers.
- Emits `FeeTierChanged` event upon tier boundary transition.
