# Emergency Pause Runbook

For incident roles, severity, communication, and the first-hour checklist, see
the [incident response runbook](runbooks/incident.md). The safer-by-default
command-line wrappers and playbooks are also documented there.

The `CreatorKeysContract` exposes `pause(admin)` and `unpause(admin)` entry points
that allow the protocol admin to halt all state-mutating operations without a contract
upgrade or migration. This document describes when to use them, how to execute them
safely, and what conditions must hold before calling `unpause`.

---

## 1. Conditions That Justify an Emergency Pause

Pausing is a last resort. Trigger it when any of the following is confirmed or
strongly suspected:

| Trigger | Example |
|---|---|
| **Active exploit in a write path** | An attacker is draining the bonding-curve pool or minting supply without payment |
| **Critical logic bug discovered** | A fee calculation produces a negative payout; a division-by-zero path is reachable in production |
| **Treasury / fee-recipient drain** | Protocol fee balance is decreasing at an anomalous rate not explained by legitimate trades |
| **Unauthorized admin change** | `set_protocol_admin` was called by an unexpected account |
| **Oracle / price manipulation** | An upstream price feed is being exploited to affect buy/sell quotes |

Do **not** pause for:

- Routine upgrades — use the upgrade path instead
- Network congestion or high fees — not a contract-level concern
- Single failed transaction by a legitimate user

---

## 2. Who Can Pause

Only the address stored in the contract's `protocol_admin` storage key can call
`pause` or `unpause`. Any other caller receives `ContractError::Unauthorized` and
the call is reverted on-chain.

The admin address is set via `set_protocol_admin(caller, new_admin)` where `caller`
must be the current admin (or the admin bootstrapping the first admin during
initialization).

---

## 3. What Is Blocked While Paused

All state-mutating operations return `ContractError::ProtocolPaused` immediately:

- `buy_key` — purchasing creator keys
- `sell_key` / `buyback` — selling or buying back creator keys
- `register_creator` — new creator registration
- `distribute_dividend` / `claim_dividend` — dividend operations
- `transfer_keys` — peer-to-peer key transfers
- Any future write entry points added to the contract

**Read-only views continue to work** while paused:

- `get_is_paused`, `is_creator_registered`, `get_total_key_supply`
- `get_key_balance`, `get_creator_details`, `get_protocol_fee_view`
- `get_buy_quote`, `get_sell_quote` (price simulation only — no state changes)
- All other `get_*` / `is_*` view methods

This means monitoring dashboards, explorers, and read-only integrations remain
operational and can be used to diagnose the issue while the contract is paused.

---

## 4. How to Pause

1. **Confirm authorized signing is available** — use the approved Stellar CLI
   key alias for the deployed network. Never paste or pass a secret key through
   a command, environment variable, source file, chat, or log. Never use a hot
   key for the admin on mainnet.

2. **Invoke `pause(admin)` on-chain:**

   Using the Stellar CLI:
   ```sh
   stellar contract invoke \
     --id <CONTRACT_ID> \
     --source <ADMIN_KEY_ALIAS> \
     --network <mainnet|testnet> \
     --send=yes \
     -- pause \
     --admin <ADMIN_STELLAR_ADDRESS>
   ```

   Prefer the confirmation-gated repository wrapper:
   ```sh
   bash scripts/emergency/pause.sh \
     --network <network> --contract <CONTRACT_ID> \
     --source <ADMIN_KEY_ALIAS> --admin <ADMIN_STELLAR_ADDRESS> --execute
   ```
   If an SDK integration is required, use the approved signer service; do not
   load a raw secret into the application process.

3. **Verify the pause is in effect:**
   ```sh
   stellar contract invoke \
     --id <CONTRACT_ID> \
     --network <mainnet|testnet> \
     --send=no \
     -- get_is_paused
   ```
   Expected output: `true`

4. **Communicate to users** — post a status update on the protocol's public channels
   (Discord, Twitter/X, status page) explaining that the contract is temporarily
   paused and which functions are affected. Do not claim funds are safe unless
   that has been verified. Avoid disclosing exploit details until a fix is deployed.

---

## 5. Steps While the Contract Is Paused

1. **Preserve state** — query all relevant on-chain state (`get_key_balance`,
   `get_creator_details`, fee balances) immediately after pausing and save a snapshot.

2. **Reproduce the issue** — reproduce the bug in a forked testnet or local `soroban`
   environment. Do not run exploits against the live contract even while paused.

3. **Assess impact** — determine which accounts were affected, what amounts are at
   risk, and whether the bug can be triggered retroactively against existing state.

