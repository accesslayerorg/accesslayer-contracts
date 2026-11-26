#!/usr/bin/env python3
"""CI check: every invariant id in docs/invariants.md must be referenced by a test.

Scans `docs/invariants.md` for invariant ids of the form `INV-<CATEGORY>-<NNN>`
and verifies that each id string appears in at least one Rust test source file
under the workspace. Exits non-zero (failing CI) when any invariant id has no
referencing test.

Usage:
    python scripts/check_invariants_tested.py          # from repo root
    python scripts/check_invariants_tested.py --docs docs/invariants.md
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

INVARIANT_ID_RE = re.compile(r"\bINV-[A-Z]+-\d+\b")
TEST_FILE_GLOBS = ["**/*test*.rs", "**/tests/**/*.rs", "src/**/test_*.rs"]


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def collect_test_files(root: Path) -> list[Path]:
    seen: set[Path] = set()
    for glob_pattern in TEST_FILE_GLOBS:
        for path in root.glob(glob_pattern):
            if path.is_file():
                seen.add(path)
    return sorted(seen)


def extract_invariant_ids(docs_path: Path) -> list[str]:
    text = docs_path.read_text(encoding="utf-8")
    # Only the invariant table (Section 2) requires tests. The "Known
    # limitations" section lists accepted risks, not testable invariants, so
    # its ids are excluded.
    table_section = text.split("## 3. Known limitations", 1)[0]
    ids: list[str] = []
    seen: set[str] = set()
    for match in INVARIANT_ID_RE.finditer(table_section):
        inv_id = match.group(0)
        if inv_id not in seen:
            seen.add(inv_id)
            ids.append(inv_id)
    return ids


def read_file_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--docs",
        default="docs/invariants.md",
        help="Path to the invariants document (default: docs/invariants.md)",
    )
    args = parser.parse_args()

    root = repo_root()
    docs_path = root / args.docs
    if not docs_path.exists():
        print(f"error: invariants document not found: {docs_path}", file=sys.stderr)
        return 2

    invariant_ids = extract_invariant_ids(docs_path)
    if not invariant_ids:
        print("error: no invariant ids found in", docs_path, file=sys.stderr)
        return 2

    test_files = collect_test_files(root)
    test_texts: dict[Path, str] = {p: read_file_text(p) for p in test_files}

    missing: list[str] = []
    for inv_id in invariant_ids:
        referenced = any(inv_id in text for text in test_texts.values())
        if not referenced:
            missing.append(inv_id)

    if missing:
        print(
            f"CI check failed: {len(missing)} invariant id(s) have no referencing test:",
            file=sys.stderr,
        )
        for inv_id in missing:
            print(f"  - {inv_id}", file=sys.stderr)
        print(
            "Add the id to a test header (e.g. `//! Implements INV-XXX-001.`) or "
            "remove the invariant from docs/invariants.md.",
            file=sys.stderr,
        )
        return 1

    print(
        f"OK: all {len(invariant_ids)} invariant id(s) are referenced by at least one test "
        f"({len(test_files)} test file(s) scanned)."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
