# Static analysis and dependency policy in CI

This repository runs four security checks on every pull request, a weekly
dependency audit, and a check that every GitHub Action is pinned to an immutable
commit SHA. This document explains what each check enforces, how to triage a
finding, and how to suppress a finding correctly.

The checks exist so that a reviewer does not have to notice a risky dependency,
a disallowed license, an unexpected dependency source or a new Soroban
anti-pattern by hand.

## What runs where

| Workflow | Trigger | Job | Fails the build when |
| --- | --- | --- | --- |
| `.github/workflows/ci.yml` | push to `main`, pull request | `verify` | formatting, clippy or tests fail |
| `.github/workflows/security.yml` | push to `main`, pull request | `scout` | a **new** Scout finding at or above the severity threshold (default: `critical`) |
| | | `dependency-policy` | cargo-deny finds a vulnerability, yanked crate, disallowed license or disallowed source |
| | | `cargo-audit` | cargo-audit reports an advisory that is not covered by a reviewed suppression |
| | | `actions-pinned` | a workflow uses an action by tag or branch instead of a commit SHA |
| | | `policy-scripts` | the security policy scripts' unit tests fail |
| `.github/workflows/malware-scan.yml` | pull request | `scan` | a known malware pattern is detected in the diff |
| `.github/workflows/security-audit-weekly.yml` | Mondays 06:00 UTC, manual | `audit` | an unsuppressed advisory is present; new advisories get a tracking issue |

All workflows run with `contents: read`. Only the weekly audit adds
`issues: write`, because it opens tracking issues.

## Reports

Every job uploads its reports as workflow artifacts (Actions run page →
*Artifacts*):

| Artifact | Contents | Retention |
| --- | --- | --- |
| `scout-report` | `scout-report.json` (raw Scout report) and `scout-summary.md` (severity table and blocking findings) | 30 days |
| `cargo-deny-report` | `cargo-deny.txt` (human readable) and `cargo-deny.json` | 30 days |
| `cargo-audit-report` | `cargo-audit.txt` and `cargo-audit.json` | 30 days |
| `cargo-audit-weekly-report` | the same files from the scheduled run | 90 days |

The Scout job also writes the same summary to the GitHub Actions job summary so
the blocking findings are visible without downloading anything.

## 1. Scout for Soroban

