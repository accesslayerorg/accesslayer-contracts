"""Unit tests for scripts/security_checklist/check.py (stdlib unittest)."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import check  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[3]

AREAS = check.parse_sensitive_paths(
    """
auth:
  description: Authorization
  paths:
    - access-control/
    - creator-keys/src/acl_*.rs
fees:
  description: Fees
  paths:
    - creator-keys/src/dynamic_fees.rs
"""
)

TICKED = """## Summary

- change

## Security review

<!--
- [ ] an example inside a comment does not count
-->

- [x] Authorization: reviewed
- [X] Arithmetic: N/A, no math changed

## Checklist

- [ ] Linked issue
"""

UNTICKED = TICKED.replace("- [X] Arithmetic", "- [ ] Arithmetic")


class ParseTests(unittest.TestCase):
    def test_parses_areas_and_paths(self):
        self.assertEqual([a.name for a in AREAS], ["auth", "fees"])
        self.assertEqual(AREAS[0].paths, ["access-control/", "creator-keys/src/acl_*.rs"])

    def test_rejects_unknown_keys(self):
        with self.assertRaises(ValueError):
            check.parse_sensitive_paths("auth:\n  owners: someone\n  paths:\n    - a/\n")

    def test_rejects_area_without_paths(self):
        with self.assertRaises(ValueError):
            check.parse_sensitive_paths("auth:\n  description: x\n")

    def test_repository_config_parses(self):
        areas = check.parse_sensitive_paths(check.SENSITIVE_PATHS.read_text())
        names = {a.name for a in areas}
        self.assertTrue({"auth", "fees", "reserve", "upgrade", "storage"} <= names)


class MatchTests(unittest.TestCase):
    def test_directory_prefix(self):
        self.assertTrue(check.matches("access-control/", "access-control/src/lib.rs"))
        self.assertFalse(check.matches("access-control/", "access-control-extra/src/lib.rs"))

    def test_single_star_stays_in_segment(self):
        self.assertTrue(check.matches("creator-keys/src/acl_*.rs", "creator-keys/src/acl_roles.rs"))
        self.assertFalse(check.matches("creator-keys/src/acl_*.rs", "creator-keys/src/acl_x/y.rs"))

    def test_exact_file_and_leading_dot_slash(self):
        self.assertTrue(check.matches("common/src/guard.rs", "./common/src/guard.rs"))
        self.assertFalse(check.matches("common/src/guard.rs", "common/src/guard.rs.bak"))


class GateTests(unittest.TestCase):
    def test_non_sensitive_change_passes_without_checklist(self):
        passed, report = check.evaluate(AREAS, ["docs/testing.md", "badge-contract/src/lib.rs"], "")
        self.assertTrue(passed)
        self.assertIn("not required", report)

    def test_sensitive_change_without_section_fails(self):
        passed, report = check.evaluate(AREAS, ["access-control/src/lib.rs"], "## Summary\n- x\n")
        self.assertFalse(passed)
        self.assertIn("no `## Security review` section", report)

    def test_sensitive_change_with_unticked_item_fails(self):
        passed, report = check.evaluate(AREAS, ["creator-keys/src/dynamic_fees.rs"], UNTICKED)
        self.assertFalse(passed)
        self.assertIn("[ ] Arithmetic", report)
        self.assertIn("fees:", report)

    def test_sensitive_change_with_all_ticked_passes(self):
        passed, report = check.evaluate(AREAS, ["creator-keys/src/acl_roles.rs"], TICKED)
        self.assertTrue(passed, report)
        self.assertIn("all 2 Security review items", report)

    def test_other_sections_do_not_count(self):
        # The unticked "Linked issue" box lives under ## Checklist, not the gate.
        passed, _ = check.evaluate(AREAS, ["access-control/src/lib.rs"], TICKED)
        self.assertTrue(passed)

    def test_section_with_no_items_fails(self):
        body = "## Security review\n\nLooks fine.\n\n## Checklist\n- [x] ok\n"
        passed, report = check.evaluate(AREAS, ["access-control/src/lib.rs"], body)
        self.assertFalse(passed)
        self.assertIn("no checklist items", report)

    def test_crlf_body(self):
        passed, _ = check.evaluate(AREAS, ["access-control/src/lib.rs"], TICKED.replace("\n", "\r\n"))
        self.assertTrue(passed)

    def test_template_section_is_detected_and_unticked(self):
        template = (REPO_ROOT / ".github" / "PULL_REQUEST_TEMPLATE.md").read_text()
        passed, report = check.evaluate(AREAS, ["access-control/src/lib.rs"], template)
        self.assertFalse(passed)
        self.assertIn("[ ] Authorization", report)


class MappingTests(unittest.TestCase):
    def test_repository_mapping_is_in_sync(self):
        areas = check.parse_sensitive_paths(check.SENSITIVE_PATHS.read_text())
        problems = check.check_mapping(
            areas, check.CODEOWNERS.read_text(), check.LABELER.read_text()
        )
        self.assertEqual(problems, [])

    def test_detects_missing_owner_and_label_glob(self):
        codeowners = "/access-control/ @owner\n"
        labeler = '"area: auth":\n  - changed-files:\n      - any-glob-to-any-file:\n          - access-control/**\n'
        problems = check.check_mapping(AREAS, codeowners, labeler)
        self.assertIn("CODEOWNERS does not own `/creator-keys/src/acl_*.rs` (auth)", problems)
        self.assertIn("labeler.yml `area: auth` is missing `creator-keys/src/acl_*.rs`", problems)
        self.assertIn("labeler.yml has no `area: fees` label", problems)


if __name__ == "__main__":
    unittest.main()
