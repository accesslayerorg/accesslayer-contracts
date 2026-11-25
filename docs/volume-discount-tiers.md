# Volume Discount Tier Evaluation Order and Tie Resolution

This document specifies the evaluation order, tie-resolution rules, and state invariants for cumulative volume discount tiers in the `creator-keys` contract.

## Overview

The contract supports volume-based fee discounts that lower the protocol fee rate as a creator accumulates trading volume. When multiple discount tiers are configured, the contract determines the active tier by evaluating the creator's volume against tier thresholds.

## Evaluation Rules

1. **Ascending Order of Thresholds**:
   - Tiers are configured and evaluated in strictly ascending order of their minimum volume threshold (`volume_threshold` / `min_volume`).
   - The contract verifies that each subsequent tier has a strictly greater threshold than the preceding tier: `tier[i].volume_threshold < tier[i+1].volume_threshold`.
   - The base tier (index 0) begins at volume 0 (`volume_threshold == 0`).

2. **Last Matching Tier Wins**:
   - Evaluation iterates sequentially through the tiers in ascending order:
     ```rust
     fn active_tier_index(tiers: &Vec<FeeTier>, vol: i128) -> u32 {
         let mut idx: u32 = 0;
         for (pos, tier) in tiers.iter().enumerate() {
             if vol >= tier.volume_threshold {
                 idx = pos as u32;
             } else {
                 break;
             }
         }
         idx
     }
     ```
   - Because tiers are arranged in ascending threshold order, the iteration stops as soon as a tier's threshold exceeds the creator's volume.
   - The **last matching tier** (the highest threshold met or exceeded) is selected.
   - This guarantees that when a creator qualifies for multiple discount tiers, the tier offering the lowest fee rate (highest tier index) is applied.

## Worked Example

Consider a configuration with three volume discount tiers:

| Tier | Min Volume Threshold | Protocol Fee Rate | Notes |
|:-----|:---------------------|:------------------|:------|
| Tier 1 | 1,000 XLM | 800 bps (8.0%) | Entry discount |
| Tier 2 | 5,000 XLM | 600 bps (6.0%) | Intermediate discount |
| Tier 3 | 10,000 XLM | 400 bps (4.0%) | Top discount |

### Scenario: Creator at 7,000 XLM Cumulative Volume

A creator with **7,000 XLM** in cumulative trading volume is evaluated against the tiers:
1. `7,000 XLM >= 1,000 XLM` -> Qualifies for **Tier 1** (matches).
2. `7,000 XLM >= 5,000 XLM` -> Qualifies for **Tier 2** (matches, supersedes Tier 1).
3. `7,000 XLM < 10,000 XLM` -> Does not qualify for **Tier 3** (evaluation stops).

**Result**: The creator qualifies for the first two tiers (Tier 1 and Tier 2). Following the **last matching tier wins** rule, the contract selects **Tier 2**, and the creator receives the Tier 2 fee rate of **600 bps (6.0%)**.

## Non-Retroactivity

- Updating discount tier definitions (via admin configuration) or advancing to a higher tier by accumulating volume **takes effect on the next trade**.
- Tier changes are **never applied retroactively** to past trades or previously collected fees.
- Quotes reflect the active tier at query time, and the fee applied during execution is determined by the state at trade execution.

## Invariant: Retention of Earned Tier

- Cumulative volume is monotonically non-decreasing over time (`volume >= prev_volume`).
- In any theoretical scenario where a creator's active volume measurement could drop below an earned threshold (e.g., in rolling-window models or window shifts), the contract enforces that an earned tier is retained and does not regress to a higher fee rate.
- Once a discount rate has been earned, a creator is never penalized retroactively.

## Related Documentation and Code References

- Fee configuration and calculations: [`docs/fee-assumptions.md`](./fee-assumptions.md)
- Storage layout and constants: [`docs/storage-layout.md`](./storage-layout.md)
- Implementation source: [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs) (`DiscountTier`, `FeeTier`, `active_tier_index`, `apply_trade_volume`)
