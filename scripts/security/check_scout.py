#!/usr/bin/env python3
"""Gate a Scout for Soroban JSON report against the reviewed baseline.

Scout's severity scale is Critical, Medium, Minor and Enhancement. "Critical"
is the tool's high-severity bucket, so ``--fail-on critical`` is the default:
a new Critical finding fails the build. The threshold can be lowered with
``--fail-on medium`` (or ``minor`` / ``enhancement``) without touching the
workflow.

Findings are matched to the reviewed baseline in
``.github/security/suppressions.toml`` by a fingerprint derived from the
detector, the file and the flagged code. Editing the flagged code changes the
fingerprint, so an existing suppression cannot hide a changed finding.

The script also emits GitHub Actions annotations and writes a Markdown summary
that the workflow publishes with the report artifact.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gha  # noqa: E402
from suppressions import (  # noqa: E402
    PolicyError,
    load_suppressions,
    normalize_path,
    scout_fingerprint,
)

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SUPPRESSIONS = REPO_ROOT / ".github/security/suppressions.toml"
DEFAULT_SUMMARY = REPO_ROOT / "security-reports/scout-summary.md"

SEVERITY_RANK = {
    "critical": 3,
    "medium": 2,
    "unknown": 2,
    "minor": 1,
    "enhancement": 0,
}
SEVERITY_ALIASES = {
    "high": "critical",
    "critical": "critical",
    "medium": "medium",
    "moderate": "medium",
    "minor": "minor",
    "low": "minor",
    "enhancement": "enhancement",
    "info": "enhancement",
    "none": "none",
}
THRESHOLD_CHOICES = ["critical", "high", "medium", "minor", "enhancement", "none"]
MAX_WARNING_ANNOTATIONS = 25
_SPAN_RE = re.compile(r":(\d+):(\d+)")
_IN_ACTIONS = os.environ.get("GITHUB_ACTIONS") == "true"


@dataclass(frozen=True)
class Finding:
    detector: str
    severity: str
    file: str
    line: int | None
    column: int | None
    message: str
    snippet: str
    fingerprint: str
    package: str = ""


class ReportError(Exception):
    """The Scout report is missing or cannot be understood."""


def _severity(value: str) -> str:
    return SEVERITY_ALIASES.get(str(value).strip().lower(), "unknown")


def load_report(path: Path) -> dict:
    if not path.is_file():
        raise ReportError(
            f"Scout report not found at {path}. Scout must run and produce a JSON "
            "report before the baseline check can pass."
        )
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReportError(f"cannot read Scout report {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise ReportError(f"Scout report {path} is not a JSON object")
    return data


def severity_map(report: dict) -> dict[str, str]:
    """Map detector id -> severity using the report's category metadata."""

    mapping: dict[str, str] = {}
    for category in report.get("categories", []) or []:
        for vulnerability in category.get("vulnerabilities", []) or []:
            detector = str(vulnerability.get("id", "")).strip()
            if detector:
                mapping[detector] = _severity(vulnerability.get("severity", ""))
    return mapping


def iter_findings(report: dict) -> list[Finding]:
    severities = severity_map(report)
    findings: list[Finding] = []

    for raw in report.get("findings", []) or []:
        detector = str(raw.get("vulnerability_id", "")).strip()
        file_path = normalize_path(raw.get("file_path", ""))
        if not detector or not file_path:
            continue
        snippet = str(raw.get("code_snippet", ""))
        line = column = None
        span_match = _SPAN_RE.search(str(raw.get("span", "")))
        if span_match:
            line = int(span_match.group(1))
            column = int(span_match.group(2))
        findings.append(
            Finding(
                detector=detector,
                severity=severities.get(detector, "unknown"),
                file=file_path,
                line=line,
                column=column,
                message=str(raw.get("error_message", "")).strip(),
                snippet=snippet,
                fingerprint=scout_fingerprint(detector, file_path, snippet),
                package=str(raw.get("package", "")).strip(),
            )
        )
    return findings


