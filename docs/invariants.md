# Invariant specification

**Repository:** accesslayer-contracts  
**Primary contract:** `creator-keys`  
**Status:** Draft for external audit  
**Owner:** Protocol security team (`security@accesslayer.org`)

## Purpose

This document lists the invariants that must always be true for the `creator-keys` contract. Every invariant has a unique id and is mapped to at least one test. The CI job [`scripts/check_invariants_tested.py`](../scripts/check_invariants_tested.py) fails the build when an invariant id has no referencing test.

Use this document together with [`docs/threat-model.md`](./threat-model.md), which covers actors, trust assumptions, assets at risk, and attack scenarios.

## How to read this document

- **Id** — Unique invariant id (`INV-<CATEGORY>-<NNN>`). Used by the CI check and referenced from tests.
- **Invariant** — What must always hold.
- **Category** — `supply`, `reserves`, `escrows`, `fees`, or `authorization`.
- **Test(s)** — Test file(s) and function name(s) that exercise the invariant. Test headers reference the ids they implement.

## 2. Invariant table

### Supply

| Id | Invariant | Test(s) |
|----|-----------|---------|
| INV-SPLY-001 | Total supply equals the sum of every holder balance for a creator (supply conservation). | `creator-keys/tests/supply_invariants.rs::test_supply_three_buyers_sum_equals_total`, `test_supply_multiple_buys_per_holder_sum_equals_total`, `test_supply_mixed_trades_three_participants`; `test_issues_487_489_492.rs` (`assert_supply_equals_holder_sum`) |
| INV-SPLY-002 | Total supply never goes negative; a sell cannot reduce supply below zero. | `creator-keys/tests/supply_invariants.rs::test_supply_never_goes_below_zero_after_all_sells` |
| INV-SPLY-003 | Buy then sell returns supply to its prior value (buy/sell conservation). | `creator-keys/tests/supply_invariants.rs::test_supply_buy_then_sell_returns_to_zero`, `test_supply_buy_two_sell_one_conserves_supply`, `test_supply_alternating_buys_and_sells` |
| INV-SPLY-004 | A failed sell leaves total supply unchanged. | `creator-keys/tests/supply_unchanged_after_failed_sell.rs::test_total_supply_unchanged_after_failed_sell_insufficient_balance` |
| INV-SPLY-005 | Supply changes for one creator do not affect another creator's supply (creator isolation). | `creator-keys/tests/supply_invariants.rs::test_supply_changes_for_one_creator_do_not_affect_another` |

### Reserves

| Id | Invariant | Test(s) |
|----|-----------|---------|
| INV-RESV-001 | Protocol treasury / recipient balance is unchanged after a failed trade (no fee credited on failure). | `creator-keys/tests/treasury_balance_unchanged_after_failed_trade.rs::test_treasury_balance_unchanged_after_failed_buy_insufficient_payment`, `test_treasury_balance_unchanged_after_failed_buy_unregistered_creator`, `test_treasury_balance_unchanged_after_failed_sell_insufficient_balance` |

### Escrows

| Id | Invariant | Test(s) |
|----|-----------|---------|
| INV-ESCW-001 | A locked allocation can only be claimed by its creator; a non-creator claim reverts. | `creator-keys/tests/claim_locked_allocation_non_creator_reverts.rs` |
| INV-ESCW-002 | Claimed locked-allocation views are consistent across the ledger boundary. | `creator-keys/tests/claim_locked_allocation_ledger_boundary.rs`, `creator-keys/tests/locked_allocation_claimed_view_regression.rs` |
| INV-ESCW-003 | Locked allocation supply is accounted consistently with the bonding curve (no double counting). | `creator-keys/tests/locked_allocation_bonding_curve_supply.rs` |

### Fees

