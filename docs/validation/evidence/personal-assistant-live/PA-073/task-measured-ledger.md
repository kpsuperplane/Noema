# PA-073 measured service ledger

The fixture ledger contained exactly nine reviewed service calls. Browser
`/docs`, `/`, and `/favicon.ico` requests and `/health` probes are outside this
service-call count.

| # | Operation | Method and path | Result |
| ---: | --- | --- | --- |
| 1 | `get_household_profile` | GET `/v1/profile` | `household://profiles/household-service-001` |
| 2 | `list_cleaners` | GET `/v1/providers` | Three bounded providers |
| 3 | `get_household_constraints` | GET `/v1/constraints` | `household://constraints/household-constraints-001` |
| 4 | `list_cleaning_availability` | GET `/v1/availability` | Three bounded slots |
| 5 | `get_cancellation_policy` | GET `/v1/cancellation-policy` | One approved backup |
| 6 | `create_recurring_schedule` | POST `/v1/schedules` | `cleaning-schedule-001`, submission 1 |
| 7 | `list_schedule_events` | GET `/v1/schedule-events` | `provider_cancelled`, effective 2026-09-19 |
| 8 | `change_schedule_to_backup` | POST `/v1/schedule-changes` | `schedule-change-001`, submission 1 |
| 9 | `get_schedule_status` | GET `/v1/schedule-status` | Primary canceled, backup scheduled |

No operation was repeated. The two POST requests used `retry: never` and
were each submitted once after a separate approval.