def _threshold_rank(threshold: str) -> int:
    if threshold == "none":
        return 99
    return SEVERITY_RANK[SEVERITY_ALIASES[threshold]]


def _is_blocking(finding: Finding, threshold_rank: int) -> bool:
    return SEVERITY_RANK.get(finding.severity, 2) >= threshold_rank


def evaluate(
    findings: list[Finding], baseline_fingerprints: set[str], threshold: str
) -> tuple[list[Finding], list[Finding], list[Finding]]:
    """Split findings into (blocking, unreviewed, suppressed)."""

    threshold_rank = _threshold_rank(threshold)
    blocking: list[Finding] = []
    unreviewed: list[Finding] = []
    suppressed: list[Finding] = []

    for finding in findings:
        reviewed = finding.fingerprint in baseline_fingerprints
        if reviewed:
            suppressed.append(finding)
        else:
            unreviewed.append(finding)
            if _is_blocking(finding, threshold_rank):
                blocking.append(finding)

    return blocking, unreviewed, suppressed


def render_summary(
    findings: list[Finding],
    blocking: list[Finding],
    unreviewed: list[Finding],
    suppressed: list[Finding],
    threshold: str,
    stale_entries: list[dict],
) -> str:
    counts: dict[str, int] = {}
    for finding in findings:
        counts[finding.severity] = counts.get(finding.severity, 0) + 1

    lines = [
        "## Scout for Soroban",
        "",
        f"- Findings: **{len(findings)}**",
        f"- Reviewed baseline matches: **{len(suppressed)}**",
        f"- Unreviewed findings: **{len(unreviewed)}**",
        f"- Blocking (severity >= `{threshold}`): **{len(blocking)}**",
        f"- By severity: "
        + ", ".join(
            f"{name}: {counts.get(name, 0)}"
            for name in ("critical", "medium", "unknown", "minor", "enhancement")
        ),
        "",
    ]

    if blocking:
        lines += [
            "### Blocking findings",
            "",
            "| Severity | Detector | Location | Message |",
            "| --- | --- | --- | --- |",
        ]
        for finding in blocking[:20]:
            location = finding.file + (
                f":{finding.line}" if finding.line is not None else ""
            )
            message = finding.message.replace("|", "\\|")[:200]
            lines.append(
                f"| {finding.severity} | `{finding.detector}` | `{location}` | {message} |"
            )
        if len(blocking) > 20:
            lines.append(f"| ... | ... | ... | {len(blocking) - 20} more in the artifact |")
        lines += [
            "",
            "Fix these findings, or add a reviewed suppression with a reason and an "
            "expiry date to `.github/security/suppressions.toml`.",
            "",
        ]

    if stale_entries:
        lines += [
            "### Stale baseline entries",
            "",
            "These suppressions no longer match any finding. Remove them or explain "
            "why they must stay (for example a detector that was renamed):",
            "",
        ]
        for entry in stale_entries:
            lines.append(
                f"- `{entry['detector']}` in `{entry['file']}` "
                f"(owner {entry['owner']}, expires {entry['expires']})"
            )
        lines.append("")

    lines += [
        "Reports: download the `scout-report` artifact from this workflow run.",
        "Triage guide: `docs/security-ci.md`.",
        "",
    ]
    return "\n".join(lines)


