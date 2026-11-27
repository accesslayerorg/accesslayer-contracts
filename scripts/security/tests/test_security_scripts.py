"""Unit tests for the CI security policy scripts.

Run with:
    python3 -m unittest discover -s scripts/security/tests
"""

from __future__ import annotations

import contextlib
import datetime as dt
import io
import json
import os
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

SECURITY_DIR = Path(__file__).resolve().parents[1]
REPO_ROOT = SECURITY_DIR.parents[1]
sys.path.insert(0, str(SECURITY_DIR))

import check_pinned_actions  # noqa: E402
import check_scout  # noqa: E402
import create_audit_issues  # noqa: E402
import gha  # noqa: E402
import render_deny_config  # noqa: E402
import suppressions  # noqa: E402

TODAY = dt.date(2026, 11, 26)
FINGERPRINT = "a" * 64

VALID_FILE = """
[[scout]]
detector = "unsafe-unwrap"
file = "creator-keys/src/lib.rs"
fingerprint = "{fingerprint}"
reason = "The value is initialised directly above and cannot be absent."
owner = "@accesslayer-security"
expires = "2027-06-30"

[[advisories]]
id = "RUSTSEC-2026-0387"
reason = "Timing side channels do not apply to public on-chain inputs."
owner = "@accesslayer-security"
expires = "2027-06-30"
""".format(fingerprint=FINGERPRINT)


def write_toml(directory: Path, text: str) -> Path:
    path = directory / "suppressions.toml"
    path.write_text(text, encoding="utf-8")
    return path


class SuppressionPolicyTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def load(self, text: str, today: dt.date = TODAY):
        return suppressions.load_suppressions(write_toml(self.tmp, text), today=today)

    def assert_policy_error(self, text: str, fragment: str, today: dt.date = TODAY):
        with self.assertRaises(suppressions.PolicyError) as ctx:
            self.load(text, today=today)
        self.assertIn(fragment, str(ctx.exception))

    def test_valid_entries_load(self):
        data = self.load(VALID_FILE)
        self.assertEqual(len(data["scout"]), 1)
        self.assertEqual(len(data["advisories"]), 1)
        self.assertEqual(data["scout"][0]["fingerprint"], FINGERPRINT)
        self.assertEqual(data["advisories"][0]["expires"], "2027-06-30")

    def test_missing_reason_is_rejected(self):
        text = VALID_FILE.replace(
            'reason = "The value is initialised directly above and cannot be absent."\n', ""
        )
        self.assert_policy_error(text, "missing required field(s) ['reason']")

    def test_placeholder_reason_is_rejected(self):
        self.assert_policy_error(
            VALID_FILE.replace(
                "The value is initialised directly above and cannot be absent.", "todo"
            ),
            "placeholder",
        )

    def test_short_reason_is_rejected(self):
        self.assert_policy_error(
            VALID_FILE.replace(
                "The value is initialised directly above and cannot be absent.", "safe"
            ),
            "at least 20 characters",
        )

    def test_missing_expiry_is_rejected(self):
        self.assert_policy_error(
            VALID_FILE.replace('expires = "2027-06-30"\n', "", 1),
            "missing required field(s) ['expires']",
        )

    def test_expired_suppression_is_rejected(self):
        self.assert_policy_error(VALID_FILE, "suppression expired on 2027-06-30", dt.date(2027, 7, 1))

    def test_far_future_expiry_is_rejected(self):
        self.assert_policy_error(
            VALID_FILE.replace('expires = "2027-06-30"', 'expires = "2030-01-01"'),
            "at least yearly",
        )

    def test_malformed_expiry_is_rejected(self):
        self.assert_policy_error(
            VALID_FILE.replace('expires = "2027-06-30"', 'expires = "next-year"'),
            "YYYY-MM-DD",
        )

    def test_missing_owner_is_rejected(self):
        self.assert_policy_error(
            VALID_FILE.replace('owner = "@accesslayer-security"\n', "", 1),
            "missing required field(s) ['owner']",
        )

    def test_unknown_field_is_rejected(self):
        self.assert_policy_error(
            VALID_FILE.replace(
                'owner = "@accesslayer-security"',
                'owner = "@accesslayer-security"\nsilenced = true',
                1,
            ),
            "unknown field",
        )

    def test_duplicate_scout_entry_is_rejected(self):
        text = VALID_FILE + VALID_FILE.split("[[advisories]]")[0].replace(
            "[[scout]]", "[[scout]]"
        )
        self.assert_policy_error(text, "duplicate of [[scout]] entry")

    def test_duplicate_advisory_is_rejected(self):
        advisory = VALID_FILE.split("[[advisories]]")[1]
        self.assert_policy_error(VALID_FILE + "[[advisories]]" + advisory, "duplicate of")

    def test_path_traversal_is_rejected(self):
        self.assert_policy_error(
            VALID_FILE.replace("creator-keys/src/lib.rs", "../outside.rs"),
            "repository-relative",
        )

    def test_unknown_top_level_key_is_rejected(self):
        self.assert_policy_error(VALID_FILE + "\n[[ignore]]\nid = 1\n", "unknown top-level key")

    def test_invalid_fingerprint_is_rejected(self):
        self.assert_policy_error(
            VALID_FILE.replace(FINGERPRINT, "deadbeef"), "SHA-256"
        )

    def test_crate_version_spec_is_allowed(self):
        data = self.load(
            VALID_FILE.replace('id = "RUSTSEC-2026-0387"', 'id = "crypto-bigint@0.5.5"')
        )
        self.assertEqual(data["advisories"][0]["id"], "crypto-bigint@0.5.5")

    def test_repository_suppression_file_is_valid(self):
        data = suppressions.load_suppressions(
            REPO_ROOT / ".github/security/suppressions.toml"
        )
        self.assertIn("scout", data)
        self.assertIn("advisories", data)
        for entry in data["scout"] + data["advisories"]:
            self.assertTrue(entry["reason"])
            self.assertTrue(entry["owner"])
            self.assertTrue(entry["expires"])

    def test_audit_ignore_ids_only_emits_rustsec_ids(self):
        import check_suppressions

        path = write_toml(
            self.tmp,
            VALID_FILE
            + "\n[[advisories]]\n"
            + 'id = "crypto-bigint@0.5.5"\n'
            + 'reason = "Kept until the parent dependency can be upgraded."\n'
            + 'owner = "@accesslayerorg"\nexpires = "2027-06-30"\n',
        )
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = check_suppressions.main(
                ["--file", str(path), "--audit-ignore-ids", "--today", TODAY.isoformat()]
            )
        self.assertEqual(result, 0)
        self.assertEqual(output.getvalue().strip(), "RUSTSEC-2026-0387")

    def test_deny_config_enforces_policy(self):
        config = tomllib.loads((REPO_ROOT / "deny.toml").read_text(encoding="utf-8"))

        self.assertEqual(config["advisories"]["ignore"], [])
        self.assertEqual(config["advisories"]["ignore-expiry"], "P0D")
        self.assertEqual(config["advisories"]["unused-ignored-advisory"], "deny")
        self.assertEqual(config["advisories"]["yanked"], "deny")
        self.assertEqual(config["sources"]["unknown-registry"], "deny")
        self.assertEqual(config["sources"]["unknown-git"], "deny")
        self.assertTrue(config["licenses"]["private"]["ignore"])
        self.assertEqual(config["bans"]["multiple-versions"], "warn")

        allowed = config["licenses"]["allow"]
        self.assertIn("MIT", allowed)
        self.assertIn("Apache-2.0", allowed)
        for copyleft in ("GPL-3.0", "AGPL-3.0", "SSPL-1.0"):
            self.assertNotIn(copyleft, allowed)


class RenderDenyConfigTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_empty_suppressions_keep_placeholder(self):
        source = (REPO_ROOT / "deny.toml").read_text(encoding="utf-8")
        rendered = render_deny_config.render(source, [])
        parsed = tomllib.loads(rendered)
        self.assertEqual(parsed["advisories"]["ignore"], [])

    def test_reviewed_suppressions_are_injected_with_expiry(self):
        source = (REPO_ROOT / "deny.toml").read_text(encoding="utf-8")
        rendered = render_deny_config.render(
            source,
            [
                {
                    "id": "RUSTSEC-2026-0387",
                    "reason": "Timing side channels do not apply to public inputs.",
                    "expires": "2027-06-30",
                }
            ],
        )
        parsed = tomllib.loads(rendered)
        ignore = parsed["advisories"]["ignore"]
        self.assertEqual(len(ignore), 1)
        self.assertEqual(ignore[0]["id"], "RUSTSEC-2026-0387")
        self.assertEqual(ignore[0]["expiry"], "2027-06-30")
        self.assertIn("reason", ignore[0])

    def test_missing_marker_fails_closed(self):
        with self.assertRaises(suppressions.PolicyError):
            render_deny_config.render("advisories = {}\n", [])

    def test_main_rejects_expired_suppression(self):
        path = write_toml(
            self.tmp, VALID_FILE.replace('expires = "2027-06-30"', 'expires = "2026-01-01"')
        )
        output = self.tmp / "deny.generated.toml"
        with contextlib.redirect_stdout(io.StringIO()):
            result = render_deny_config.main(
                [
                    "--config",
                    str(REPO_ROOT / "deny.toml"),
                    "--suppressions",
                    str(path),
                    "--output",
                    str(output),
                ]
            )
        self.assertEqual(result, 1)
        self.assertFalse(output.exists())

    def test_main_writes_rendered_config(self):
        path = write_toml(self.tmp, VALID_FILE)
        output = self.tmp / "deny.generated.toml"
        with contextlib.redirect_stdout(io.StringIO()):
            result = render_deny_config.main(
                [
                    "--config",
                    str(REPO_ROOT / "deny.toml"),
                    "--suppressions",
                    str(path),
                    "--output",
                    str(output),
                ]
            )
        self.assertEqual(result, 0)
        parsed = tomllib.loads(output.read_text(encoding="utf-8"))
        self.assertEqual(parsed["advisories"]["ignore"][0]["id"], "RUSTSEC-2026-0387")


def scout_report() -> dict:
    return {
        "name": "accesslayer-contracts",
        "summary": {"total_vulnerabilities": 2},
        "categories": [
            {
                "id": "vulnerabilities",
                "name": "Vulnerabilities",
                "vulnerabilities": [
                    {
                        "id": "unprotected-update-current-contract-wasm",
                        "severity": "Critical",
                        "name": "Unprotected update of the current contract wasm",
                    }
                ],
            },
            {
                "id": "best-practices",
                "name": "Best practices",
                "vulnerabilities": [
                    {"id": "unsafe-unwrap", "severity": "Medium", "name": "Unsafe unwrap"}
                ],
            },
        ],
        "findings": [
            {
                "id": 0,
                "occurrence_index": 1,
                "category_id": "vulnerabilities",
                "vulnerability_id": "unprotected-update-current-contract-wasm",
                "error_message": "The contract wasm is updated without checking the caller.",
                "span": "lib.rs:10:5 - 12:9",
                "code_snippet": "env.deployer().update_current_contract_wasm(new_wasm);",
                "package": "creator-keys",
                "file_path": "creator-keys/src/lib.rs",
            },
            {
                "id": 1,
                "occurrence_index": 1,
                "category_id": "best-practices",
                "vulnerability_id": "unsafe-unwrap",
                "error_message": "Unwrap may panic.",
                "span": "lib.rs:40:9 - 40:30",
                "code_snippet": "let value = maybe.unwrap();",
                "package": "creator-keys",
                "file_path": "creator-keys/src/lib.rs",
            },
        ],
    }


class ScoutGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)
        self.report_path = self.tmp / "scout-report.json"
        self.report_path.write_text(json.dumps(scout_report()), encoding="utf-8")
        self.summary = self.tmp / "summary.md"

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def run_main(self, suppressions_path: Path, *extra: str) -> tuple[int, str]:
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = check_scout.main(
                [
                    "--report",
                    str(self.report_path),
                    "--suppressions",
                    str(suppressions_path),
                    "--summary-out",
                    str(self.summary),
                    "--today",
                    TODAY.isoformat(),
                    *extra,
                ]
            )
        return result, output.getvalue()

    def suppress_critical(self) -> Path:
        finding = check_scout.iter_findings(scout_report())[0]
        text = f"""
[[scout]]
detector = "{finding.detector}"
file = "{finding.file}"
fingerprint = "{finding.fingerprint}"
reason = "The wasm update is guarded by the admin check in the caller above."
owner = "@accesslayer-security"
expires = "2027-06-30"
"""
        return write_toml(self.tmp, text)

    def test_report_is_parsed_with_severity_and_location(self):
        findings = check_scout.iter_findings(scout_report())
        self.assertEqual(findings[0].severity, "critical")
        self.assertEqual(findings[0].line, 10)
        self.assertEqual(findings[0].column, 5)
        self.assertEqual(findings[1].severity, "medium")

    def test_new_critical_finding_fails_default_threshold(self):
        result, _ = self.run_main(write_toml(self.tmp, ""))
        self.assertEqual(result, 1)
        self.assertTrue(self.summary.exists())
        self.assertIn("Blocking findings", self.summary.read_text(encoding="utf-8"))

    def test_medium_finding_does_not_fail_default_threshold(self):
        finding = check_scout.iter_findings(scout_report())[0]
        text = (
            f'[[scout]]\ndetector = "{finding.detector}"\nfile = "{finding.file}"\n'
            f'fingerprint = "{finding.fingerprint}"\n'
            'reason = "The wasm update is guarded by the admin check in the caller above."\n'
            'owner = "@accesslayer-security"\nexpires = "2027-06-30"\n'
        )
        result, output = self.run_main(write_toml(self.tmp, text))
        self.assertEqual(result, 0, output)

    def test_baselined_critical_finding_passes(self):
        result, output = self.run_main(self.suppress_critical())
        self.assertEqual(result, 0, output)

    def test_changed_code_invalidates_baseline(self):
        path = self.suppress_critical()
        report = scout_report()
        report["findings"][0]["code_snippet"] += " // changed"
        self.report_path.write_text(json.dumps(report), encoding="utf-8")
        result, _ = self.run_main(path)
        self.assertEqual(result, 1)

    def test_lower_threshold_blocks_medium(self):
        finding = check_scout.iter_findings(scout_report())[0]
        text = (
            f'[[scout]]\ndetector = "{finding.detector}"\nfile = "{finding.file}"\n'
            f'fingerprint = "{finding.fingerprint}"\n'
            'reason = "The wasm update is guarded by the admin check in the caller above."\n'
            'owner = "@accesslayer-security"\nexpires = "2027-06-30"\n'
        )
        result, _ = self.run_main(write_toml(self.tmp, text), "--fail-on", "medium")
        self.assertEqual(result, 1)

    def test_expired_baseline_entry_fails_closed(self):
        path = self.suppress_critical()
        path.write_text(
            path.read_text(encoding="utf-8").replace("2027-06-30", "2026-01-01"),
            encoding="utf-8",
        )
        result, output = self.run_main(path)
        self.assertEqual(result, 1)
        self.assertIn("expired", output)

    def test_missing_report_fails(self):
        self.report_path.unlink()
        result, output = self.run_main(write_toml(self.tmp, ""))
        self.assertEqual(result, 1)
        self.assertIn("Scout report not found", output)

    def test_clean_report_passes(self):
        self.report_path.write_text(
            json.dumps({"categories": [], "findings": []}), encoding="utf-8"
        )
        result, output = self.run_main(write_toml(self.tmp, ""))
        self.assertEqual(result, 0, output)

    def test_suppression_template_is_generated(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = check_scout.main(
                [
                    "--report",
                    str(self.report_path),
                    "--suppressions",
                    str(write_toml(self.tmp, "")),
                    "--print-suppression-template",
                ]
            )
        self.assertEqual(result, 0)
        text = output.getvalue()
        self.assertIn("[[scout]]", text)
        self.assertIn("unprotected-update-current-contract-wasm", text)
        self.assertIn('reason = ""', text)
        self.assertIn('expires = ""', text)

    def test_stale_baseline_entry_is_reported(self):
        path = write_toml(
            self.tmp,
            f"""
[[scout]]
detector = "unsafe-unwrap"
file = "creator-keys/src/other.rs"
fingerprint = "{'b' * 64}"
reason = "Kept for a detector that reports the same pattern in generated code."
owner = "@accesslayer-security"
expires = "2027-06-30"
""",
        )
        result, _ = self.run_main(path)
        self.assertEqual(result, 1)
        self.assertIn("Stale baseline entries", self.summary.read_text(encoding="utf-8"))


class PinnedActionTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        (self.root / ".github/workflows").mkdir(parents=True)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def write_workflow(self, body: str) -> Path:
        path = self.root / ".github/workflows/ci.yml"
        path.write_text(body, encoding="utf-8")
        return path

    def test_classify(self):
        self.assertEqual(check_pinned_actions.classify("./local/action")[0], "ok")
        self.assertEqual(check_pinned_actions.classify("actions/checkout@v4")[0], "missing-sha")
        self.assertEqual(
            check_pinned_actions.classify(f"actions/checkout@{'c' * 40}")[0], "ok"
        )
        self.assertEqual(
            check_pinned_actions.classify("docker://alpine:3.20")[0], "missing-sha"
        )
        self.assertEqual(
            check_pinned_actions.classify(f"docker://alpine@sha256:{'d' * 64}")[0], "ok"
        )
        self.assertEqual(check_pinned_actions.classify("actions/checkout")[0], "missing-sha")

    def test_unpinned_actions_fail(self):
        self.write_workflow(
            "jobs:\n  test:\n    steps:\n      - uses: actions/checkout@v4\n"
        )
        results = check_pinned_actions.scan(self.root)
        self.assertEqual(len([item for item in results if item[0] == "error"]), 1)

    def test_pinned_action_passes(self):
        self.write_workflow(
            "jobs:\n  test:\n    steps:\n"
            f"      - uses: actions/checkout@{'c' * 40}  # v4.2.2\n"
        )
        results = check_pinned_actions.scan(self.root)
        self.assertEqual(results, [])

    def test_pinned_without_version_comment_warns(self):
        self.write_workflow(
            "jobs:\n  test:\n    steps:\n"
            f"      - uses: actions/checkout@{'c' * 40}\n"
        )
        results = check_pinned_actions.scan(self.root)
        self.assertEqual([item[0] for item in results], ["warning"])

    def test_local_action_and_run_blocks_are_ignored(self):
        self.write_workflow(
            "jobs:\n  test:\n    steps:\n"
            "      - uses: ./.github/actions/local  # local composite\n"
            "      - name: Script\n"
            "        run: |\n"
            "          echo 'uses: actions/checkout@v4'\n"
        )
        self.assertEqual(check_pinned_actions.scan(self.root), [])

    def test_repository_workflows_are_pinned(self):
        errors = [
            message
            for level, _, message, _ in check_pinned_actions.scan(REPO_ROOT)
            if level == "error"
        ]
        self.assertEqual(errors, [])

    def test_main_fails_on_unpinned(self):
        self.write_workflow(
            "jobs:\n  test:\n    steps:\n      - uses: actions/checkout@v4\n"
        )
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(check_pinned_actions.main(["--root", str(self.root)]), 1)


class AuditIssueTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def report(self) -> dict:
        return {
            "vulnerabilities": {
                "found": True,
                "count": 1,
                "list": [
                    {
                        "advisory": {
                            "id": "RUSTSEC-2026-0387",
                            "title": "Timing variability in crypto-bigint",
                            "description": "Constant-time operations may branch.",
                            "date": "2026-10-23",
                            "aliases": ["CVE-2026-0001"],
                            "url": "https://example.invalid/advisory",
                            "cvss": None,
                        },
                        "versions": {"patched": [">=0.7.6"], "unaffected": []},
                        "package": {
                            "name": "crypto-bigint",
                            "version": "0.5.5",
                            "source": "registry+https://github.com/rust-lang/crates.io-index",
                        },
                    }
                ],
            },
            "warnings": {"unmaintained": [{"package": {"name": "chrono"}}]},
        }

    def write_report(self) -> Path:
        path = self.tmp / "cargo-audit.json"
        path.write_text(json.dumps(self.report()), encoding="utf-8")
        return path

    def test_load_advisories(self):
        advisories = create_audit_issues.load_advisories(self.write_report())
        self.assertEqual(len(advisories), 1)
        self.assertEqual(advisories[0]["id"], "RUSTSEC-2026-0387")
        self.assertEqual(advisories[0]["crate"], "crypto-bigint")
        self.assertEqual(advisories[0]["patched"], [">=0.7.6"])

    def test_marker_round_trip(self):
        marker = create_audit_issues.issue_marker("RUSTSEC-2026-0387")
        found = create_audit_issues.existing_advisory_ids(
            ["Some text\n" + marker + "\nmore text"]
        )
        self.assertEqual(found, {"RUSTSEC-2026-0387"})
        self.assertEqual(create_audit_issues.existing_advisory_ids([]), set())

    def test_duplicate_issues_are_prevented(self):
        advisories = create_audit_issues.load_advisories(self.write_report())
        tracked = create_audit_issues.existing_advisory_ids(
            [create_audit_issues.issue_marker("RUSTSEC-2026-0387")]
        )
        self.assertEqual(create_audit_issues.partition_new(advisories, tracked), [])
        self.assertEqual(
            len(create_audit_issues.partition_new(advisories, set())), 1
        )

    def test_issue_body_contains_marker_and_remediation(self):
        title, body = create_audit_issues.build_issue(
            create_audit_issues.load_advisories(self.write_report())[0], "o/r"
        )
        self.assertIn("RUSTSEC-2026-0387", title)
        self.assertIn("crypto-bigint", title)
        self.assertIn("<!-- cargo-audit-advisory: RUSTSEC-2026-0387 -->", body)
        self.assertIn("cargo update -p crypto-bigint", body)
        self.assertIn("suppressions.toml", body)

    def empty_suppressions(self) -> Path:
        return write_toml(self.tmp, "")

    def test_dry_run_reports_issue_without_creating_it(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = create_audit_issues.main(
                [
                    "--report",
                    str(self.write_report()),
                    "--repo",
                    "owner/repo",
                    "--suppressions",
                    str(self.empty_suppressions()),
                    "--dry-run",
                ]
            )
        self.assertEqual(result, 0)
        self.assertIn("[dry-run] would create", output.getvalue())

    def test_suppressed_advisory_is_not_opened(self):
        advisories = create_audit_issues.load_advisories(self.write_report())
        entries = suppressions.load_suppressions(
            write_toml(self.tmp, VALID_FILE.replace("2027-06-30", "2027-06-30")), today=TODAY
        )["advisories"]
        accepted = create_audit_issues.suppressed_advisories(advisories, entries)
        self.assertEqual([item["id"] for item in accepted], ["RUSTSEC-2026-0387"])

    def test_crate_spec_suppression_matches(self):
        advisories = create_audit_issues.load_advisories(self.write_report())
        entries = [
            {
                "id": "crypto-bigint@0.5.5",
                "reason": "Not reachable from our code paths, reviewed by security.",
                "expires": "2027-06-30",
            }
        ]
        accepted = create_audit_issues.suppressed_advisories(advisories, entries)
        self.assertEqual(len(accepted), 1)

    def test_clean_report_is_not_an_error(self):
        path = self.tmp / "clean.json"
        path.write_text(
            json.dumps({"vulnerabilities": {"found": False, "count": 0, "list": []}}),
            encoding="utf-8",
        )
        with contextlib.redirect_stdout(io.StringIO()):
            result = create_audit_issues.main(
                [
                    "--report",
                    str(path),
                    "--repo",
                    "owner/repo",
                    "--suppressions",
                    str(self.empty_suppressions()),
                    "--dry-run",
                ]
            )
        self.assertEqual(result, 0)


class AnnotationTests(unittest.TestCase):
    def test_annotation_escapes_newlines_and_properties(self):
        line = gha.error("line one\nline two", title="a:b,c", file="src/lib.rs", line="4")
        self.assertTrue(line.startswith("::error "))
        self.assertIn("title=a%3Ab%2Cc", line)
        self.assertIn("line one%0Aline two", line)

    def test_step_summary_is_appended(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "summary.md"
            gha.append_step_summary("## Title", path)
            gha.append_step_summary("body", path)
            self.assertEqual(path.read_text(encoding="utf-8"), "## Title\nbody\n")


if __name__ == "__main__":
    unittest.main()
