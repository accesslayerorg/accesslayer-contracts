"""Shared loader/validator for this repository's reviewed security suppressions.

Every consumer of ``.github/security/suppressions.toml`` (the Scout baseline
gate, the cargo-deny config renderer and the standalone CI validation step)
imports this module so the reason/owner/expiry policy can never diverge or be
bypassed by a tool that reads the file differently.

Policy:
  * every entry needs a ``reason`` of at least 20 characters that is not a
    placeholder,
  * every entry needs an ``owner``,
  * every entry needs an ``expires`` date that is in the future and at most
    ``MAX_SUPPRESSION_DAYS`` away, so suppressions are re-reviewed at least
    once a year,
  * unknown keys are rejected rather than silently ignored.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import re
import tomllib
from pathlib import Path

MIN_REASON_LENGTH = 20
MAX_SUPPRESSION_DAYS = 366

SCOUT_KIND = "scout"
ADVISORY_KIND = "advisories"

_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
_FINGERPRINT_RE = re.compile(r"^[0-9a-f]{64}$")
_ADVISORY_RE = re.compile(
    r"^(RUSTSEC-\d{4}-\d{4}|[A-Za-z0-9_.-]+@[0-9A-Za-z_.*^<>=+~-]+)$"
)

_SCOUT_ALLOWED = {"detector", "file", "fingerprint", "reason", "expires", "owner", "ticket"}
_SCOUT_REQUIRED = ("detector", "file", "fingerprint", "reason", "expires", "owner")
_ADVISORY_ALLOWED = {"id", "reason", "expires", "owner", "ticket"}
_ADVISORY_REQUIRED = ("id", "reason", "expires", "owner")

_PLACEHOLDER_REASONS = {
    "",
    "-",
    "n/a",
    "na",
    "none",
    "tbd",
    "tba",
    "todo",
    "fixme",
    "later",
    "temporary",
    "false positive",
    "known issue",
}

_TOP_LEVEL = {SCOUT_KIND, ADVISORY_KIND}


class PolicyError(Exception):
    """The suppression file does not satisfy the repository policy."""


def scout_fingerprint(detector: str, file_path: str, snippet: str) -> str:
    """Return the stable fingerprint used to recognise a reviewed Scout finding.

    Whitespace inside the snippet is normalised so reformatting does not change
    the identity of a finding, while any change to the flagged code does. That
    forces a fresh review when the code under a baseline entry is edited.
    """

    normalized = " ".join(snippet.split())
    payload = "\n".join((detector.strip(), normalize_path(file_path), normalized))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def normalize_path(path: str) -> str:
    """Normalise a report path to the repository-relative POSIX form."""

    cleaned = str(path).strip().replace("\\", "/")
    while cleaned.startswith("./"):
        cleaned = cleaned[2:]
    return cleaned


def parse_date(value: str, today: dt.date, where: str) -> dt.date:
    """Parse and enforce the expiry rules for one suppression entry."""

    if not isinstance(value, str) or not _DATE_RE.match(value.strip()):
        raise PolicyError(
            f"{where}: 'expires' must be a date in YYYY-MM-DD form, got {value!r}"
        )
    try:
        expires = dt.date.fromisoformat(value.strip())
    except ValueError as exc:
        raise PolicyError(f"{where}: 'expires' is not a valid date: {value!r}") from exc

    if expires < today:
        raise PolicyError(
            f"{where}: suppression expired on {expires.isoformat()}. "
            "Fix the finding or re-review it and extend the expiry date."
        )

    horizon = today + dt.timedelta(days=MAX_SUPPRESSION_DAYS)
    if expires > horizon:
        raise PolicyError(
            f"{where}: expiry {expires.isoformat()} is more than "
            f"{MAX_SUPPRESSION_DAYS} days away. Suppressions must be re-reviewed "
            f"at least yearly (latest allowed: {horizon.isoformat()})."
        )
    return expires


def _check_reason(entry: dict, where: str) -> None:
    reason = entry.get("reason")
    if not isinstance(reason, str) or not reason.strip():
        raise PolicyError(f"{where}: 'reason' is required")
    reason = " ".join(reason.split()).strip()
    if reason.lower() in _PLACEHOLDER_REASONS:
        raise PolicyError(
            f"{where}: 'reason' is a placeholder ({reason!r}); explain why the "
            "finding is safe to accept"
        )
    if len(reason) < MIN_REASON_LENGTH:
        raise PolicyError(
            f"{where}: 'reason' must be at least {MIN_REASON_LENGTH} characters, "
            f"got {len(reason)}"
        )
    entry["reason"] = reason


def _check_owner(entry: dict, where: str) -> None:
    owner = entry.get("owner")
    if not isinstance(owner, str) or not owner.strip():
        raise PolicyError(f"{where}: 'owner' is required (person or team accountable)")


def _check_fields(entry: dict, required, allowed, where: str) -> None:
    if not isinstance(entry, dict):
        raise PolicyError(f"{where}: expected a table entry")
    unknown = set(entry) - set(allowed)
    if unknown:
        raise PolicyError(
            f"{where}: unknown field(s) {sorted(unknown)}; allowed: {sorted(allowed)}"
        )
    missing = [field for field in required if field not in entry]
    if missing:
        raise PolicyError(f"{where}: missing required field(s) {missing}")


def _validate_scout(entries, today: dt.date) -> list[dict]:
    validated: list[dict] = []
    seen: dict[tuple[str, str, str], int] = {}

    for index, entry in enumerate(entries, start=1):
        where = f"[[scout]] entry {index}"
        _check_fields(entry, _SCOUT_REQUIRED, _SCOUT_ALLOWED, where)

        detector = entry.get("detector")
        if not isinstance(detector, str) or not detector.strip():
            raise PolicyError(f"{where}: 'detector' must be a non-empty string")

        file_path = normalize_path(entry.get("file", ""))
        if not file_path:
            raise PolicyError(f"{where}: 'file' must be a non-empty path")
        if file_path.startswith("/") or ".." in Path(file_path).parts:
            raise PolicyError(
                f"{where}: 'file' must be a repository-relative path, got {file_path!r}"
            )

        fingerprint = entry.get("fingerprint")
        if not isinstance(fingerprint, str) or not _FINGERPRINT_RE.match(
            fingerprint.strip()
        ):
            raise PolicyError(
                f"{where}: 'fingerprint' must be a 64 character lowercase SHA-256 hex digest"
            )
        fingerprint = fingerprint.strip()

        _check_reason(entry, where)
        _check_owner(entry, where)
        expires = parse_date(entry.get("expires"), today, where)

        key = (detector.strip(), file_path, fingerprint)
        if key in seen:
            raise PolicyError(
                f"{where}: duplicate of [[scout]] entry {seen[key]} "
                f"({detector} in {file_path})"
            )
        seen[key] = index

        validated.append(
            {
                "detector": detector.strip(),
                "file": file_path,
                "fingerprint": fingerprint,
                "reason": entry["reason"],
                "owner": entry["owner"].strip(),
                "expires": expires.isoformat(),
                "ticket": str(entry.get("ticket", "")).strip(),
            }
        )

    return validated


def _validate_advisories(entries, today: dt.date) -> list[dict]:
    validated: list[dict] = []
    seen: dict[str, int] = {}

    for index, entry in enumerate(entries, start=1):
        where = f"[[advisories]] entry {index}"
        _check_fields(entry, _ADVISORY_REQUIRED, _ADVISORY_ALLOWED, where)

        advisory_id = entry.get("id")
        if not isinstance(advisory_id, str) or not _ADVISORY_RE.match(advisory_id.strip()):
            raise PolicyError(
                f"{where}: 'id' must be a RustSec id (RUSTSEC-YYYY-NNNN) or a "
                f"crate spec (crate@version), got {advisory_id!r}"
            )
        advisory_id = advisory_id.strip()

        _check_reason(entry, where)
        _check_owner(entry, where)
        expires = parse_date(entry.get("expires"), today, where)

        if advisory_id in seen:
            raise PolicyError(
                f"{where}: duplicate of [[advisories]] entry {seen[advisory_id]} "
                f"({advisory_id})"
            )
        seen[advisory_id] = index

        validated.append(
            {
                "id": advisory_id,
                "reason": entry["reason"],
                "owner": entry["owner"].strip(),
                "expires": expires.isoformat(),
                "ticket": str(entry.get("ticket", "")).strip(),
            }
        )

    return validated


def load_suppressions(path, today: dt.date | None = None) -> dict[str, list[dict]]:
    """Load and validate the suppression file, raising ``PolicyError`` on failure."""

    path = Path(path)
    if not path.is_file():
        raise PolicyError(f"suppression file not found: {path}")

    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError) as exc:
        raise PolicyError(f"{path}: cannot read suppression file: {exc}") from exc
    except tomllib.TOMLDecodeError as exc:
        raise PolicyError(f"{path}: invalid TOML: {exc}") from exc

    unknown = set(data) - _TOP_LEVEL
    if unknown:
        raise PolicyError(
            f"{path}: unknown top-level key(s) {sorted(unknown)}; "
            f"allowed: {sorted(_TOP_LEVEL)}"
        )

    today = today or dt.date.today()
    return {
        SCOUT_KIND: _validate_scout(data.get(SCOUT_KIND, []), today),
        ADVISORY_KIND: _validate_advisories(data.get(ADVISORY_KIND, []), today),
    }
