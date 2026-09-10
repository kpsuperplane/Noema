# PA-079 direct Go recheck

Updated: 2026-09-10

The agent kept the restricted 150 USD credit separate from the pending 80 USD
refund, prevented a double claim, and recorded one eligible recovery request.

- Chat turns: request `855`, approval `856`, status `858`.
- Final status: `verified_synthetic_value_recovery`.
- Receipts: `recovery-request-001`, `refund-reminder-001`.
- Fixture action: `pa-079-action-001`.

The final status read confirmed one action and both receipts. No real money,
merchant, or payment account was touched. See the [v4 ledger](../direct-recheck-ledger-v4.json).
