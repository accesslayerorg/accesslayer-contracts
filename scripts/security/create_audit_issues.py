#!/usr/bin/env python3
"""Open one tracked GitHub issue per new RustSec advisory from cargo-audit.

Duplicate prevention: every issue this script creates contains a hidden marker
``<!-- cargo-audit-advisory: RUSTSEC-YYYY-NNNN -->``. Before creating anything
the script lists every issue in the repository (open and closed) and skips any
advisory whose marker already exists, so a weekly run cannot open the same
advisory twice and a manually closed issue is not reopened.

Only vulnerable dependencies (``vulnerabilities.list``) produce issues;
informational warnings such as unmaintained crates are reported in the workflow
summary instead.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gha  # noqa: E402
from suppressions import PolicyError, load_suppressions  # noqa: E402

MARKER_PREFIX = "cargo-audit-advisory:"
DEFAULT_SUPPRESSIONS = Path(__file__).resolve().parents[2] / ".github/security/suppressions.toml"
DEFAULT_MAX_PAGES = 20
PER_PAGE = 100


class ApiError(Exception):
    """A GitHub API call failed."""


def issue_marker(advisory_id: str) -> str:
    return f"<!-- {MARKER_PREFIX} {advisory_id} -->"


def load_advisories(report_path: Path) -> list[dict]:
    if not report_path.is_file():
        raise ValueError(f"cargo-audit report not found: {report_path}")
    report = json.loads(report_path.read_text(encoding="utf-8"))
    vulnerabilities = report.get("vulnerabilities") or {}
    entries = vulnerabilities.get("list") or []

    advisories: list[dict] = []
    seen: set[str] = set()
    for entry in entries:
        advisory = entry.get("advisory") or {}
        advisory_id = str(advisory.get("id", "")).strip()
        if not advisory_id or advisory_id in seen:
            continue
        seen.add(advisory_id)
        package = entry.get("package") or {}
        advisories.append(
            {
                "id": advisory_id,
                "title": str(advisory.get("title", "")).strip() or advisory_id,
                "description": str(advisory.get("description", "")).strip(),
                "url": str(advisory.get("url", "")).strip(),
                "date": str(advisory.get("date", "")).strip(),
                "aliases": [str(alias) for alias in advisory.get("aliases") or []],
                "cvss": advisory.get("cvss"),
                "crate": str(package.get("name", "")).strip(),
                "version": str(package.get("version", "")).strip(),
                "patched": [
                    str(version) for version in (entry.get("versions") or {}).get("patched") or []
                ],
            }
        )
    return advisories


def existing_advisory_ids(issue_bodies: list[str]) -> set[str]:
    """Extract advisory ids already tracked from issue bodies."""

    tracked: set[str] = set()
    for body in issue_bodies:
        if not body or MARKER_PREFIX not in body:
            continue
        for line in body.splitlines():
            if MARKER_PREFIX not in line:
                continue
            marker = line.split(MARKER_PREFIX, 1)[1]
            marker = marker.split("-->", 1)[0].strip()
            if marker:
                tracked.add(marker)
    return tracked


def partition_new(advisories: list[dict], tracked: set[str]) -> list[dict]:
    return [advisory for advisory in advisories if advisory["id"] not in tracked]


def suppressed_advisories(
    advisories: list[dict], suppression_entries: list[dict]
) -> list[dict]:
    """Return advisories covered by a reviewed, unexpired suppression.

    Suppressions are the repository's reviewed record of accepted advisories;
    an advisory that is explicitly accepted with a justification and an expiry
    date is not a new advisory and must not open a tracking issue.
    """

    russ_ids = {entry["id"] for entry in suppression_entries if entry["id"].startswith("RUSTSEC-")}
    crate_specs = {
        entry["id"] for entry in suppression_entries if not entry["id"].startswith("RUSTSEC-")
    }
    return [
        advisory
        for advisory in advisories
        if advisory["id"] in russ_ids
        or f"{advisory['crate']}@{advisory['version']}" in crate_specs
    ]


def build_issue(advisory: dict, repo: str) -> tuple[str, str]:
    title = (
        f"[security] {advisory['id']}: {advisory['title']} "
        f"({advisory['crate']} {advisory['version']})"
    )
    lines = [
        issue_marker(advisory["id"]),
        f"## RustSec advisory `{advisory['id']}`",
        "",
        f"**Crate:** `{advisory['crate']}` `{advisory['version']}`",
        f"**Published:** {advisory['date'] or 'unknown'}",
    ]
    if advisory["aliases"]:
        lines.append(f"**Aliases:** {', '.join(advisory['aliases'])}")
    if advisory["url"]:
        lines.append(f"**Advisory:** {advisory['url']}")
    lines += [
        "",
        "### Summary",
        "",
        advisory["description"] or advisory["title"],
        "",
        "### Remediation",
        "",
        "1. Run `cargo update -p "
        f"{advisory['crate']}` (or bump the parent dependency) so the crate resolves to "
        + (", ".join(advisory["patched"]) if advisory["patched"] else "a patched version")
        + ".",
        "2. Re-run `cargo audit` and `cargo deny check advisories` locally.",
        "3. If the advisory does not apply to this repository, add a reviewed entry to "
        "`.github/security/suppressions.toml` with a justification, an owner and an "
        "expiry date, then close this issue.",
        "",
        "See `docs/security-ci.md` for the triage workflow.",
        "",
        "_Opened automatically by the weekly cargo-audit workflow. Do not delete the "
        "hidden marker above: it prevents duplicate issues._",
        "",
        f"<!-- repository: {repo} -->",
    ]
    return title, "\n".join(lines)


class GitHubClient:
    def __init__(self, repo: str, token: str, api_url: str = "https://api.github.com"):
        self.repo = repo
        self.token = token
        self.api_url = api_url.rstrip("/")

    def _request(self, method: str, path: str, payload: dict | None = None):
        url = f"{self.api_url}{path}"
        data = json.dumps(payload).encode("utf-8") if payload is not None else None
        request = urllib.request.Request(url, data=data, method=method)
        request.add_header("Accept", "application/vnd.github+json")
        request.add_header("Authorization", f"Bearer {self.token}")
        request.add_header("X-GitHub-Api-Version", "2022-11-28")
        request.add_header("User-Agent", "accesslayer-security-ci")
        if data is not None:
            request.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                body = response.read()
        except urllib.error.HTTPError as exc:
            detail = exc.read().decode("utf-8", "replace")[:500]
            raise ApiError(f"{method} {path} failed with HTTP {exc.code}: {detail}") from exc
        except urllib.error.URLError as exc:
            raise ApiError(f"{method} {path} failed: {exc.reason}") from exc
        return json.loads(body) if body else None

    def list_issues(self, max_pages: int = DEFAULT_MAX_PAGES) -> list[dict]:
        issues: list[dict] = []
        for page in range(1, max_pages + 1):
            query = urllib.parse.urlencode(
                {
                    "state": "all",
                    "per_page": PER_PAGE,
                    "page": page,
                    "sort": "created",
                    "direction": "desc",
                }
            )
            batch = self._request(
                "GET", f"/repos/{self.repo}/issues?{query}"
            ) or []
            issues.extend(batch)
            if len(batch) < PER_PAGE:
                break
        return [issue for issue in issues if "pull_request" not in issue]

    def create_issue(self, title: str, body: str) -> dict:
        return self._request(
            "POST", f"/repos/{self.repo}/issues", {"title": title, "body": body}
        )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", required=True, help="cargo-audit JSON report")
    parser.add_argument(
        "--repo",
        default=os.environ.get("GITHUB_REPOSITORY"),
        help="owner/repo (default: $GITHUB_REPOSITORY)",
    )
    parser.add_argument(
        "--token-env",
        default="GITHUB_TOKEN",
        help="environment variable holding a token with issues:write",
    )
    parser.add_argument("--api-url", default=os.environ.get("GITHUB_API_URL", "https://api.github.com"))
    parser.add_argument("--suppressions", default=str(DEFAULT_SUPPRESSIONS))
    parser.add_argument("--dry-run", action="store_true", help="print, do not create")
    parser.add_argument("--max-pages", type=int, default=DEFAULT_MAX_PAGES)
    return parser


def main(argv=None) -> int:
    args = build_parser().parse_args(argv)

    try:
        advisories = load_advisories(Path(args.report))
        suppression_entries = load_suppressions(args.suppressions)["advisories"]
    except (OSError, ValueError, PolicyError, json.JSONDecodeError) as exc:
        print(gha.error(str(exc), title="cargo-audit issues"))
        return 1

    accepted = suppressed_advisories(advisories, suppression_entries)
    accepted_ids = {advisory["id"] for advisory in accepted}

    if not advisories:
        print("cargo-audit: no vulnerable dependencies; no issues to open.")
        gha.append_step_summary(
            "## cargo-audit\n\nNo vulnerable dependencies were reported this week.\n"
        )
        return 0

    token = os.environ.get(args.token_env, "")
    if not token and not args.dry_run:
        print(
            gha.error(
                f"{args.token_env} is not set; cannot list or create tracking issues",
                title="cargo-audit issues",
            )
        )
        return 1
    if not args.repo and not args.dry_run:
        print(gha.error("--repo or GITHUB_REPOSITORY is required", title="cargo-audit issues"))
        return 1

    client = GitHubClient(args.repo or "owner/repo", token, args.api_url)

    try:
        existing = client.list_issues(args.max_pages) if not args.dry_run else []
        tracked = existing_advisory_ids([issue.get("body") or "" for issue in existing])
        new = partition_new(advisories, tracked | accepted_ids)
    except ApiError as exc:
        print(gha.error(str(exc), title="cargo-audit issues"))
        return 1

    created: list[tuple[str, str]] = []
    for advisory in new:
        title, body = build_issue(advisory, args.repo or "")
        if args.dry_run:
            print(f"[dry-run] would create: {title}")
            created.append((title, ""))
            continue
        try:
            issue = client.create_issue(title, body)
        except ApiError as exc:
            print(gha.error(str(exc), title="cargo-audit issues"))
            return 1
        created.append((title, str(issue.get("html_url", ""))))
        print(f"Opened {issue.get('html_url', title)}")

    skipped = len(advisories) - len(new) - len(accepted)
    print(
        f"cargo-audit: {len(advisories)} advisory(ies), {len(created)} new issue(s), "
        f"{skipped} already tracked, {len(accepted)} suppressed by reviewed policy."
    )

    summary = ["## cargo-audit", ""]
    if created:
        summary.append(f"**{len(created)} new advisory issue(s) opened:**")
        summary.append("")
        for title, url in created:
            summary.append(f"- {url or title}")
    else:
        summary.append("No new advisories since the last run.")
    if skipped:
        summary.append("")
        summary.append(f"{skipped} advisory(ies) were already tracked and were not duplicated.")
    if accepted:
        summary.append("")
        summary.append(
            f"{len(accepted)} advisory(ies) are accepted by a reviewed suppression "
            "in `.github/security/suppressions.toml` and were not opened as issues:"
        )
        expiries = {entry["id"]: entry["expires"] for entry in suppression_entries}
        for advisory in accepted:
            expiry = expiries.get(advisory["id"], "")
            if not expiry:
                expiry = expiries.get(f"{advisory['crate']}@{advisory['version']}", "")
            summary.append(f"- `{advisory['id']}` (expires {expiry})")
    gha.append_step_summary("\n".join(summary) + "\n")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