4. **Prepare a fix** — implement and test the fix. The fix must pass:
   - All existing Soroban unit tests (`cargo test`)
   - A regression test that specifically covers the exploit scenario
   - Manual review by at least one additional engineer

5. **Deploy the fix** — use the `upgrade(new_wasm_hash)` entry point (admin-only)
   to upgrade the contract WASM in place. Soroban upgrades preserve all storage state.
   Verify the new WASM hash matches the expected build output before invoking.

6. **Test on testnet first** — deploy the fixed WASM to testnet, reproduce the full
   attack scenario, confirm it is blocked, and confirm legitimate operations work.

---

## 6. Criteria That Must Hold Before Calling `unpause`

Do **not** unpause until all of the following are true:

- [ ] The vulnerability is **fully understood** — root cause identified, not just
  symptoms suppressed
- [ ] A **fix is deployed** on mainnet (not just testnet) via `upgrade`
- [ ] A second engineer has **independently verified** the fix and regression
  scenario on testnet and confirmed the expected deployed WASM hash on mainnet
- [ ] Any **affected users have been identified** and a remediation plan exists (even
  if not yet executed)
- [ ] A **post-mortem draft** is ready to publish within 24 hours of unpausing
- [ ] **Key rotation has been considered** — if the admin key or any privileged key
  may have been exposed, rotate before unpausing

If the fix cannot be deployed in time (e.g. requires a breaking state migration),
keep the contract paused and evaluate a controlled migration to a new contract
deployment with state replay.

---

## 7. How to Unpause

Once all criteria above are satisfied:

```sh
bash scripts/emergency/safe-mode.sh unpause \
  --network mainnet --contract <CONTRACT_ID> \
  --source <ADMIN_KEY_ALIAS> --admin <ADMIN_STELLAR_ADDRESS>
```

Review the dry run and add `--execute`; confirm the exact network and contract
when prompted.

Immediately verify:
```sh
stellar contract invoke --id <CONTRACT_ID> --network mainnet --send=no -- get_is_paused
# Expected: false
```

Use read-only health checks and monitor normal operations before announcing
recovery. Do not submit a test trade to mainnet unless the incident commander
explicitly approves the risk; exercise transaction-level smoke tests on the
designated testnet.

---

## 8. Warning: Never Unpause Without a Deployed Fix

Unpausing before the vulnerability is fixed re-exposes users to the same exploit.
Even under stakeholder pressure, **pausing again after an early unpause is worse
than staying paused** — it signals to the ecosystem that the protocol is reactive
rather than deliberate.

If you are uncertain whether the fix is sufficient, keep the contract paused and
consult additional reviewers before proceeding.

---

## 9. Multisig Platform Trading Halt (#1000)

Separate from the single-admin `pause`/`unpause` above, the multisig admin set
configured with `set_global_pause_admins` can halt **bonding-curve trading only**
(every buy and sell across all keys) without blocking other entry points.

Every call below takes `signers: Vec<Address>`: at least **two distinct**
members of the global-pause admin set, each of whom must sign the transaction.
Fewer signers, a repeated signer, or a non-member fails with
`EmergencyPauseError::{InsufficientSigners, DuplicateSigner, Unauthorized}`.

| Entry point | Effect |
|---|---|
| `pause_platform(signers)` | Halts trading immediately; emits `plat_pau` with `actor` and `timestamp` |
| `queue_platform_resume(signers)` | Starts the 24h timelock; returns the executable timestamp; emits `plat_rq` |
| `resume_platform(signers)` | Lifts the halt once the timelock has elapsed; emits `plat_res` |
| `set_key_pause_override(signers, key_id, paused)` | Halts or releases a single key; emits `key_pau` |
| `is_paused()` / `get_platform_resume_eta()` / `is_key_paused(key_id)` | Read-only views |

While halted, `buy_key`, `buy_keys`, `buy_key_with_referrer`,
`buy_keys_with_referrer`, `sell_key`, `batch_buy`, `batch_buy_v2` and
`batch_sell` return `ContractError::GlobalTradingHalted`.

Operational notes:

- `resume_platform` before the queued timestamp fails with
  `TimelockNotElapsed`; use the 24h window to complete the checklist in
  section 6.
- The per-key override is independent of the platform halt: resuming the
  platform does not release an overridden key, and clearing a key's override
  does not let it trade while the platform is halted.
- `is_paused` reports only this platform halt. The legacy protocol pause is
  still reported by `get_is_paused`.
