#!/usr/bin/env python3
"""Validate one-to-one coverage between the audit tracker and ledger."""

from pathlib import Path
import re
import sys
from typing import Dict, List, Tuple


ROOT = Path(__file__).resolve().parents[1]
TRACKER_PATH = ROOT / "docs" / "codebase-audit-tracker.md"
LEDGER_PATH = ROOT / "docs" / "codebase-audit-ledger.md"

ID_PATTERN = re.compile(
    r"^(?:SEC|GOV|DATA|RUN|PERF|UX|A11Y|REL|ARCH|TEST|DOC|MILESTONE)-\d{3}$"
)
CHECKBOX_PATTERN = re.compile(
    r"^\s*(?:[-+*]|\d+[.)])\s+\[([ xX])\]\s+(?:`([^`]+)`)?"
)
PRIMARY_MILESTONES = {"Foundation", "M1+M2", "M3", "M4", "M7", "M6", "M5", "M8"}


def _parse_tracker(text: str) -> Tuple[Dict[str, bool], List[str]]:
    items: Dict[str, bool] = {}
    errors: List[str] = []

    for line_number, line in enumerate(text.splitlines(), 1):
        match = CHECKBOX_PATTERN.match(line)
        if not match:
            continue
        checked, item_id = match.groups()
        if item_id is None:
            errors.append(f"tracker checkbox is missing an ID at line {line_number}")
            continue
        if not ID_PATTERN.fullmatch(item_id):
            errors.append(f"malformed tracker ID {item_id} at line {line_number}")
            continue
        if item_id in items:
            errors.append(f"duplicate tracker ID {item_id} at line {line_number}")
            continue
        items[item_id] = checked.lower() == "x"

    return items, errors


def _parse_ledger(text: str) -> Tuple[Dict[str, List[str]], List[str]]:
    rows: Dict[str, List[str]] = {}
    errors: List[str] = []

    for line_number, line in enumerate(text.splitlines(), 1):
        if not line.startswith("|") or not line.endswith("|"):
            continue
        cells = [cell.strip() for cell in line[1:-1].split("|")]
        if not cells or cells[0] == "ID" or (
            cells[0] and set(cells[0]) <= {"-", ":"}
        ):
            continue
        if len(cells) != 7:
            errors.append(f"malformed ledger row at line {line_number}: expected 7 columns")
            continue
        item_id = cells[0]
        if not ID_PATTERN.fullmatch(item_id):
            display_id = item_id or "<empty>"
            errors.append(f"malformed ledger ID {display_id} at line {line_number}")
            continue
        if item_id in rows:
            errors.append(f"duplicate ledger row {item_id} at line {line_number}")
            continue
        rows[item_id] = cells

    return rows, errors


def validate_coverage(tracker_text: str, ledger_text: str) -> List[str]:
    tracker, errors = _parse_tracker(tracker_text)
    ledger, ledger_errors = _parse_ledger(ledger_text)
    errors.extend(ledger_errors)

    for item_id, cells in ledger.items():
        if item_id not in tracker:
            errors.append(f"ledger ID {item_id} is not in tracker")

        primary_milestone = cells[1]
        if not primary_milestone:
            errors.append(f"{item_id} has empty primary milestone")
        elif primary_milestone not in PRIMARY_MILESTONES:
            errors.append(f"{item_id} has invalid primary milestone {primary_milestone}")

    for item_id, checked in tracker.items():
        cells = ledger.get(item_id)
        if cells is None:
            errors.append(f"tracker ID {item_id} is missing from ledger")
            continue
        if checked and not cells[5]:
            errors.append(f"checked tracker ID {item_id} has blank acceptance evidence")
        elif checked and cells[5] == "Pending":
            errors.append(f"checked tracker ID {item_id} has Pending acceptance evidence")
        if checked and not cells[6]:
            errors.append(f"checked tracker ID {item_id} has blank completion commit")
        elif checked and cells[6] == "Pending":
            errors.append(f"checked tracker ID {item_id} has Pending completion commit")

    return errors


def main() -> int:
    errors = validate_coverage(
        TRACKER_PATH.read_text(encoding="utf-8"),
        LEDGER_PATH.read_text(encoding="utf-8"),
    )
    if errors:
        for error in errors:
            print(f"audit coverage error: {error}", file=sys.stderr)
        return 1
    print("audit coverage check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
