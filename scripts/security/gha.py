"""Small helpers for GitHub Actions workflow commands and step summaries.

They keep the security scripts free of duplicated escaping logic and keep the
annotation behaviour testable.
"""

from __future__ import annotations

import os
from pathlib import Path


def escape_data(value: str) -> str:
    """Escape a workflow command payload (see GitHub Actions command escaping)."""

    return (
        str(value)
        .replace("%", "%25")
        .replace("\r", "%0D")
        .replace("\n", "%0A")
    )


def escape_property(value: str) -> str:
    """Escape a workflow command property value."""

    return escape_data(value).replace(":", "%3A").replace(",", "%2C")


def annotation(level: str, message: str, **properties: str) -> str:
    """Build a workflow command line (``::error``, ``::warning`` or ``::notice``)."""

    props = ",".join(
        f"{key}={escape_property(value)}"
        for key, value in properties.items()
        if value not in (None, "")
    )
    prefix = f"::{level}" + (f" {props}" if props else "") + "::"
    return f"{prefix}{escape_data(message)}"


def error(message: str, **properties: str) -> str:
    return annotation("error", message, **properties)


def warning(message: str, **properties: str) -> str:
    return annotation("warning", message, **properties)


def notice(message: str, **properties: str) -> str:
    return annotation("notice", message, **properties)


def append_step_summary(markdown: str, summary_path: Path | None = None) -> None:
    """Append Markdown to the job summary when running inside GitHub Actions."""

    if summary_path is None:
        env_path = os.environ.get("GITHUB_STEP_SUMMARY")
        if not env_path:
            return
        summary_path = Path(env_path)
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    with summary_path.open("a", encoding="utf-8") as handle:
        handle.write(markdown)
        if not markdown.endswith("\n"):
            handle.write("\n")