| Id | Invariant | Test(s) |
|----|-----------|---------|
| INV-FEE-001 | Sell fee conservation: `creator_fee + protocol_fee + total_amount == price` and `creator_fee + protocol_fee == price - total_amount`. | `creator-keys/tests/sell_fee_split_invariants.rs` (all tests) |
| INV-FEE-002 | Fee balance conservation: `creator_amount + protocol_amount == total` for every split. | `creator-keys/tests/fee_rounding_invariants.rs` (all tests) |
| INV-FEE-003 | The remainder from integer division goes to the creator, never lost. | `creator-keys/tests/fee_rounding_invariants.rs::fee_split_remainder_favors_creator_at_boundary_999`, `fee_split_remainder_favors_creator_at_boundary_1001` |
| INV-FEE-004 | Protocol share never exceeds 50%: `protocol_bps <= 5000`. | `creator-keys/tests/sell_fee_split_invariants.rs::sell_fee_split_invariant_max_protocol_50_percent`; `set_fee_config` constraint in contract |
| INV-FEE-005 | Fee config sums to the whole: `creator_bps + protocol_bps == 10000`. | `set_fee_config` constraint in contract; `creator-keys/tests/sell_fee_split_invariants.rs::sell_fee_split_invariant_across_multiple_fee_configs` |
| INV-FEE-006 | Co-creator split conserves the creator fee: `co_creator_amount + creator_recipient_amount == total_creator_fee`. | `creator-keys/tests/co_creator_fee_split_invariant.rs` |

### Authorization

| Id | Invariant | Test(s) |
|----|-----------|---------|
| INV-AUTH-001 | Admin-only functions reject non-admin callers with `ContractError::Unauthorized`. | `creator-keys/tests/admin_unauthorized.rs` (all tests) |
| INV-AUTH-002 | A rejected privileged call leaves contract state unchanged. | `creator-keys/tests/admin_unauthorized.rs::test_pause_no_state_change_on_non_admin_call` |
| INV-AUTH-003 | Oracle price updates are accepted only from the configured oracle address; unauthorized updates revert. | `creator-keys/tests/price_oracle.rs`; `creator-keys/src/test_issues_904_905_906_908.rs::test_oracle_unauthorised_update_is_rejected`, `test_oracle_address_update_is_admin_only` |
| INV-AUTH-004 | Emergency pause/resume requires `GLOBAL_PAUSE_THRESHOLD` distinct signers from the admin set. | `creator-keys/tests/emergency_pause_blocks_trading.rs`; `creator-keys/src/test_issue_1000.rs::pause_platform_requires_every_signer_to_authorise` |
| INV-AUTH-005 | The protocol admin can be read and transferred only by (or to) the configured admin. | `creator-keys/tests/protocol_admin.rs` |

## 3. Known limitations and accepted risks

| Id | Limitation / accepted risk | Owner | Rationale |
|----|---------------------------|-------|-----------|
| INV-LIMIT-01 | No on-chain invariant test directly covers every read-only view's return-type stability. | Protocol security team | View stability is covered by `docs/read-only-methods.md` and event-convention tests rather than invariant ids. |
| INV-LIMIT-02 | Cross-contract invariants involving `multisig-admin` or `treasury-contract` are not yet enumerated here. | Protocol security team | This specification is scoped to `creator-keys`; multi-contract invariants are a follow-up. |
| INV-LIMIT-03 | Invariant ids are referenced from test headers but not asserted programmatically inside test bodies. | Protocol security team | The CI check enforces the id-to-test mapping at the repository level; per-test assertions are a future hardening item. |

## 4. Adding or changing an invariant

1. Add a row to the table in Section 2 with a new unique id (`INV-<CATEGORY>-<NNN>`).
2. Add or update at least one test that exercises the invariant, and reference the id in the test's `//!` header (for example `//! Implements INV-SPLY-001.`).
3. Run `python scripts/check_invariants_tested.py` (also run by CI) and confirm it exits 0.
4. Update `docs/threat-model.md` if the invariant changes a trust assumption or asset at risk.

---

*See also: [`docs/threat-model.md`](./threat-model.md), [`docs/authorization-model.md`](./authorization-model.md).*
