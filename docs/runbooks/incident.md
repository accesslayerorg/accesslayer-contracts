# Incident response runbook

Use this runbook for the first hour of a suspected security incident or
production outage. Favor preserving evidence and limiting additional impact
over restoring service quickly. Do not promise that funds are safe until the
impact has been established.

## Severity

| Level | Examples | Initial response |
|---|---|---|
| **SEV-0 — Critical** | Active loss of assets, exploitable write path, confirmed privileged-key compromise, or multiple users at immediate risk | Declare immediately; page the full response group; evaluate emergency pause controls without waiting for a complete root-cause analysis |
| **SEV-1 — High** | Reserve insolvency, materially incorrect oracle pricing, widespread stuck transactions/keeper work, or sustained failure of a critical protocol function | Declare and page protocol operations and security; mitigate within minutes and escalate to SEV-0 if loss or exploitability is confirmed |
| **SEV-2 — Degraded** | Isolated keeper delay, stale data with no unsafe execution, or a recoverable partial outage | Assign an incident lead, monitor impact, and escalate if scope or risk increases |

When uncertain between levels, use the higher level until evidence rules it
out. Record the declaration time, evidence, and severity changes in the incident
log.

## Roles and escalation

Assign named people in the incident channel; one person may cover more than one
role, but never let an unassigned role be assumed.

1. **Incident commander (IC):** declares severity, assigns tasks, maintains the
   timeline, and makes the pause/continue decision after consulting security.
2. **Security lead:** validates exploit/key-compromise signals, preserves
   evidence, coordinates independent review, and advises on containment.
3. **Protocol operator / key custodian:** checks deployment and network state,
   executes only authorized transactions, verifies each transaction, and
   records ledger/transaction identifiers. The custodian does not disclose
   secrets or paste them into shell commands, chat, logs, or this repository.
4. **Communications lead:** posts approved user and stakeholder updates and
   maintains the next-update time.
5. **Engineering lead:** reproduces the failure in local/testnet environments,
   prepares the fix, and coordinates a second-person review.

Escalation order: **on-call operator → incident commander → security lead →
protocol/key custodian → engineering lead → communications lead**. For SEV-0,
page the full group concurrently; do not wait for a response from one role
before contacting the next. If the IC or security lead is unreachable, the
first available on-call operator declares the incident and assigns a temporary
IC. Record unanswered contacts and continue containment.

Use the team’s approved incident channel and paging directory for real contact
details. Do not put personal contact information, private keys, seed phrases, or
RPC credentials in this document.

## First-hour procedure

### 0–5 minutes: declare and preserve

- Open an incident channel, declare a provisional severity, and assign the IC,
  security, operator, engineering, and communications roles.
- Record UTC time, network, affected contract IDs, observed ledger/transaction
  IDs, alert source, and first known impact. Preserve relevant alerts and
  read-only RPC results; do not alter or delete evidence.
- Pause deployment, scheduled upgrades, nonessential admin operations, and
  keeper retries that may make the state harder to reason about.
- Decide whether activity is unsafe now. For an actively exploitable write
  path, prioritize the hard protocol pause over collecting a complete
  diagnosis.

### 5–15 minutes: contain

- Verify the deployed contract ID and current admin/control configuration using
  trusted read-only sources.
- Choose the narrowest control that blocks the dangerous operation:
  - `scripts/emergency/pause.sh` calls `pause(admin)` and blocks contract
    state-changing operations. This is admin-authorized and does not require
    an additional multisig threshold.
  - `scripts/emergency/safe-mode.sh pause` calls `pause_all` with a bounded
    expiry (1 second to 7 days). It is appropriate when the timed trading
    guard is sufficient; it is **not** a substitute for the all-write pause.
  - The on-chain multisig `global_pause` requires two distinct configured
    admins and stops buys/sells. The separate `pause_platform` control also
    stops buys/sells and requires its signer vector. Use the existing
    [emergency pause runbook](../emergency-pause-runbook.md) for those flows.
- Never unpause as a test of whether a mitigation worked. Use the testnet drill
  only against a designated testnet deployment.
