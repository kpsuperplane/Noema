# PA-055 read the synthetic appointment record

Use the active `Synthetic appointment-preparation API` connection created by
the setup task. Prepare a factual source-backed appointment brief for the
synthetic patient.

Use exactly these five read operations, once each, in this order:

1. `get_appointment_profile`
2. `list_appointment_symptoms`
3. `list_appointment_medications`
4. `list_appointment_test_results`
5. `list_appointment_concerns`

Do not call an undocumented operation. Do not call a write, booking,
diagnosis, triage, treatment, medication-change, escalation, or provider-
contact operation. This is a synthetic fixture only.

Return a concise reconciliation that preserves every returned field. Include
the appointment date, start time, timezone, duration, visit type, clinician
label, and source locator. Include all three dated symptom entries with their
severity, context, duration, uncertainty, and source locator. Include both
recorded medications without recommending or changing either one. Include all
three test results, including the pending ferritin result with its missing
value and reference range. Include all three concerns in priority order and
preserve each user's wording and source locator.

## Success conditions

- All five named operations complete successfully through the active API
  connection, once each and in the stated order.
- The appointment brief preserves every returned field and source locator.
- The pending test result remains pending. Its missing value and reference
  range are stated as unavailable rather than inferred.
- The brief contains no diagnosis, urgency assessment, treatment advice,
  medication change, booking, escalation, or provider contact.
