# PA-065 create the final synthetic asset-inventory brief

Create exactly one local Markdown artifact from the completed PA-065
reconciliation. Do not call the connector. Do not create a shared workspace.
Use only facts already returned by the completed reconciliation task.

Use this exact filename:

`synthetic-asset-inventory-final.md`

The artifact must preserve all five assets and their evidence in a readable
table or equivalent sections. Include, for every asset:

- asset ID, product label, brand, model, serial number, purchase date, receipt
  ID and product `asset://` locator;
- matching warranty ID, provider, start date, expiry date, coverage, serial
  match, expiry assessment, and warranty `asset://` locator;
- matching serial-photo ID, captured date, serial text, match status, and
  serial-photo `asset://` locator;
- the original receipt ID, purchase date, amount, status, duplicate relation
  when present, and receipt `asset://` locator.

Also include:

- the existing `repair-000` record on `asset-001` only, with its date, vendor,
  summary, and `asset://` locator;
- the exact recall `recall-001` on `asset-002`, including model, prefix,
  active status, and locator;
- the near-match `recall-002`, including its different model and prefix, and a
  clear statement that it was not flagged;
- the approved synthetic repair receipt on `asset-002` with all six submitted
  arguments, returned receipt ID, status, submission count, and locator;
- final status values: case ID, asset count, both duplicate IDs, exact recall
  ID and asset ID, near-match ID, repaired asset ID, repair receipt ID,
  submission count, and status locator;
- a clear statement that `warranty-004` expired on 2026-08-22 relative to the
  profile date 2026-09-09;
- a clear statement that the write was a synthetic fixture receipt only. No
  real product was repaired, no warranty claim or recall was filed, no
  manufacturer was contacted, and no money moved;
- every `asset://` locator returned by the source records. Do not invent URLs,
  fields, dates, amounts, IDs, or status values.

Include a short operation ledger stating that the seven reads ran once in the
required order, the repair receipt ran once after explicit approval, and the
final status read ran once after the write. State that no other connector
operation ran.

If a human follow-up is useful, ask whether they want a real manufacturer or
warranty follow-up for the exact recall and expired warranty. Do not contact
anyone or take that action.

## Success conditions

- Exactly one local artifact is created with the exact filename and Markdown
  media type.
- The artifact preserves all five assets, all product/warranty/receipt/photo
  fields, duplicate relation, exact and near recall distinction, existing and
  approved repairs, final status values, dates, amounts, counts, statuses, and
  all returned `asset://` locators.
- The artifact marks `warranty-004` expired and states that `recall-002` was not
  flagged.
- No connector operation or real-world action is performed.