- If an admin key may be compromised, do not use it to transact. Use an
  independent authorized control path and escalate to the key custodian and
  security lead.

### 15–30 minutes: snapshot and scope

- Once containment is verified, use `scripts/emergency/snapshot.sh` to export
  pause state, treasury/reward balances, reserves, and specified account
  balances and pending records. The exporter records the ledger sequence/hash
  and refuses to write a snapshot if the ledger changes during collection.
  Configure `STELLAR_RPC_URL` to the same endpoint used by the selected Stellar
  CLI network. If the ledger advances, retry; never label partial output as a
  consistent snapshot.
- Identify affected contracts, creators, wallets, reserves, transactions, and
  the earliest potentially affected ledger. Preserve the raw command log and
  snapshot with restricted access.
- Reconcile observed balances/liabilities against a known-good prior state.
  Separate verified facts from hypotheses in the timeline.

### 30–60 minutes: choose recovery path

- Engineering reproduces the issue locally or on testnet and prepares a
  regression test. Do not probe a live contract with exploit transactions.
- Security and a second engineer review the mitigation and affected-state
  analysis. Check whether a contract upgrade, key rotation, keeper restart, or
  user remediation is required.
- IC records the go/no-go decision. Keep the protocol paused if the cause,
  impact, or fix remains uncertain.
- Communications lead posts a factual status update with affected
  functionality, mitigation status, and the next update time. Do not publish
  exploit details that would increase user risk.
- Continue recording actions, authorizations, transaction hashes, verification
  results, decisions, and owners. Schedule a post-incident review and preserve
  the incident log.

## Playbooks

### Stale or suspect oracle

**Detection signals**

- Oracle timestamp exceeds the configured freshness limit or price updates stop.
- Oracle and independent market/reference sources diverge beyond the expected
  tolerance.
- Quotes jump without corresponding market activity, or oracle update events
  fail/revert repeatedly.

**Actions**

1. Confirm ledger timestamps and feed health from independent read-only
   sources; rule out delayed indexing before changing contract state.
2. Stop keeper retries that could publish stale values. If an unsafe quote can
   still be consumed, use the narrowest effective pause; use the protocol pause
   if other writes are also unsafe.
3. Capture a ledger-consistent snapshot and preserve feed payloads, update
   transactions, and affected quote/trade identifiers.
4. Restore the oracle only from a trusted source; do not manually inject a
   price without the documented authorization and review process.

**Verification**

- Feed timestamp is current and independent sources are within the configured
  tolerance for the agreed observation window.
- A read-only quote matches the expected calculation; no stale update is being
  accepted.
- A testnet regression reproduces the stale case and confirms it is rejected.
- Resume only after security and engineering sign off and the IC records the
  decision.

### Drained or insolvent reserve

**Detection signals**

- Reserve balance declines unexpectedly or becomes less than tracked
  liabilities.
- Reserve events do not reconcile with authorized accrual/deployment activity.
- Repeated failed payouts or a sharp unexplained balance change appears at a
  specific ledger.

**Actions**

1. Pause the affected operation; use the all-write protocol pause if the drain
   can continue through unrelated entry points.
2. Capture reserves, treasury/reward balances, affected account balances, and
   pending records at one ledger. Preserve transaction/event IDs and reconcile
   liabilities independently.
3. Disable implicated keeper/automation credentials and review admin activity.
   Do not transfer or replenish funds until the root cause and authorization
   are understood.
4. Identify impacted users and prepare a reviewed remediation plan.

**Verification**

- Reserve and liabilities reconcile to the same ledger and the cause of every
  discrepancy is documented.
- The vulnerable path fails in a testnet regression and authorized reserve
  operations still work.
- Independent reviewer confirms the deployed fix and recovery amounts before
  the IC considers resuming.

### Admin key compromise

**Detection signals**

- Unrecognized admin-authenticated transaction, changed admin/configuration, or
  unexpected signing activity.
- Secret material appears in a system, log, device, or message accessible to
  an unauthorized person.
- Key custodian cannot establish exclusive control of the signing environment.

