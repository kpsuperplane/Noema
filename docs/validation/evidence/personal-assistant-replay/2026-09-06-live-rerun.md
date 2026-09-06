# Live 100-case replay — 2026-09-06

The replay ran all 100 personal-assistant cases through the real Go `noema-dev` backend.

| Field | Result |
| --- | --- |
| Cases selected | 100 |
| Cases completed | 100 |
| Cases requiring human intervention | 0 |
| Cases blocked | 0 |
| Backend | Go `noema-dev` |
| GraphQL socket | `/var/lib/noema-dev/run/graphql.sock` |
| Noema home | `/var/lib/noema-dev` |
| Fixture mode | Local synthetic gateway, case marker `PA-REPLAY-20260906` |
| External writes | None requested by the prompts |

Command used:

```sh
bun run scripts/acceptance/personal-assistant-replay.ts \
  --execute \
  --socket /var/lib/noema-dev/run/graphql.sock \
  --limit 100
```

The raw result was written to `/tmp/personal-assistant-replay-20260906-rerun.json` during the run. Each result had terminal state `completed`.
