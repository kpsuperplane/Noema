# PA-054 read and reconcile the synthetic medication list after repair

Use the active `Synthetic medication and refill API` connection after its
reviewed `get_refill_plan` transform was repaired. Read the baseline case
profile, medication records, and refill plan.

Use exactly these three read operations, once each, in this order:

1. `get_medication_profile`
2. `list_medication_records`
3. `get_refill_plan`

Do not call either request operation or any status operation. Do not call an
undocumented operation. Do not diagnose, recommend treatment, select a dose,
change medication, contact a clinician or pharmacy, or infer a condition. This
is a synthetic fixture only.

Return a concise reconciliation that preserves every returned field. The
active medication list must exclude the discontinued `rx-old-001` prescription
from current medicines while retaining it as source history. Preserve the
active replacement `rx-current-001`, the conflicting patient report with its
reported dose, and the unrelated active prescription. Report the refill due on
`2026-09-14`, five days from the fixture date, and preserve both clarification
flags, including `clarification_complete:false`.

## Success conditions

- All three named operations complete successfully through the active API
  connection.
- Four medication records are reported with their statuses and source
  locators.
- The discontinued record is not presented as an active medication.
- The dose conflict is explicit and is not resolved by choosing a dose.
- The refill plan names the active replacement, due date, five-day interval,
  and both clarification flags.
- No state-changing operation or clinical advice appears.
