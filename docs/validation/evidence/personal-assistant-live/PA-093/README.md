# PA-093 direct Go recheck

Updated: 2026-09-10

The agent retained 25 unique files and both differing same-name files,
preserved retention data, and kept duplicate removal as a separate approval.

- Initial provider error: turn `913`, before the case was read.
- Retry and approval: turns `914` and `915`; status turn `917`.
- Final status: `verified_synthetic_archive`.
- Receipts: `archive-001`, `backup-001`.
- Fixture action: `pa-093-action-001`.

The failed request created no fixture action. The retry created one action and
the final read confirmed both receipts. No real files or storage account was
changed. See the [v4 ledger](../direct-recheck-ledger-v4.json).
