# Personal assistant replay pack

This directory contains the provider-neutral replay contract for the 100 task assessment.

The pack uses the current assessment table as its single task list. The replay script checks that the table still contains exactly 100 unique rows. Each run receives a stable case ID with the `PA-REPLAY-20260906` marker.

The mock gateway mirrors common read and write shapes for Gmail, Calendar, Notion, files, and transaction portals. It returns synthetic records for one case run at a time. Every record carries the marker and case ID. Writes return a synthetic receipt and never contact a real service.

Use the existing development home by setting `NOEMA_HOME` when needed. The script detects `/var/lib/noema-dev`, which is the configured `noema-dev` home, before falling back to a user home.

## Validation

```sh
bun run scripts/acceptance/personal-assistant-replay.ts --limit 2
NOEMA_PERSONAL_ASSISTANT_MOCK_PORT=3742 bun run scripts/acceptance/run-mock-personal-assistant-services.ts
```

The first command is read-only. Add `--execute` only after the mock gateway and the normal live runner are available. It then sends each synthetic prompt through `scripts/run-live-noema-case.ts` and records the runner output as evidence.

The pack does not claim a case passed when it was not run. Results use `not_run`, `completed`, `review_required`, or `blocked` explicitly.
