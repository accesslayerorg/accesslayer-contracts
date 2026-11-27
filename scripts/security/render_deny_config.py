#!/usr/bin/env python3
"""Render a cargo-deny config that contains only reviewed advisory suppressions.

The checked-in ``deny.toml`` must keep ``ignore = []`` in its ``[advisories]``
section. This script validates ``.github/security/suppressions.toml`` (reason,
owner and unexpired date are mandatory) and injects those reviewed entries into
a temporary config that CI passes to ``cargo deny --config``.

Because cargo-deny never reads the suppression file directly, a suppression
cannot be smuggled in as a bare advisory id: the only path into the deny config
is through the validated policy file.
"""

from __future__ import annotations

import argparse
import json
import sys
import tomllib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gha  # noqa: E402
from suppressions import PolicyError, load_suppressions  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_CONFIG = REPO_ROOT / "deny.toml"
DEFAULT_SUPPRESSIONS = REPO_ROOT / ".github/security/suppressions.toml"
DEFAULT_OUTPUT = REPO_ROOT / "target/security/deny.toml"
MARKER = "ignore = []"


def render(config_text: str, advisories: list[dict]) -> str:
    """Return the config text with reviewed advisory ignores injected."""

    if config_text.count(MARKER) != 1:
        raise PolicyError(
            f"deny.toml must contain exactly one '{MARKER}' line in [advisories] "
            f"(found {config_text.count(MARKER)}); suppressions are injected there"
        )

    if advisories:
        lines = ["ignore = ["]
        for entry in advisories:
            key = "crate" if "@" in entry["id"] else "id"
            lines.append(
                f"    {{ {key} = {json.dumps(entry['id'])}, "
                f"reason = {json.dumps(entry['reason'])}, "
                f"expiry = {json.dumps(entry['expires'])} }},"
            )
        lines.append("]")
        replacement = "\n".join(lines)
    else:
        replacement = MARKER

    rendered = config_text.replace(MARKER, replacement, 1)

    # Fail closed if the rendered document would not be valid TOML or if the
    # ignores did not land where cargo-deny expects them.
    parsed = tomllib.loads(rendered)
    ignores = parsed.get("advisories", {}).get("ignore", [])
    if len(ignores) != len(advisories):
        raise PolicyError(
            "rendered deny config does not contain the expected advisory ignores"
        )

    header = (
        "# GENERATED FILE - do not edit.\n"
        "# Source: deny.toml + .github/security/suppressions.toml\n"
        "# Regenerate with: python3 scripts/security/render_deny_config.py\n\n"
    )
    return header + rendered


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", default=str(DEFAULT_CONFIG))
    parser.add_argument("--suppressions", default=str(DEFAULT_SUPPRESSIONS))
    parser.add_argument("--output", default=str(DEFAULT_OUTPUT))
    parser.add_argument("--stdout", action="store_true", help="write to stdout instead")
    return parser


def main(argv=None) -> int:
    args = build_parser().parse_args(argv)

    try:
        suppressions = load_suppressions(args.suppressions)
        config_path = Path(args.config)
        rendered = render(
            config_path.read_text(encoding="utf-8"), suppressions["advisories"]
        )
    except (OSError, PolicyError) as exc:
        print(gha.error(str(exc), title="cargo-deny config"))
        print(f"cargo-deny config failed: {exc}", file=sys.stderr)
        return 1

    if args.stdout:
        print(rendered)
        return 0

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(rendered, encoding="utf-8")
    print(
        f"Rendered {output} with {len(suppressions['advisories'])} reviewed "
        "advisory suppression(s)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
