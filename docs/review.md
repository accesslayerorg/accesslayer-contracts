# Review process for sensitive contract paths

Changes to authorization, fund movement, upgrade and storage code must not
merge without a reviewer who knows the risks. This page describes how the
repository enforces that and what a maintainer has to approve.

## The pieces

| File | What it does |
| --- | --- |
| [`.github/sensitive-paths.yml`](../.github/sensitive-paths.yml) | Single source of truth: the sensitive paths, grouped by risk area. |
| [`.github/CODEOWNERS`](../.github/CODEOWNERS) | Requests a review from the owners of every sensitive path a pull request touches. |
| [`.github/PULL_REQUEST_TEMPLATE.md`](../.github/PULL_REQUEST_TEMPLATE.md) | Carries the **Security review** checklist (authorization, arithmetic, fund movement, storage, upgrades, events). |
| [`.github/workflows/security-review.yml`](../.github/workflows/security-review.yml) | The **Security checklist** job. Fails a pull request that changes a sensitive path while any Security review box is unticked. |
| [`.github/labeler.yml`](../.github/labeler.yml) and [`.github/workflows/labeler.yml`](../.github/workflows/labeler.yml) | Label each pull request `area: <name>` for every area it touches. |
| [`scripts/security_checklist/check.py`](../scripts/security_checklist/check.py) | The checker the job runs, with unit tests in `scripts/security_checklist/tests/`. |

The Security checklist job also fails when `CODEOWNERS` or `labeler.yml` stops
covering a pattern from `sensitive-paths.yml`, so the three files cannot drift
apart unnoticed.

## Sensitive areas

| Area | Label | Covers | Code owners |
| --- | --- | --- | --- |
| `auth` | `area: auth` | `access-control`, `multisig-admin`, `nonce-registry`, `session-nonce`, `sponsored-transactions`, `personhood-hook`, `timelock`, `common/src/guard.rs`, and the `creator-keys` ACL, badge access, ownership transfer, social recovery and governance modules | @Chucks1093 |
| `fees` | `area: fees` | `protocol-fee-contract`, `fee-discount-contract`, `creator-keys/src/dynamic_fees.rs` | @Chucks1093 |
| `reserve` | `area: reserve` | `reserve-contract`, `treasury-contract`, `treasury-diversification`, `holder-insurance-pool`, `creator-keys` token rescue and multi-asset payment | @Chucks1093 |
| `upgrade` | `area: upgrade` | `creator-keys/src/lib.rs` (holds `upgrade`), `proxy.rs`, emergency and timed pause, the `test_wasm` upgrade fixture, `creator-keys-factory`, `safe-mode-contract`, `circuit-breaker` | @Chucks1093 |
| `storage` | `area: storage` | `creator-keys/src/lib.rs` (holds `DataKey` and TTL handling), `storage-cleanup` | @Chucks1093 |
| `review-process` | `area: review-process` | `CODEOWNERS`, `sensitive-paths.yml`, `labeler.yml`, the pull request template, `.github/workflows/`, `scripts/security_checklist/` | @Chucks1093 |

`sensitive-paths.yml` has the exact globs. `creator-keys/src/lib.rs` is listed
under both `upgrade` and `storage` because it holds the `upgrade` entrypoint
and the `DataKey` layout, so most changes to the main contract need the
checklist. Moving those into their own modules later would narrow the gate.

Non-sensitive pull requests also get `area: contracts`, `area: tests`,
`area: docs` or `area: ci` labels. Those labels never require the checklist.

## Approvals required

| Change touches | Approvals | Code owner approval | Security checklist |
| --- | --- | --- | --- |
| Only non-sensitive paths (other contracts, tests, docs) | 1 maintainer | Not required | Not required |
| `auth`, `fees`, `reserve` or `storage` | 2 | Required | Every item ticked |
| `upgrade` | 2 | Required | Every item ticked, and the WASM fixture refreshed when contract code changed |
| `review-process` | 2 | Required | Every item ticked |

GitHub's branch protection sets one approval count for the whole branch, so
the second approval on sensitive paths is a maintainer rule: do not merge a
sensitive pull request with fewer than two approvals, even when GitHub allows
it. The code owner approval and the checklist are enforced by GitHub.

## How to complete the checklist

1. Copy the **Security review** section from the pull request template if the
   body does not already have it.
2. Tick each item you reviewed. If an item does not apply, tick it and write
   `N/A` with the reason, for example `N/A: no token transfers changed`.
3. Editing the description re-runs the job; you do not need to push again.

The job lists which sensitive files triggered it. Boxes under other headings,
such as the general **Checklist**, do not affect this job.

## Branch protection guidance

Maintainers should configure these settings on `main` (Settings → Branches →
Branch protection rules, or an equivalent ruleset):

- Require a pull request before merging.
  - Required approvals: **1** (the two-approval rule for sensitive paths above
    is applied by maintainers).
  - Dismiss stale pull request approvals when new commits are pushed.
  - Require review from Code Owners.
- Require status checks to pass before merging, and require branches to be up
  to date before merging. Required checks:
  - `verify` (Contracts CI: fmt, clippy, tests)
  - `Security checklist` (Security review)
- Require conversation resolution before merging.
- Do not allow force pushes or deletions.
- Apply the rules to administrators too, so the gate cannot be skipped.

The labeler job is informational and should not be a required check.

## Changing the ownership mapping

To add or move a sensitive path, update `.github/sensitive-paths.yml`,
`.github/CODEOWNERS` and `.github/labeler.yml` in the same pull request, then
run:

```sh
python3 scripts/security_checklist/check.py mapping
python3 -m unittest discover -s scripts/security_checklist/tests
```

Because these files are in the `review-process` area, the change itself needs
a code owner approval. Maintainers confirm the mapping when they review it.