[Scout](https://github.com/CoinFabrik/scout-audit) is a static analyzer for
Soroban contracts. CI installs the pinned version (`cargo-scout-audit 0.3.17`)
and runs it over the workspace.

Scout 0.3.17 resolves Dylint 4.1.2, whose build currently requests the removed
`master` branch of `rust-clippy` (upstream issue
[CoinFabrik/scout-audit#350](https://github.com/CoinFabrik/scout-audit/issues/350)).
CI mirrors the repository's default branch and provides a temporary local
`master` alias so Dylint can continue checking out the toolchain-specific
historical commit. This compatibility step does not skip detectors or change
the finding threshold; remove it once Scout ships a compatible Dylint version.

Scout's severities are **Critical**, **Medium**, **Minor** and **Enhancement**.
There is no separate "high" label: Critical is the tool's high-severity bucket,
so the gate fails on new Critical findings. The threshold can be lowered in
`.github/workflows/security.yml` (`--fail-on medium`) if the team decides Medium
findings should block as well.

### The reviewed baseline

`.github/security/suppressions.toml` holds the reviewed baseline. An entry
matches one reviewed occurrence of a detector:

```toml
[[scout]]
detector = "unsafe-unwrap"
file = "creator-keys/src/lib.rs"
fingerprint = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
reason = "The value is written by the initializer directly above and cannot be None."
owner = "@accesslayerorg"
expires = "2027-06-30"
ticket = "#1319"
```

The fingerprint is a SHA-256 over the detector id, the repository-relative file
path and the whitespace-normalised code snippet. That means:

* reformatting the file does not invalidate the review,
* editing the flagged code produces a new fingerprint, so the finding becomes
  unreviewed again and must be re-reviewed,
* moving the finding to another file also produces a new fingerprint.

Run the comparison locally with:

```bash
cargo scout-audit --output-format json --output-path security-reports/scout-report.json
python3 scripts/security/check_scout.py --report security-reports/scout-report.json
```

To see a starter suppression block for every unreviewed finding:

```bash
python3 scripts/security/check_scout.py \
  --report security-reports/scout-report.json \
  --print-suppression-template
```

The template intentionally leaves `reason`, `expires` and `owner` empty. CI
rejects the file until they are filled in, so a template cannot be committed as
an accidental blanket suppression.

Baseline entries that no longer match any finding are reported in the summary
and as a notice annotation. Remove them in the same PR that fixes the finding.

## 2. Dependency policy with cargo-deny

`deny.toml` is the policy:

* **advisories** – vulnerability advisories always fail, yanked crates fail, and
  unmaintained/unsound advisories fail for crates the workspace depends on
  directly. An advisory ignore that no longer matches anything also fails.
* **licenses** – only the licenses in `[licenses].allow` are accepted. GPL,
  AGPL and SSPL are not allowed. Workspace crates are unpublished
  (`publish = false`) and inherit MIT from `[workspace.package]`, so
  `[licenses.private] ignore = true` keeps the check focused on third-party
  dependencies.
* **sources** – only crates.io is allowed. Unknown registries and git
  dependencies fail, so a PR cannot silently add another source.
* **bans** – duplicate versions and path-only dependency requirements are
  reported as warnings (the Soroban dependency tree currently has unavoidable
  duplicates and the workspace uses path dependencies).

Run it locally:

```bash
make deny
# equivalent to:
python3 scripts/security/render_deny_config.py --output target/security/deny.toml
cargo deny --config target/security/deny.toml check advisories bans licenses sources
```

### Why the config is rendered

The checked-in `deny.toml` has an empty `[advisories].ignore` list. CI renders a
temporary config (`target/security/deny.toml`) that injects only the reviewed
entries from `.github/security/suppressions.toml`, each with its `reason` and
native cargo-deny `expiry`. Two independent mechanisms therefore prevent an
open-ended suppression:

1. `scripts/security/check_suppressions.py` rejects a missing, placeholder,
   short or expired justification before cargo-deny runs.
2. `deny.toml` sets `ignore-expiry = "P0D"`, so an ignore entry without its own
   expiry is treated as already expired, and `unused-ignored-advisory = "deny"`,
   so a stale ignore fails the build.

To accept an advisory, add an `[[advisories]]` entry to
`.github/security/suppressions.toml`; do not edit `deny.toml`.

### Handling a license failure

`error[license-not-encountered]`/`error[unlicensed]` means a dependency uses a
license outside the allowlist. Do not add the license to `deny.toml` in the same
PR that introduces the dependency without review:

1. Confirm the license is compatible with shipping these contracts (and with the
   project's MIT licensing).
2. If the license is acceptable, add it to `[licenses].allow` with a short
   comment explaining why.
3. If only one crate needs it, prefer a scoped
   `[[licenses.exceptions]]` entry (`crate = "name", allow = ["License"]`) over a
   global allow.
4. If the license is not acceptable, replace the dependency.

## 3. cargo-audit and the weekly issue

`cargo-audit` checks `Cargo.lock` against the RustSec advisory database. It runs
on every pull request (as a report plus gate) and every Monday at 06:00 UTC.

The weekly workflow opens **one GitHub issue per new advisory**. Duplicate
prevention uses a hidden marker in the issue body:

```html
<!-- cargo-audit-advisory: RUSTSEC-2026-0387 -->
```

Before creating anything the script lists every issue in the repository (open
and closed) and skips advisories whose marker already exists. A closed issue is
therefore not reopened and the same advisory is never filed twice. Advisory
ids that appear in `.github/security/suppressions.toml` are treated as reviewed
and are not filed at all; they are listed in the job summary with their expiry
date instead.

When an issue is opened:

1. Try `cargo update -p <crate>` first. If the advisory has a patched version,
   that is usually the whole fix.
2. If the parent dependency pins an affected version, open an upstream issue and
   add a reviewed suppression with an expiry and a follow-up ticket.
3. Close the advisory issue only when the crate is updated or the suppression is
   recorded. Do not close it because the weekly job will re-open it — it will
   not, because the marker prevents duplicates, but the build will fail until it
   is handled.

Run the audit locally:

```bash
make audit
# ignore reviewed advisories exactly like CI does:
cargo audit $(python3 scripts/security/check_suppressions.py --audit-ignore-ids | sed 's/^/--ignore /')
```

## 4. Pinned GitHub Actions

Every `uses:` in `.github/workflows` and `.github/actions` must reference an
immutable commit SHA:

```yaml
- uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683  # v4.2.2
```

Tags and branches are mutable, so whoever controls the action repository could
move them and run arbitrary code with this repository's token. Local actions
(`uses: ./path`) and Docker actions pinned by digest
(`docker://image@sha256:…`) are accepted.

`scripts/security/check_pinned_actions.py` scans the workflow files and fails on
any mutable reference. It also warns when a pinned action has no trailing
version comment, so reviewers can tell what the SHA corresponds to.

To update an action:

```bash
git ls-remote https://github.com/actions/checkout 'refs/tags/v4.2.2'
# use the returned commit SHA and update the trailing comment
```

Run the check locally with `make security-actions-check`.

## 5. Suppression policy

Every suppression — Scout baseline entry or advisory ignore — must have:

| Field | Requirement |
| --- | --- |
| `reason` | At least 20 characters. Explain why the finding is safe to accept and what compensating control exists. Placeholders such as `tbd`, `todo`, `n/a` or `false positive` are rejected. |
| `owner` | The person or team accountable for the suppression. |
| `expires` | `YYYY-MM-DD`, in the future and at most 366 days away. |
| `ticket` | Optional issue or PR reference for the follow-up work. |

Extra fields are rejected rather than ignored, so a typo cannot silently drop a
control. Duplicate entries are rejected. Scout `file` paths must be
repository-relative.

Validation runs in three places:

* `python3 scripts/security/check_suppressions.py` (also part of the Scout and
  cargo-deny jobs),
* the Scout gate and the cargo-deny renderer, which both import the same
  validation module,
* the unit tests in `scripts/security/tests`.

An expired entry fails the build with the date it expired, which forces a
re-review at least once a year.

### Suppression examples

```toml
# Scout: one reviewed occurrence.
[[scout]]
detector = "unsafe-unwrap"
file = "creator-keys/src/lib.rs"
fingerprint = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
reason = "The key is written by the initializer above and unwrap cannot fail."
owner = "@accesslayerorg"
expires = "2027-06-30"
ticket = "#1319"

# Advisory: a RustSec id, or a crate@version for a yanked version.
[[advisories]]
id = "RUSTSEC-2026-0387"
reason = "The affected crypto code is only used on public ledger inputs and the timing attack precondition does not apply."
owner = "@accesslayerorg"
expires = "2027-06-30"
ticket = "#1319"
```

## 6. Security triage

### Severity and response

| Severity | Examples | Response |
| --- | --- | --- |
| Critical (Scout) | unprotected wasm update, insufficient randomness, storage collision, arithmetic that can be exploited | Fix or add a reviewed suppression **before merge**; the PR fails otherwise. |
| Medium (Scout) | `unsafe-unwrap`, `unsafe-expect`, divide-before-multiply | Triage in the PR. Fix when straightforward; otherwise record a suppression with an expiry and a follow-up ticket. Does not block by default. |
| Minor / Enhancement (Scout) | best-practice deviations, unused return values | Batch into scheduled hardening work. Do not suppress without a ticket. |
| RustSec vulnerability | any `cargo-audit`/cargo-deny vulnerability | Treat as Critical if a patched version exists; update first. An unreachable-code argument needs a written justification. |
| Disallowed license or source | GPL dependency, git dependency | Blocks merge. Replace the dependency or get an explicit policy decision. |

### Reviewing a finding

1. Download the relevant artifact from the failed job (or reproduce locally with
   the commands in this document).
2. Read the detector or advisory documentation and locate the flagged code.
3. Decide: fix, suppress, or (for a false positive) fix the detector input.
4. If fixing, delete any baseline entry for that finding in the same PR.
5. If suppressing, add the entry with a real reason, an owner, an expiry no more
   than a year away and a follow-up ticket.
6. Ask a second maintainer to review suppressions and allowlist changes.

### Reviewing suppressions

At least once per quarter (and whenever a suppression expires) the security
owner:

* re-reads every `[[scout]]` entry against the current code,
* confirms each advisory suppression is still accurate for the current
  dependency versions,
* removes entries that no longer match (the Scout job reports them, and
  cargo-deny fails on unused advisory ignores),
* confirms every entry still has a reachable follow-up ticket.

### Escalation

An unexplained Critical finding, a vulnerability in a crate that handles keys or
signatures, or a disallowed source added by a PR is escalated to the maintainers
immediately and must not be merged while unresolved.

## 7. Local commands

```bash
make security-tests          # unit tests for the policy scripts
make security-suppressions   # validate the suppression file
make deny                    # render + run cargo-deny
make audit                   # run cargo-audit
make security-actions-check  # detect unpinned GitHub Actions
```
