# PA-087 direct Go recheck

Updated: 2026-09-10

Verdict: **Pass — synthetic bounded verification**

The agent selected Camp B at 320 USD after the sibling discount and kept Camp
C waitlisted.

- Chat request turn: `888`.
- Final status turn: `892` after a corrected approval follow-up.
- Final status: `verified_synthetic_activity`.
- Receipts: `camp-b-001`, `waitlist-001`.
- Fixture action: `pa-087-action-001`.

The first approval text changed 40 USD to 0 because of shell quoting. The
agent caught the mismatch and made no action. The corrected follow-up produced
the single approved action. No real camp or payment was used. See the [v4 ledger](../direct-recheck-ledger-v4.json).
