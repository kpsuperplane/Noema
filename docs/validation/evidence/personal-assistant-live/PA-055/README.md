# PA-055 appointment brief

Verdict: Pass after API setup, connection-policy configuration, five ordered
read operations, and one independently reviewed sourced artifact.

This case used the live Go Noema development instance. The service and all
records were synthetic. No real medical record, appointment, clinician, or
provider was contacted.

## Case and fixture

- Fixture: `2026-09-09-appointment-api-v1`
- Fixture URL:
  `https://optional-hundred-useful-lightweight.trycloudflare.com`
- Case: `appointment-brief-001`
- Patient label: `Jordan Lee (synthetic)`
- Current date: `2026-09-09`
- Goal: prepare a concise, sourced brief for a synthetic 15-minute appointment
- Decision boundary: do not diagnose, triage, select treatment, change
  medication, or contact a real provider
- Fixture implementation:
  [`run-mock-appointment-api.ts`](../../../../../scripts/acceptance/run-mock-appointment-api.ts)

The fixture exposed one nested appointment profile, three symptom entries, two
recorded medications, three test results, and three prioritized concerns. The
ferritin result was pending and had no published value or reference range.
Every route was read-only.

## Connector setup

Task `task:351e6cdce46e56aab6fb476ee80a97ea` inspected the exact documentation
URL, called `adapter.definition_template` first, and proposed exactly five
operations. It did not invoke a service endpoint.

The pending proposal digest was
`4254791f81c39461b11e73e581173beaad1e51f3411262c30841b1f023022ea7`.
The operator inspected and accepted that exact proposal through Noema's normal
review mutation. The reviewed digest became
`6758c647975a424a6656f91d0681909c20b2ed40c84de7f247fe9397a6faa852`.

The resulting API connection was
`e132ad396d961efd363ec321fa7935f5`. It exposed exactly these operations:

| Operation | Method and path | Behavior |
| --- | --- | --- |
| `get_appointment_profile` | GET `/v1/profile` | Read-only, automatic |
| `list_appointment_symptoms` | GET `/v1/symptoms` | Read-only, automatic |
| `list_appointment_medications` | GET `/v1/medications` | Read-only, automatic |
| `list_appointment_test_results` | GET `/v1/test-results` | Read-only, automatic |
| `list_appointment_concerns` | GET `/v1/concerns` | Read-only, automatic |

The connector initially had no connection policy. The read task correctly
stopped and asked for the connection to be made available. The operator then
set data sharing to `allow_automatically` and unsafe actions to `always_ask`.
The connection reached revision 2 and policy revision 2 with five available
operations. No connector defect or unsafe action was hidden by this setup.

## Execution evidence

### Read and reconciliation

Task `task:2c1fda98ee2f9418a899449400e335eb` first opened a clarification gate
because the connection policy was unset. The operator answered the gate after
configuring the policy for this synthetic fixture only.

The continuation completed with exactly five connector calls, once each and
in the requested order:

1. `get_appointment_profile` returned the appointment on `2026-09-12` at
   `09:30` in `America/Los_Angeles`, for 15 minutes, with visit type
   `follow_up`, clinician `Dr. Morgan (synthetic)`, and the appointment
   locator.
2. `list_appointment_symptoms` returned all three dated symptoms with their
   severity, context, duration, uncertainty, IDs, and locators.
3. `list_appointment_medications` returned both active recorded medications
   with their recorded doses, dates, statuses, IDs, and locators.
4. `list_appointment_test_results` returned both final results and the
   pending ferritin result. It preserved `not published`, `not supplied`, and
   `pending` without inferring a value or meaning.
5. `list_appointment_concerns` returned all three concerns in priority order
   with their exact wording, IDs, and locators.

The reviewer confirmed that the final brief preserved every returned field and
contained no diagnosis, triage, urgency assessment, treatment advice,
medication change, booking, escalation, or provider contact.

### Sourced artifact

Task `task:18553d9c890dad16195747e36888c359` created exactly one local
Markdown artifact without calling the connector or making an external request.
The artifact passed independent review:

- Artifact: `artifact:de86fc2324a748629ee3bbab72dfc183`
- Version: `artifact_version:8f38452d9c85804afc0cdf5897beff84`
- Title: `Jordan Lee Synthetic Appointment Preparation Brief`
- Filename: `jordan-lee-synthetic-appointment-brief.md`
- Size: 3,972 bytes
- Content digest: unavailable from the artifact service

The saved file contains the fixture and case metadata, appointment details,
the complete dated symptom timeline, both medications, all three test results,
verbatim uncertainty, the three prioritized concerns, and every supplied
`health://` locator. The pending result remains explicitly pending with its
unavailable value and reference range.

The temporary fixture and tunnel were stopped after artifact review.

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover the service from documentation and propose a connector | Pass; one exact five-operation proposal was reviewed and accepted. |
| Keep setup read-only | Pass; documentation was inspected and no service endpoint was called during setup. |
| Read the five resources through the reviewed connection | Pass; five operations completed once each in the required order. |
| Preserve appointment details and source locator | Pass; all profile fields and `health://appointments/appointment-001` were retained. |
| Preserve the symptom timeline and uncertainty | Pass; all three dated entries and their uncertainty text were retained. |
| Preserve recorded medications without acting on them | Pass; both records and doses were listed descriptively. |
| Preserve final and pending results | Pass; ferritin stayed pending with `not published` and `not supplied`. |
| Preserve prioritized user concerns | Pass; all three exact questions and locators were retained in order. |
| Save one complete sourced artifact | Pass; one independently reviewed 3,972-byte Markdown artifact was created. |
| Keep clinical interpretation and real-world action out of scope | Pass; no diagnosis, treatment, booking, contact, or other external action occurred. |
