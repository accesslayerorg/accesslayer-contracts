#!/usr/bin/env python3
"""Fail when a GitHub Actions workflow uses a mutable action reference.

Every ``uses:`` in ``.github/workflows`` and ``.github/actions`` must reference
an immutable commit SHA (40 lowercase hex characters). Tags and branches are
mutable: whoever controls the action repository can move them and run arbitrary
code with this repository's token.

Docker actions must be pinned by digest (``...@sha256:<64 hex>``). Local
composite actions (``./path``) are allowed because they live in this repository.

A pinned action should keep a trailing comment naming the human-readable
version (``# v4.2.2``); a missing comment is reported as a warning so reviewers
can see what the SHA corresponds to, but it does not fail the build.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gha  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]

USES_RE = re.compile(
    r"^(?P<indent>\s*)(?:-\s+)?uses:\s*(?P<value>[^\s#]+)\s*(?:#\s*(?P<comment>.*))?$"
)
BLOCK_SCALAR_RE = re.compile(r"^(?P<indent>\s*)[A-Za-z0-9_.-]+:\s*[|>][-+]?\s*$")
SHA_RE = re.compile(r"^[0-9a-f]{40}$")
DOCKER_DIGEST_RE = re.compile(r"@sha256:[0-9a-f]{64}$")
_IN_ACTIONS = os.environ.get("GITHUB_ACTIONS") == "true"


def workflow_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for directory in (root / ".github/workflows", root / ".github/actions"):
        if not directory.is_dir():
            continue
        for pattern in ("*.yml", "*.yaml"):
            files.extend(sorted(directory.rglob(pattern)))
    return sorted(set(files))


def _strip_quotes(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "'\"":
        return value[1:-1]
    return value


def classify(value: str) -> tuple[str, str]:
    """Return (status, detail) for one ``uses:`` reference.

    status is "ok", "missing-sha" (error) or "uncommented" (warning).
    """

    value = _strip_quotes(value.strip())

    if value.startswith("./") or value.startswith(".\\"):
        return "ok", "local action"

    if value.startswith("docker://"):
        if DOCKER_DIGEST_RE.search(value):
            return "ok", "docker digest"
        return "missing-sha", "docker image must be pinned with @sha256:<digest>"

    if "@" not in value:
        return "missing-sha", "action reference has no version at all"

    _, ref = value.rsplit("@", 1)
    if SHA_RE.match(ref):
        return "ok", "commit sha"

    return (
        "missing-sha",
        f"ref {ref!r} is mutable; pin the action to a 40 character commit SHA",
    )


def scan_file(path: Path, root: Path) -> list[tuple[str, int, str]]:
    """Return (level, line_number, message) for every finding in one file."""

    findings: list[tuple[str, int, str]] = []
    block_indent: int | None = None

    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        return [("error", 0, f"cannot read {path}: {exc}")]

    for number, line in enumerate(lines, start=1):
        stripped = line.strip()

        if block_indent is not None:
            if not stripped:
                continue
            indent = len(line) - len(line.lstrip())
            if indent > block_indent:
                continue
            block_indent = None

        block_match = BLOCK_SCALAR_RE.match(line)
        if block_match:
            block_indent = len(block_match.group("indent"))
            continue

        if not stripped or stripped.startswith("#"):
            continue

        match = USES_RE.match(line)
        if not match:
            continue

        status, detail = classify(match.group("value"))
        relative = path.relative_to(root) if path.is_relative_to(root) else path
        if status == "missing-sha":
            findings.append(("error", number, f"{relative}:{number}: {detail}"))
        elif not (match.group("comment") or "").strip():
            findings.append(
                (
                    "warning",
                    number,
                    f"{relative}:{number}: pinned without a version comment "
                    "(add e.g. '# v4.2.2')",
                )
            )
    return findings


def scan(root: Path) -> list[tuple[str, int, str, Path]]:
    results: list[tuple[str, int, str, Path]] = []
    for path in workflow_files(root):
        for level, number, message in scan_file(path, root):
            results.append((level, number, message, path))
    return results


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=str(REPO_ROOT))
    return parser


def main(argv=None) -> int:
    args = build_parser().parse_args(argv)
    root = Path(args.root).resolve()

    results = scan(root)
    errors = [item for item in results if item[0] == "error"]
    warnings = [item for item in results if item[0] == "warning"]

    for level, number, message, path in results:
        if _IN_ACTIONS:
            print(
                gha.annotation(
                    level,
                    message,
                    title="Unpinned GitHub Action" if level == "error" else "Action pin comment",
                    file=str(path.relative_to(root)) if path.is_relative_to(root) else str(path),
                    line=str(number) if number else "",
                )
            )
        else:
            print(f"{level}: {message}")

    print(
        f"Checked {len(workflow_files(root))} workflow file(s): "
        f"{len(errors)} unpinned action reference(s), {len(warnings)} missing "
        "version comment(s)."
    )

    if errors:
        print(
            f"::error title=Action pinning::{len(errors)} action reference(s) are not "
            "pinned to immutable commit SHAs."
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
