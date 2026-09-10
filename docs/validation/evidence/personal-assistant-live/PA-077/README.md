# PA-077 direct Go recheck

Updated: 2026-09-10

The agent rejected the inaccessible option, selected the accessible
replacement, reconciled the hotel, and suppressed the unchanged alert.

- Chat turns: request `847`, approval `848`, status `850`.
- Final status: `verified_synthetic_disruption`.
- Receipts: `replacement-001`, `hotel-reconcile-001`.
- Fixture action: `pa-077-action-001`.

The approved action was followed by a status read with both receipts. No real
carrier, hotel, or traveller record changed. See the [v4 ledger](../direct-recheck-ledger-v4.json).