**Actions**

1. Declare SEV-0. Do not use the suspected key or copy it into a CLI command,
   log, issue, or chat. Preserve the transaction and access evidence.
2. Use an independent, already-authorized multisig or emergency control to
   stop unsafe activity. If no independent authority exists, escalate to
   security and governance immediately; do not pretend that the compromised
   key is safe.
3. Review recent admin actions and affected contracts. Rotate/revoke the key
   only through the documented governance process and after confirming the
   replacement authority is independent and available.
4. Keep affected operations paused while reviewing deployments, signers,
   automation, and any external systems that held the credential.

**Verification**

- The compromised key is no longer authorized on every affected control path;
  verify on-chain admin and multisig membership from a trusted read-only
  source.
- New signing authority is independently tested on testnet and safeguarded by
  the approved custody process.
- Security confirms there are no unexplained admin transactions after
  rotation. Do not resume until the IC approves.

### Stuck keeper or delayed work

**Detection signals**

- Expected keeper heartbeat/ledger progress stops or repeatedly fails.
- Pending work remains unchanged beyond its documented processing window.
- Retries produce duplicate submissions, sequence errors, or growing backlog.

**Actions**

1. Determine whether the keeper is only delayed or is submitting unsafe or
   duplicate operations. A delay alone does not justify a protocol-wide pause.
2. Stop automated retries if they may duplicate state changes; preserve logs,
   pending identifiers, and last successful ledger.
3. Check network/RPC health and signer availability without exposing
   credentials. Restart only after identifying the stuck operation and
   confirming idempotency/replay behavior.
4. Escalate to a narrow pause only if the stuck work can cause loss or unsafe
   state changes.

**Verification**

- Keeper resumes from the last confirmed state without duplicate effects.
- Pending work is resolved or has a documented owner and retry plan.
- Heartbeat and ledger progress remain healthy through the agreed observation
  window; verify balances and relevant pending records.

## Script safety and testnet drill

All emergency scripts are dry-run by default, display the commands they would
run, and log commands/results to
`$HOME/.local/state/accesslayer/incident-commands.log` with restrictive file
permissions. Set `INCIDENT_LOG` to choose another restricted log path. Logs
include command arguments and output; do not pass secrets or credentials to a
script. Use a configured Stellar CLI key alias with `--source`; never pass a
secret key as an argument or environment value.

Examples (replace the IDs and alias with approved testnet values):

```sh
bash scripts/emergency/pause.sh \
  --network testnet --contract <CONTRACT_ID> \
  --source incident-testnet --admin <ADMIN_ADDRESS>

bash scripts/emergency/safe-mode.sh pause \
  --network testnet --contract <CONTRACT_ID> \
  --source incident-testnet --admin <ADMIN_ADDRESS> \
  --reason "testnet incident drill" --duration-seconds 3600

bash scripts/emergency/snapshot.sh \
  --network testnet --contract <CONTRACT_ID> \
  --source incident-testnet --output incident-snapshot.json
```

Before any submission, rerun with `--execute` and confirm the exact contract
or drill string at the interactive prompt. `testnet-drill.sh` is hard-coded to
the testnet network and exercises `pause`, verifies paused, calls `unpause`,
then verifies resumed:

```sh
bash scripts/emergency/testnet-drill.sh \
  --contract <TESTNET_CONTRACT_ID> \
  --source incident-testnet --admin <TESTNET_ADMIN_ADDRESS>

# After reviewing the dry run and confirming the contract is not already paused:
bash scripts/emergency/testnet-drill.sh \
  --contract <TESTNET_CONTRACT_ID> \
  --source incident-testnet --admin <TESTNET_ADMIN_ADDRESS> --execute
```

A drill passes only when both on-chain state checks match the expected values;
otherwise it exits nonzero and preserves the paused state if recovery did not
complete. Record the date, network, contract ID, transaction hashes, ledger,
operator, and outcome in the team’s approved drill record. Run and record this
drill against the designated testnet deployment before relying on the scripts
for an incident. Never point a drill at mainnet.
