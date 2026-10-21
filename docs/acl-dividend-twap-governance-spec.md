# Specification: ACL Whitelist, Dividend Pool, TWAP Oracle & Governance Proposals

## 1. Cross-Contract ACL Whitelist (#972)
- Maintains an authorized whitelist of approved external contract addresses and permitted function selectors.
- Restricts whitelist modifications (`add_to_acl`, `remove_from_acl`) to multi-sig / protocol admin only.
- Exposes `is_permitted(contract_address, function_selector)` view check supporting exact selector matches and wildcard (`all`/`wildcard`) permissions.
- Exposes `get_acl()` returning the full active whitelist with permitted function sets.
- Emits `ACLUpdated` event on every add and remove.

## 2. Dividend Pool with Pro-Rata Distribution (#971)
- Enables creators to deposit distribution funds into a dedicated dividend pool across sequential epochs.
- Records holder key balances and total circulating supply at distribution time via `snapshot_holdings`.
- Computes pro-rata claimable shares accurately (`(deposit_amount * holder_balance) / total_snapshot_supply`).
- Prevents double-claiming per epoch per holder and tracks pending claimable balances in real time.
- Emits `DividendDeposited` and `DividendClaimed` events.

## 3. TWAP Oracle for Bonding Curve (#968)
- Accumulates price-time products on bonding curve buy and sell trades.
- Exposes `get_twap(key_id, window_seconds)` for configurable time windows (e.g. 1h, 4h, 24h).
- Enforces minimum observation count guard before TWAP computations are valid.
- Prunes historical observations beyond the maximum window (e.g. 24h) to prevent unbounded storage growth.
- Guards against wash trades within the same block by updating price without advancing time or inflating cumulative products.
- Emits `TWAPUpdated` event on each trade.

## 4. Governance Proposals with Token-Weighted Voting (#969)
- Allows token holders meeting minimum holding eligibility thresholds to create parameter change proposals.
- Supports token-weighted voting with strict double-voting prevention.
- Enforces quorum and approval threshold requirements before proposal execution.
- Transitions through complete proposal lifecycle states: `Pending`, `Active`, `Passed`, `Failed`, and `Executed`.
- Emits `ProposalCreated`, `ProposalVoted`, and `ProposalExecuted` events.
