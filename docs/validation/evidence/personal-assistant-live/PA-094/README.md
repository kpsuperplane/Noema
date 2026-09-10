# PA-094 direct Go recheck

Updated: 2026-09-10

The agent inventoried metadata, flagged the stale device and MFA gap, and
revoked only the unrecognized session.

- Chat turns: request `918`, approval `919`, status `921`.
- Final status: `verified_synthetic_security`.
- Receipt: `session-revocation-001`.
- Fixture action: `pa-094-action-001`.

The final read confirmed one action and the session receipt. No real session,
device, or account was changed. See the [v4 ledger](../direct-recheck-ledger-v4.json).
