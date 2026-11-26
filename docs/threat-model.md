# Threat model

**Repository:** accesslayer-contracts  
**Primary contract:** `creator-keys` (with `multisig-admin`, `referral-contract`, `lp-reward-contract`, `creator-keys-factory`, `treasury-contract`)  
**Status:** Draft for external audit  
**Owner:** Protocol security team (`security@accesslayer.org`)

## Purpose

This document describes the actors in the system, what each actor is trusted to do, the assets at risk, and the attack scenarios auditors and contributors should evaluate against. It is the authoritative reference for trust assumptions and is reviewed whenever those assumptions change (see the pull request checklist in `.github/PULL_REQUEST_TEMPLATE.md`).

The companion document [`docs/invariants.md`](./invariants.md) lists the runtime invariants that must always hold, each tied to at least one test, and a CI job fails when an invariant has no referencing test.

---

## 1. System overview (brief)

The `creator-keys` contract is a bonding-curve key marketplace. Creators register profiles, users buy and sell keys, and a configurable fee split routes a portion of each trade to the creator and a portion to the protocol treasury. Trading can be halted globally or per-key by a multisig admin set, prices can be published by an authorized oracle, and privileged config changes are gated behind the protocol admin (with optional timelocked, multi-sig approved upgrades).

## 2. Actors

| Actor | On-chain representation | Powers | Trusted to | Not trusted to |
|-------|-------------------------|--------|------------|----------------|
| **Admin** (protocol admin) | Stored `admin` address; `admin.require_auth()` on privileged entrypoints; changeable via `set_protocol_admin` | `set_fee_config`, `set_key_price`, `set_treasury_address`, `set_oracle_address`, `set_oracle_staleness_threshold`, `set_supply_milestones`, `set_protocol_fee_recipient`, `set_global_pause_admins`, `set_timelock_delay`, `set_governance_address`, propose timelocked upgrades | Act in the protocol's interest; keep the admin key secure; rotate the admin when compromised | Censor specific users beyond the global/per-key pause; extract user funds directly |
| **Governance** | Registered governance contract address (`read_governance_address`); `assert_is_admin_or_governance` allows governance or admin on some paths | `take_snapshot`, `apply_governance_violation`, reputation scoring | Snapshot state and record violations honestly | Modify balances, supply, or fee config directly |
| **Oracle** | Configured oracle address (`ORACLE_ADDRESS` storage key); `set_oracle_price` requires caller == configured oracle | Publish the current price and staleness flag via `set_oracle_price` | Publish accurate, timely prices; keep the oracle signing key secure | Set the oracle address itself (admin-only) or modify contract state beyond the price |
| **Keepers** (emergency-pause signers) | Global-pause admin set (`set_global_pause_admins`, `GLOBAL_PAUSE_THRESHOLD` distinct signers) | `pause_platform` (halt all trading immediately), `set_key_pause_override` (per-key halt), `queue_platform_resume`/`resume_platform` (24h timelock) | Halt trading only during genuine incidents; resume promptly after the timelock | Indefinitely freeze trading for profit or collusion below threshold |
| **Attestors** | No dedicated on-chain attester role exists today | Oracle prices are self-attested (signed by the configured oracle address); governance snapshots/violations are recorded off-chain | Provide truthful attestations of price and governance state | Exist as a separately authorized on-chain role |

> **Note on keepers and attestors:** The codebase does not define standalone `keeper` or `attestor` contracts. Keeper-like authority is exercised by the emergency-pause admin set (a 2-of-N multisig), and attestation is currently performed by the oracle (self-attested price publication) plus off-chain governance. This is recorded as a known limitation in Section 6.

## 3. Trust assumptions

### 3.1 Admin
- The admin private key is held by a trusted party / multisig and is not leaked.
- Admin changes (`set_protocol_admin`) are deliberate and authorized by the current admin.
- The admin will not configure a fee split that violates `creator_bps + protocol_bps == 10000` or `protocol_bps > 5000` (the contract enforces both, so a misconfiguration simply reverts).
- Timelocked upgrades (when used) require the configured delay and multi-sig approval before execution.

### 3.2 Governance
- The registered governance contract address is correct and controlled by a trusted party.
- Governance snapshots and violation records reflect genuine off-chain activity; there is no on-chain enforcement that a snapshot matches reality.

### 3.3 Oracle
- The configured oracle address is controlled by a trusted party that publishes accurate prices.
- The staleness threshold (`set_oracle_staleness_threshold`, admin-set) is set high enough that stale prices are flagged before they distort quotes materially.

### 3.4 Keepers (emergency-pause signers)
- At least `GLOBAL_PAUSE_THRESHOLD` distinct signers are honest and available to halt or resume trading.
- Signers collude only below the threshold; a paused market can always be resumed after the 24h timelock by the same admin set.

### 3.5 Attestors
- Off-chain governance attestations are truthful.
- Oracle price attestations are treated as potentially stale or wrong; consumers should check `is_stale` and the staleness threshold.

## 4. Assets at risk

