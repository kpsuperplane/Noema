# PA-061 save the synthetic discharge-transition handoff

Use the completed result from `PA-061 coordinate the synthetic discharge
transition`. Create exactly one local Markdown artifact. Do not call the
synthetic connector again and do not contact any real service.

The artifact must be a concise handoff packet. Preserve the returned case
profile, both discharge orders, both pre-discharge medication records, the
Examplemed conflict, the exact two warning instructions, both equipment
records, transport, appointment, caregiver, coordination receipt, later status,
request count, and every `transition://` source locator. State that no dose was
selected and that the care team must reconcile the conflict before the next
dose.

Include a clear synthetic-only boundary. Include the approval decision and
the factual coordination note. Do not add diagnosis, treatment, medical
advice, legal advice, real contact, real transport, payment, or any invented
fact. Do not include credentials or other secrets.

## Success conditions

- Exactly one local Markdown artifact is created.
- The artifact preserves all required returned facts, warning text, receipts,
  statuses, counts, dates, IDs, and `transition://` locators.
- The artifact states the unresolved medication conflict without selecting a
  dose and states the synthetic-only boundary.
- No connector operation or real-world service is invoked.
