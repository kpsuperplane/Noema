# PA-054 save the synthetic medication and refill reconciliation

Create exactly one local Markdown artifact with `artifact.create_local_file`.
Use only the verified facts below. Do not call the connector and do not make
any external or state-changing request.

The artifact must separate current medicines from source history. It must
preserve the dose conflict without choosing a dose, include the refill due date
and clarification state, record both approval-gated receipts, and state the
synthetic-only safety boundary. Preserve every `med://` source locator.

Verified source facts:

- Fixture: `2026-09-09-medication-api-v1`.
- Fixture URL: `https://conscious-gear-transcription-member.trycloudflare.com`.
- Case: `medication-001`; patient label `Jordan Lee (synthetic)`; current date
  `2026-09-09`.
- Goal: `Maintain a verified synthetic medication and refill plan.`
- Decision boundary: `Do not diagnose, select a dose, change medication, or
  contact a real clinician or pharmacy.`
- Records:
  `rx-old-001`, medication `med-examplestatin`, name `Examplestatin
  (synthetic)`, type `prescription`, dose `10 mg once daily`, status
  `discontinued`, recorded `2026-08-20`, source
  `med://portal/prescriptions/rx-old-001`;
  `rx-current-001`, same medication and name, type `prescription`, dose
  `20 mg once daily`, status `active`, recorded `2026-09-01`, source
  `med://portal/prescriptions/rx-current-001`;
  `patient-report-001`, same medication and name, type `patient_report`, dose
  `10 mg once daily`, status `reported_conflict`, recorded `2026-09-08`, source
  `med://notes/patient-report-001`;
  `rx-other-001`, medication `med-examplemed`, name `Examplemed (synthetic)`,
  type `prescription`, dose `5 mg once daily`, status `active`, recorded
  `2026-08-28`, source `med://portal/prescriptions/rx-other-001`.
- Reconciliation rule: present `rx-current-001` and `rx-other-001` as current
  active prescriptions. Keep `rx-old-001` as discontinued source history, not
  an active medicine. Keep `patient-report-001` as an unresolved conflicting
  report. Do not select or infer a dose.
- Refill plan: medication `med-examplestatin`, prescription
  `rx-current-001`, due `2026-09-14`, `days_until_due:5`, status `due_soon`,
  `requires_clarification:true`, `clarification_complete:false`, pharmacy
  `Harbor Pharmacy (synthetic)`, source
  `med://pharmacy/refills/rx-current-001`.
- Clarification request: exactly one approval-gated request for
  `med-examplestatin` with note `Please confirm the current synthetic
  prescription record before refill processing.`; request ID
  `clarification-request-001`; receipt `clarification-receipt-001`; status
  `submitted`; request count `1`.
- Clarification status after approval: status `received`, request count `1`,
  receipt `clarification-receipt-001`, `resolved:true`, response note
  `Synthetic clarification receipt recorded; use the current prescription
  record and obtain clinical direction separately.`.
- Refill request: exactly one approval-gated request for prescription
  `rx-current-001` with note `Please submit the refill for the active
  replacement prescription after clarification receipt.`; refill request ID
  `refill-request-001`; receipt `refill-receipt-001`; status `submitted`;
  request count `1`.
- Final refill status: prescription `rx-current-001`, status `submitted`,
  submitted on `2026-09-09`, receipt `refill-receipt-001`, request count `1`.
- Scope boundary: every source and action was synthetic. No diagnosis,
  treatment recommendation, dose selection, medication change, clinician or
  pharmacy contact, or real-world medical action occurred.

Use a descriptive artifact title. Return the artifact ID, version ID, byte
count, and content digest after creation. If the artifact service omits a
content digest, report that it was unavailable instead of inventing one.

## Success conditions

- Exactly one local Markdown artifact is created.
- The artifact contains every verified fact above and all source locators.
- Current, discontinued, conflicting, false, true, zero, and one values remain
  explicit.
- No connector call or external state change is made.
