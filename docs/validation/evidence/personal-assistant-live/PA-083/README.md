# PA-083 direct Go recheck

Updated: 2026-09-10

The agent chose listing A within the rent, commute, and pet constraints and
recorded one synthetic application.

- Chat turns: request `871`, approval `872`, corrected status `875`.
- Final status: `verified_synthetic_housing`.
- Receipt: `application-001`.
- Fixture action: `pa-083-action-001`.

The first fixture response incorrectly named listing C. The fixture source and
saved state were corrected. A status-only read then confirmed listing A. No
second action was submitted. No real listing or application was used. See the
[v4 ledger](../direct-recheck-ledger-v4.json).