| Asset | Where it lives | What protects it | Impact if compromised |
|-------|----------------|------------------|-----------------------|
| **Key supply** | Per-creator `total_supply` and holder balances (storage) | Supply conservation; buys/sells are atomic; failed trades revert with no mutation | Incorrect pricing, insolvency claims, inflated/deflated key value |
| **Reserves / treasury** | Protocol fee recipient balance (`get_protocol_recipient_balance`) | Fees credited only on the success path; treasury address admin-only | Fee theft or loss on failed trades |
| **Escrows / locked allocations** | Creator-locked allocations claimable only by the creator | `creator.require_auth()` on claim; non-creator reverts | Unauthorized claim or loss of locked creator funds |
| **Fees** | `creator_fee`, `protocol_fee`, `total_amount` on quotes; `fee::compute_fee_split` | Balance conservation (`creator + protocol == total`); remainder favors creator; `protocol_bps <= 5000` | Fee mis-split, protocol over-collection, dust loss |
| **Authorization / access control** | Admin address, oracle address, governance address, pause admin set | `require_auth()` guards; multisig thresholds; admin-only setters | Unauthorized config changes, trading halt, price manipulation |

## 5. Attack scenarios

| ID | Actor / threat | Asset | Scenario | Impact | Mitigation |
|----|----------------|-------|----------|--------|------------|
| THREAT-01 | Compromised admin key | All | Attacker with the admin key changes fees, price, treasury address, or halts trading | Funds rerouted; market manipulated or frozen | Multisig admin, key rotation via `set_protocol_admin`, monitoring of admin calls |
| THREAT-02 | Malicious oracle | Fees / supply | Oracle publishes a stale or inflated price; buy/sell quotes computed from it | Users over/under-pay; bonding curve distorted | Staleness flag + threshold; admin can rotate oracle address |
| THREAT-03 | Rogue keeper(s) | Supply / trading | Pause signers halt trading (denial of service) or refuse to resume after timelock | Trading frozen | `GLOBAL_PAUSE_THRESHOLD` (2-of-N); 24h resume timelock; same admin set resumes |
| THREAT-04 | Unauthorized caller | Authorization | Non-admin calls `set_fee_config`, `pause`, `set_oracle_address`, etc. | Reverted with `ContractError::Unauthorized`; no state mutation | `require_auth()` guards on every privileged path; state-unchanged-on-failure tests |
| THREAT-05 | Fee misconfiguration | Fees | Admin sets `creator_bps + protocol_bps != 10000` or `protocol_bps > 5000` | Config rejected before storage write | Contract enforces both constraints; no-op on unchanged config |
| THREAT-06 | Partial state mutation on failed trade | Reserves / supply | A failed buy or sell leaves fees credited or supply changed | Treasury/supply drift | Atomicity: failed trades revert fully; treasury-balance and supply-unchanged tests |
| THREAT-07 | Unauthorized locked-allocation claim | Escrows | Non-creator attempts to claim a locked allocation | Reverted | `creator.require_auth()` on claim; `NonCreator` revert path |
| THREAT-08 | Colluding pause signers below threshold | Trading | Fewer than `GLOBAL_PAUSE_THRESHOLD` signers attempt a pause | Rejected (`InsufficientSigners`) | Threshold enforcement in `assert_multisig` |

## 6. Known limitations and accepted risks

| ID | Limitation / accepted risk | Owner | Rationale |
|----|---------------------------|-------|-----------|
| LIMIT-01 | No dedicated on-chain `keeper` contract; keeper powers are the emergency-pause admin set | Protocol security team | Keeper behavior (halt/resume) is already covered by the 2-of-N pause multisig; a separate keeper contract is out of scope |
| LIMIT-02 | No dedicated on-chain `attestor`; oracle prices are self-attested and governance snapshots are off-chain | Protocol security team | Attestation is off-chain by design; on-chain price publication is bound to a single admin-configured oracle address |
| LIMIT-03 | Governance snapshots/violations are not cryptographically tied to off-chain votes | Governance working group | Enforcing off-chain vote proofs on-chain is deferred; reputation/violation logic is bounded by `GOVERNANCE_VIOLATION_CAP` |
| LIMIT-04 | Admin is a single address unless a multisig is configured externally; `set_protocol_admin` is admin-only | Protocol security team | Admin rotation is possible; adopting an on-chain multisig for the admin role is a future hardening item |
| LIMIT-05 | Stale-oracle window equals the configured staleness threshold; trades may execute on stale prices within that window | Protocol security team | The threshold is admin-tunable; consumers check `is_stale` before acting on quotes |

## 7. Review cadence

This threat model and `docs/invariants.md` must be reviewed by a maintainer whenever:
- trust assumptions change (new actor, changed powers, new multisig threshold),
- a privileged entrypoint is added, removed, or re-gated,
- or an invariant is added, removed, or re-scoped.

The review confirms the documents match current contract behavior. The PR checklist requires updating these documents when trust assumptions change.

---

*See also: [`docs/authorization-model.md`](./authorization-model.md), [`docs/error-codes.md`](./error-codes.md), [`docs/invariants.md`](./invariants.md).*
