#!/usr/bin/env python3
"""Security review gate for sensitive contract paths.

Two modes:

  check.py gate --changed-files FILE --event EVENT_JSON
      Fails (exit 1) when the pull request touches a path listed in
      .github/sensitive-paths.yml and the "Security review" section of the
      pull request body has an unchecked box. Pull requests that touch no
      sensitive path always pass.

  check.py mapping
      Fails when .github/CODEOWNERS or .github/labeler.yml no longer cover
      every pattern in .github/sensitive-paths.yml, so the three files cannot
      drift apart silently.

Standard library only, so the CI job needs no dependency install.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SENSITIVE_PATHS = REPO_ROOT / ".github" / "sensitive-paths.yml"
CODEOWNERS = REPO_ROOT / ".github" / "CODEOWNERS"
LABELER = REPO_ROOT / ".github" / "labeler.yml"

SECTION_HEADING = "security review"
LABEL_PREFIX = "area: "


@dataclass
class Area:
    name: str
    description: str = ""
    paths: list[str] = field(default_factory=list)


# ─── sensitive-paths.yml ────────────────────────────────────────────────────


def parse_sensitive_paths(text: str) -> list[Area]:
    """Parses the restricted YAML shape used by .github/sensitive-paths.yml.

    Only three constructs are accepted: a top-level `area:` key, an indented
    `description:` scalar and an indented `paths:` list of `- item` scalars.
    Anything else is an error, so a malformed edit fails loudly instead of
    quietly dropping a protected path.
    """
    areas: list[Area] = []
    current: Area | None = None
    in_paths = False
    for lineno, raw in enumerate(text.splitlines(), 1):
        line = raw.split(" #", 1)[0].rstrip() if not raw.lstrip().startswith("#") else ""
        if not line.strip():
            continue
        indent = len(line) - len(line.lstrip(" "))
        body = line.strip()
        if indent == 0:
            if not body.endswith(":"):
                raise ValueError(f"line {lineno}: expected `area:`, got {body!r}")
            current = Area(body[:-1].strip())
            areas.append(current)
            in_paths = False
        elif current is None:
            raise ValueError(f"line {lineno}: indented line before any area")
        elif body.startswith("- "):
            if not in_paths:
                raise ValueError(f"line {lineno}: list item outside `paths:`")
            current.paths.append(_unquote(body[2:].strip()))
        elif body == "paths:":
            in_paths = True
        elif body.startswith("description:"):
            current.description = _unquote(body[len("description:") :].strip())
            in_paths = False
        else:
            raise ValueError(f"line {lineno}: unexpected {body!r}")
    for area in areas:
        if not area.paths:
            raise ValueError(f"area {area.name!r} lists no paths")
    return areas


def _unquote(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        return value[1:-1]
    return value


# ─── path matching ──────────────────────────────────────────────────────────


def _glob_to_regex(pattern: str) -> re.Pattern[str]:
    if pattern.endswith("/"):
        pattern += "**"
    out = []
    i = 0
    while i < len(pattern):
        if pattern.startswith("**", i):
            out.append(".*")
            i += 2
        elif pattern[i] == "*":
            out.append("[^/]*")
            i += 1
        elif pattern[i] == "?":
            out.append("[^/]")
            i += 1
        else:
            out.append(re.escape(pattern[i]))
            i += 1
    return re.compile("".join(out) + r"\Z")


def matches(pattern: str, path: str) -> bool:
    if path.startswith("./"):
        path = path[2:]
    return bool(_glob_to_regex(pattern).match(path))


def touched_areas(areas: list[Area], changed: list[str]) -> dict[str, list[str]]:
    """Maps each sensitive area to the changed files that fall inside it."""
    hits: dict[str, list[str]] = {}
    for area in areas:
        files = [f for f in changed if any(matches(p, f) for p in area.paths)]
        if files:
            hits[area.name] = files
    return hits


# ─── pull request body ──────────────────────────────────────────────────────

CHECKBOX = re.compile(r"^\s*[-*]\s+\[( |x|X)\]\s+(.*)$")


def security_section(body: str) -> str | None:
    """Returns the text under the `## Security review` heading, or None."""
    lines = (body or "").replace("\r\n", "\n").split("\n")
    start = None
    for i, line in enumerate(lines):
        m = re.match(r"^\s{0,3}(#{2,3})\s+(.*?)\s*#*\s*$", line)
        if not m:
            continue
        title = m.group(2).strip().lower()
        if start is None:
            if title.startswith(SECTION_HEADING):
                start = i + 1
                level = len(m.group(1))
        elif len(m.group(1)) <= level:
            return "\n".join(lines[start:i])
    return None if start is None else "\n".join(lines[start:])


def checklist_items(section: str) -> list[tuple[bool, str]]:
    items = []
    in_comment = False
    for line in section.split("\n"):
        if "<!--" in line:
            in_comment = "-->" not in line.split("<!--", 1)[1]
            continue
        if in_comment:
            in_comment = "-->" not in line
            continue
        m = CHECKBOX.match(line)
        if m:
            items.append((m.group(1) != " ", m.group(2).strip()))
    return items


def evaluate(areas: list[Area], changed: list[str], body: str) -> tuple[bool, str]:
    """Returns (passed, report) for one pull request."""
    hits = touched_areas(areas, changed)
    if not hits:
        return True, "No sensitive paths changed; the security checklist is not required."

    report = ["This pull request changes sensitive paths:"]
    for name, files in hits.items():
        report.append(f"  {name}:")
        report.extend(f"    - {f}" for f in files)

    section = security_section(body)
    if section is None:
        report.append(
            "\nFAIL: the pull request body has no `## Security review` section. "
            "Copy it from .github/PULL_REQUEST_TEMPLATE.md and complete it."
        )
        return False, "\n".join(report)

    items = checklist_items(section)
    if not items:
        report.append("\nFAIL: the `## Security review` section has no checklist items.")
        return False, "\n".join(report)

    unchecked = [text for done, text in items if not done]
    if unchecked:
        report.append(
            "\nFAIL: every Security review item must be ticked. Tick an item that "
            "does not apply and write `N/A` with the reason next to it."
        )
        report.extend(f"  [ ] {text}" for text in unchecked)
        return False, "\n".join(report)

    report.append(f"\nPASS: all {len(items)} Security review items are ticked.")
    return True, "\n".join(report)


# ─── mapping drift check ────────────────────────────────────────────────────


def codeowners_patterns(text: str) -> set[str]:
    out = set()
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) >= 2:
            out.add(parts[0].lstrip("/"))
    return out


def labeler_globs(text: str) -> dict[str, set[str]]:
    """Extracts `label -> globs` from a labeler v5 config (flat list form)."""
    out: dict[str, set[str]] = {}
    label = None
    for raw in text.splitlines():
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        if not raw.startswith(" "):
            label = _unquote(raw.rstrip().rstrip(":").strip())
            out[label] = set()
            continue
        body = raw.strip()
        if label and body.startswith("- ") and ":" not in body:
            out[label].add(_unquote(body[2:].strip()))
    return out


def to_labeler_glob(pattern: str) -> str:
    return pattern + "**" if pattern.endswith("/") else pattern


def check_mapping(areas: list[Area], codeowners: str, labeler: str) -> list[str]:
    problems = []
    owned = codeowners_patterns(codeowners)
    labels = labeler_globs(labeler)
    for area in areas:
        label = LABEL_PREFIX + area.name
        if label not in labels:
            problems.append(f"labeler.yml has no `{label}` label")
        for pattern in area.paths:
            if pattern not in owned:
                problems.append(f"CODEOWNERS does not own `/{pattern}` ({area.name})")
            if label in labels and to_labeler_glob(pattern) not in labels[label]:
                problems.append(f"labeler.yml `{label}` is missing `{to_labeler_glob(pattern)}`")
    return problems


# ─── CLI ────────────────────────────────────────────────────────────────────


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    gate = sub.add_parser("gate")
    gate.add_argument("--changed-files", required=True, type=Path)
    gate.add_argument("--event", required=True, type=Path)
    sub.add_parser("mapping")
    args = parser.parse_args(argv)

    areas = parse_sensitive_paths(SENSITIVE_PATHS.read_text())

    if args.cmd == "mapping":
        problems = check_mapping(areas, CODEOWNERS.read_text(), LABELER.read_text())
        for p in problems:
            print(f"FAIL: {p}")
        if not problems:
            print(f"OK: CODEOWNERS and labeler.yml cover all {len(areas)} sensitive areas.")
        return 1 if problems else 0

    changed = [l.strip() for l in args.changed_files.read_text().splitlines() if l.strip()]
    event = json.loads(args.event.read_text())
    body = (event.get("pull_request") or {}).get("body") or ""
    passed, report = evaluate(areas, changed, body)
    print(report)
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
