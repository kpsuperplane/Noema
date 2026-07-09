import unittest

from scripts.check_audit_coverage import validate_coverage


VALID_TRACKER = """\
# Audit tracker

- [ ] `SEC-001` **P0** Authenticate requests.
- [ ] `GOV-001` **P1** Enforce policy.
"""

VALID_LEDGER = """\
# Audit ledger

| ID | Primary milestone | Known consumers | Automated coverage | Browser scenario | Acceptance evidence | Completion commit |
| --- | --- | --- | --- | --- | --- | --- |
| SEC-001 | M1+M2 | Pending | Pending | Pending | Pending | Pending |
| GOV-001 | M1+M2 | Pending | Pending | Pending | Pending | Pending |
"""


class ValidateCoverageTests(unittest.TestCase):
    def assert_has_error(self, tracker: str, ledger: str, expected: str) -> None:
        errors = validate_coverage(tracker, ledger)
        self.assertTrue(
            any(expected in error for error in errors),
            f"expected error containing {expected!r}, got {errors!r}",
        )

    def test_accepts_complete_one_to_one_coverage(self) -> None:
        self.assertEqual(validate_coverage(VALID_TRACKER, VALID_LEDGER), [])

    def test_rejects_checkbox_without_tracker_id(self) -> None:
        tracker = VALID_TRACKER.replace(
            "- [ ] `SEC-001` **P0** Authenticate requests.",
            "- [ ] **P0** Authenticate requests.",
        )
        self.assert_has_error(tracker, VALID_LEDGER, "tracker checkbox is missing an ID")

    def test_accepts_indented_dash_checkbox_with_tracker_id(self) -> None:
        tracker = VALID_TRACKER.replace(
            "- [ ] `SEC-001`", "  - [ ] `SEC-001`"
        )
        self.assertEqual(validate_coverage(tracker, VALID_LEDGER), [])

    def test_rejects_indented_dash_checkbox_without_tracker_id(self) -> None:
        tracker = VALID_TRACKER + "  - [ ] Nested task without an ID.\n"
        self.assert_has_error(tracker, VALID_LEDGER, "tracker checkbox is missing an ID")

    def test_accepts_asterisk_checkbox_with_tracker_id(self) -> None:
        tracker = VALID_TRACKER.replace(
            "- [ ] `SEC-001`", "* [ ] `SEC-001`"
        )
        self.assertEqual(validate_coverage(tracker, VALID_LEDGER), [])

    def test_rejects_asterisk_checkbox_without_tracker_id(self) -> None:
        tracker = VALID_TRACKER + "* [ ] Task without an ID.\n"
        self.assert_has_error(tracker, VALID_LEDGER, "tracker checkbox is missing an ID")

    def test_rejects_duplicate_tracker_id(self) -> None:
        tracker = VALID_TRACKER.replace("`GOV-001`", "`SEC-001`")
        self.assert_has_error(tracker, VALID_LEDGER, "duplicate tracker ID SEC-001")

    def test_rejects_malformed_tracker_id(self) -> None:
        tracker = VALID_TRACKER.replace("`SEC-001`", "`SEC-1`")
        self.assert_has_error(tracker, VALID_LEDGER, "malformed tracker ID SEC-1")

    def test_rejects_empty_ledger_id(self) -> None:
        ledger = VALID_LEDGER + (
            "|  | M1+M2 | Pending | Pending | Pending | Pending | Pending |\n"
        )
        self.assert_has_error(VALID_TRACKER, ledger, "malformed ledger ID <empty>")

    def test_rejects_duplicate_ledger_row(self) -> None:
        ledger = VALID_LEDGER + (
            "| SEC-001 | M1+M2 | Pending | Pending | Pending | Pending | Pending |\n"
        )
        self.assert_has_error(VALID_TRACKER, ledger, "duplicate ledger row SEC-001")

    def test_rejects_ledger_id_unknown_to_tracker(self) -> None:
        ledger = VALID_LEDGER + (
            "| DATA-999 | M1+M2 | Pending | Pending | Pending | Pending | Pending |\n"
        )
        self.assert_has_error(VALID_TRACKER, ledger, "ledger ID DATA-999 is not in tracker")

    def test_rejects_tracker_id_missing_from_ledger(self) -> None:
        ledger = VALID_LEDGER.replace(
            "| GOV-001 | M1+M2 | Pending | Pending | Pending | Pending | Pending |\n",
            "",
        )
        self.assert_has_error(VALID_TRACKER, ledger, "tracker ID GOV-001 is missing from ledger")

    def test_rejects_empty_primary_owner(self) -> None:
        ledger = VALID_LEDGER.replace("| SEC-001 | M1+M2 |", "| SEC-001 |  |")
        self.assert_has_error(VALID_TRACKER, ledger, "SEC-001 has empty primary milestone")

    def test_rejects_multiple_primary_owners(self) -> None:
        ledger = VALID_LEDGER.replace(
            "| SEC-001 | M1+M2 |", "| SEC-001 | M1+M2, M4 |"
        )
        self.assert_has_error(
            VALID_TRACKER,
            ledger,
            "SEC-001 has invalid primary milestone M1+M2, M4",
        )

    def test_rejects_checked_item_with_pending_evidence(self) -> None:
        tracker = VALID_TRACKER.replace("- [ ] `SEC-001`", "- [x] `SEC-001`")
        ledger = VALID_LEDGER.replace(
            "| SEC-001 | M1+M2 | Pending | Pending | Pending | Pending | Pending |",
            "| SEC-001 | M1+M2 | Pending | Pending | Pending | Pending | abc1234 |",
        )
        self.assert_has_error(
            tracker, ledger, "checked tracker ID SEC-001 has Pending acceptance evidence"
        )

    def test_rejects_checked_item_with_blank_acceptance_evidence(self) -> None:
        tracker = VALID_TRACKER.replace("- [ ] `SEC-001`", "- [x] `SEC-001`")
        ledger = VALID_LEDGER.replace(
            "| SEC-001 | M1+M2 | Pending | Pending | Pending | Pending | Pending |",
            "| SEC-001 | M1+M2 | Pending | Pending | Pending |  | abc1234 |",
        )
        self.assert_has_error(
            tracker, ledger, "checked tracker ID SEC-001 has blank acceptance evidence"
        )

    def test_rejects_checked_item_with_pending_completion_commit(self) -> None:
        tracker = VALID_TRACKER.replace("- [ ] `SEC-001`", "- [x] `SEC-001`")
        ledger = VALID_LEDGER.replace(
            "| SEC-001 | M1+M2 | Pending | Pending | Pending | Pending | Pending |",
            "| SEC-001 | M1+M2 | Pending | Pending | Pending | reviewed | Pending |",
        )
        self.assert_has_error(
            tracker, ledger, "checked tracker ID SEC-001 has Pending completion commit"
        )

    def test_rejects_checked_item_with_blank_completion_commit(self) -> None:
        tracker = VALID_TRACKER.replace("- [ ] `SEC-001`", "- [x] `SEC-001`")
        ledger = VALID_LEDGER.replace(
            "| SEC-001 | M1+M2 | Pending | Pending | Pending | Pending | Pending |",
            "| SEC-001 | M1+M2 | Pending | Pending | Pending | reviewed |  |",
        )
        self.assert_has_error(
            tracker, ledger, "checked tracker ID SEC-001 has blank completion commit"
        )


if __name__ == "__main__":
    unittest.main()
