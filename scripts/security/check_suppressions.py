#!/usr/bin/env python3
"""Validate .github/security/suppressions.toml against the repository policy.

Fails (exit 1) when an entry is missing a justification, an owner or an expiry
date, when the expiry date is in the past or too far in the future, or when the
file contains unknown keys. CI runs this before any tool is allowed to consume
the suppression file.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gha  # noqa: E402
from suppressions import PolicyError, load_suppressions  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SUPPRESSIONS = REPO_ROOT / ".github/security/suppressions.toml"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--file",
        default=str(DEFAULT_SUPPRESSIONS),
        help="suppression file to validate (default: %(default)s)",
    )
    parser.add_argument(
        "--today",
        default=None,
        help="override today's date (YYYY-MM-DD) for expiry checks and tests",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="print the validated suppressions as JSON",
    )
    parser.add_argument(
        "--audit-ignore-ids",
        action="store_true",
        help="print the RustSec ids to pass to cargo-audit --ignore, one per line",
    )
    return parser


def main(argv=None) -> int:
    args = build_parser().parse_args(argv)

    today = None
    if args.today:
        try:
            today = dt.date.fromisoformat(args.today)
        except ValueError:
            print(f"::error title=Security suppressions::invalid --today date {args.today!r}")
            return 2

    try:
        suppressions = load_suppressions(args.file, today=today)
    except PolicyError as exc:
        print(gha.error(str(exc), title="Security suppression policy"))
        print(f"Suppression policy failed: {exc}", file=sys.stderr)
        return 1

    scout_count = len(suppressions["scout"])
    advisory_count = len(suppressions["advisories"])

    if args.audit_ignore_ids:
        # cargo-audit only understands RustSec advisory ids, not crate specs.
        for entry in suppressions["advisories"]:
            if entry["id"].startswith("RUSTSEC-"):
                print(entry["id"])
    elif args.json:
        print(json.dumps(suppressions, indent=2, sort_keys=True))
    else:
        print(
            f"Suppression policy OK: {scout_count} Scout baseline entr"
            f"{'y' if scout_count == 1 else 'ies'}, "
            f"{advisory_count} advisory entr{'y' if advisory_count == 1 else 'ies'}."
        )
        for entry in suppressions["scout"]:
            print(
                f"  scout: {entry['detector']} in {entry['file']} "
                f"(expires {entry['expires']}, owner {entry['owner']})"
            )
        for entry in suppressions["advisories"]:
            print(
                f"  advisory: {entry['id']} "
                f"(expires {entry['expires']}, owner {entry['owner']})"
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