def print_suppression_template(findings: list[Finding]) -> None:
    seen: set[str] = set()
    for finding in findings:
        if finding.fingerprint in seen:
            continue
        seen.add(finding.fingerprint)
        location = finding.file + (f":{finding.line}" if finding.line else "")
        print(f"# {finding.severity}: {finding.detector} in {location}")
        print("[[scout]]")
        print(f"detector = {json.dumps(finding.detector)}")
        print(f"file = {json.dumps(finding.file)}")
        print(f"fingerprint = {json.dumps(finding.fingerprint)}")
        print('reason = ""  # TODO: why is this finding safe to accept? (>= 20 chars)')
        print('expires = ""  # TODO: YYYY-MM-DD, within the next 366 days')
        print('owner = ""  # TODO: accountable person or team')
        print()


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", required=True, help="Scout JSON report path")
    parser.add_argument("--suppressions", default=str(DEFAULT_SUPPRESSIONS))
    parser.add_argument(
        "--fail-on",
        default="critical",
        choices=THRESHOLD_CHOICES,
        help="lowest Scout severity that fails the build (default: %(default)s)",
    )
    parser.add_argument("--summary-out", default=str(DEFAULT_SUMMARY))
    parser.add_argument("--today", default=None, help="override today's date for tests")
    parser.add_argument(
        "--print-suppression-template",
        action="store_true",
        help="print starter suppression entries for unreviewed findings and exit",
    )
    return parser


def main(argv=None) -> int:
    args = build_parser().parse_args(argv)

    try:
        today = dt.date.fromisoformat(args.today) if args.today else None
        suppressions = load_suppressions(args.suppressions, today=today)
        report = load_report(Path(args.report))
    except (PolicyError, ReportError, ValueError) as exc:
        message = str(exc)
        print(gha.error(message, title="Scout baseline"))
        print(f"Scout baseline check failed: {message}", file=sys.stderr)
        return 1

    findings = iter_findings(report)
    baseline = suppressions["scout"]
    baseline_fingerprints = {entry["fingerprint"] for entry in baseline}
    matched = {finding.fingerprint for finding in findings}

    if args.print_suppression_template:
        print_suppression_template(findings)
        return 0

    blocking, unreviewed, suppressed = evaluate(
        findings, baseline_fingerprints, args.fail_on
    )
    stale_entries = [
        entry for entry in baseline if entry["fingerprint"] not in matched
    ]

    if _IN_ACTIONS:
        for finding in blocking:
            print(
                gha.error(
                    f"New {finding.severity} Scout finding: {finding.message}",
                    title=f"Scout {finding.detector}",
                    file=finding.file,
                    line=str(finding.line) if finding.line is not None else "",
                    col=str(finding.column) if finding.column is not None else "",
                )
            )
        warnings_emitted = 0
        for finding in unreviewed:
            if finding in blocking or warnings_emitted >= MAX_WARNING_ANNOTATIONS:
                continue
            warnings_emitted += 1
            print(
                gha.warning(
                    f"Unreviewed {finding.severity} Scout finding: {finding.message}",
                    title=f"Scout {finding.detector}",
                    file=finding.file,
                    line=str(finding.line) if finding.line is not None else "",
                )
            )
        if len(unreviewed) - len(blocking) > warnings_emitted:
            print(
                gha.notice(
                    f"{len(unreviewed) - len(blocking) - warnings_emitted} further "
                    "unreviewed Scout finding(s) are listed in the scout-report artifact"
                )
            )
        for entry in stale_entries:
            print(
                gha.notice(
                    f"Baseline entry for {entry['detector']} in {entry['file']} "
                    "matches no finding; remove it if the issue is fixed",
                    title="Stale Scout suppression",
                )
            )

    summary = render_summary(
        findings, blocking, unreviewed, suppressed, args.fail_on, stale_entries
    )
    summary_path = Path(args.summary_out)
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_path.write_text(summary, encoding="utf-8")
    gha.append_step_summary(summary)

    print(
        f"Scout: {len(findings)} finding(s), {len(suppressed)} reviewed, "
        f"{len(unreviewed)} unreviewed, {len(blocking)} blocking "
        f"(threshold: {args.fail_on})."
    )

    if blocking:
        print(
            f"::error title=Scout baseline::{len(blocking)} new finding(s) at or above "
            f"the '{args.fail_on}' threshold. See the scout-report artifact."
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
