# Gmail and Calendar library validation

Date: 2026-09-11. Base: `c71b914a` on `main`.

## Delivered behavior

Gmail and Google Calendar use bundled, reviewed definitions.
Settings and the agent select the same library entries.
Selection checks the bundled digest and preserves installed revisions.
It does not approve account access or connection permissions.

`Connect API` uses the same service choice component as `Add provider`.
The picker opens a Connect step, then continues through existing account setup.
Missing Google application setup opens protected document import.
Successful import resumes the selected connection.
Additional accounts remain available after the first connection.
Partial consent proceeds to connection permissions without repeated consent requests.

The library has 21 Gmail operations and 10 Calendar operations.
Definitions use the existing compiler, Google profile, storage, and execution path.
No migration, registry service, updater, or new execution system was added.

## Server checks

Eight focused tests cover these separate risks:

1. Bundled compilation and broad scope contracts.
2. Gmail requests, pagination, mail content, mutation receipts, and content limits.
3. Calendar time zones, recurrence, attendees, notifications, and omitted patch fields.
4. Partial grants and disabled tools.
5. Repeated selection, installed revisions, and connection permissions.
6. Changed digests, unknown entries, and injected definitions.
7. Account boundaries, shared application setup, and credential disclosure.
8. Agent selection, Chat and Task authentication requests, and duplicate prevention.

Table-driven cases share the relevant assertions.
Existing setup catalog and prompt expectations include the new selection tool.
No frontend tests were added.

Passed against the final server changes:

- `CGO_ENABLED=0 go test ./internal/graphql -run 'TestLibrary|TestOAuth|TestRustAPI_schema_sdl_exposes_initial_noema_fields'`
- Focused library and prompt checks in `internal/adapter` and `internal/runtime`.
- `CGO_ENABLED=0 go test ./cmd/... ./internal/...`
- `CGO_ENABLED=0 go vet ./cmd/... ./internal/...`

Earlier focused checks exposed schema formatting and prompt-reference differences.
Those differences were corrected before the broad run.
An intermittent runtime replay test also failed against unchanged base prompt files.
The completed broad run passed, including that runtime package.

## Frontend checks

`bun run build` passed after the final connection routing change.
`bunx eslint src --max-warnings=0` passed against the final frontend changes.

`bun run lint` reached TypeScript checking and failed in an unchanged file:
`apps/web/src/components/settings/TaskModelPoolsSettings.tsx:100` omits the required `enabled` field.
The same omission exists at the base revision.
This task did not change that file or its input contract.

`bun run check:generated` passed at `5c8fa8a0`.
It ran after commit because its script rejects staged generated files.
Only this evidence text changed after that check; the successful code checks remain valid.

## Rendered inspection

Playwright used the authenticated, read-only development relay.
Settings was inspected at 1,440 × 900 and 390 × 900.
The API and provider pickers use the same item layout at both widths.
Gmail and Calendar selection, Back, Escape cancellation, and page width checks passed.

Synthetic GraphQL replies exercised protected application import, account choice, and a recoverable selection error.
Those replies stayed inside the inspection browser. No setup mutation reached the development server.
Account choice retained one selection, and focus remained inside its dialog.
The protected import dialog kept technical information behind disclosure.
Chat was inspected at both widths after its transcript loaded.
The transcript and composer had no horizontal overflow.
The inspection origin required a browser-only UUID shim for synthetic mutations.
Production code and public authentication were unchanged by that shim.

Local screenshots and logs are under `/var/tmp/noema-library-*`.

## Limits

Real Google consent, provider cancellation, and a real partial grant were not performed.
The development account had no Gmail or Calendar grant for those rendered states.
Server tests cover partial grants and account reuse; synthetic replies cover setup presentation.
No real messages, drafts, or calendar events were changed during validation.

## Size and sources

Before this evidence file, the patch contained:

| Category | Added lines | Removed lines |
| --- | ---: | ---: |
| Production Go | 244 | 26 |
| Production frontend and GraphQL schema | 136 | 52 |
| Tests | 436 | 4 |
| Definition data | 414 | 0 |
| Generated GraphQL | 589 | 2 |
| Contracts | 44 | 1 |

The inclusive patch had 1,977 added lines and 85 removed lines before this final evidence update.
All code budgets remain below their stop conditions.
There are no database migration lines.

Definitions were checked against official Google discovery documents and references:

- [Gmail REST reference](https://developers.google.com/workspace/gmail/api/reference/rest)
- [Calendar REST reference](https://developers.google.com/workspace/calendar/api/v3/reference)
- [Gmail scopes](https://developers.google.com/workspace/gmail/api/auth/scopes)
- [Calendar scopes](https://developers.google.com/workspace/calendar/api/auth)
- [Google granular permissions](https://developers.google.com/identity/protocols/oauth2/resources/granular-permissions)

Broad scopes do not divide into smaller permissions during consent.
Narrower grants enable only operations whose declared scope contracts are satisfied.

## Setup correction

Follow-up base: `c583f570`.

Selecting an API now opens setup directly. The redundant Connect step was removed.
Unconnected reviewed APIs show Resume setup under Finish setup.
That action remains available when application import is incomplete or already complete.
Cancellation preserves the installed definition and allows setup to resume.

The correction changes only frontend code and contracts.
The two frontend files have 24 added lines and 38 removed lines.
No server, generated GraphQL, or test code changed.

Validation against these frontend changes:

- `bun run check:generated` passed.
- `bun run build` passed.
- `bunx eslint src --max-warnings=0` passed.
- `bun run lint` still fails on the unchanged Task model settings error documented above.

Browser inspection used synthetic setup replies at desktop and phone widths.
It checked direct selection, error recovery, cancellation, and Resume setup.
It also checked resuming after application import and selecting an existing account.
Focus remained inside the account dialog. Neither width had horizontal overflow.
No inspection mutation reached the development server.
Screenshots and logs use `/var/tmp/noema-resume-*` and `/var/tmp/noema-library-resume-*`.

## Inline loading and shorter action

Follow-up base: `d611f4a9`.
The selected API replaces its arrow with an accessible spinner while setup opens.
The spinner uses the existing arrow space. The separate loading paragraph was removed.
The unfinished API action now reads Setup and includes a settings icon.

This frontend change adds 18 code lines and removes 12. No server or test code changed.
`bun run build`, `bun run check:generated`, and `bunx eslint src --max-warnings=0` passed.
`bun run lint` still reports the unchanged Task model settings error documented above.
Browser inspection passed at 1,440 × 900 and 390 × 900 with synthetic setup replies.
The dialog bounds were identical before and during loading at both widths.
Only the selected API showed the setup spinner. Error recovery and Setup still worked.
No inspection mutation reached the server. Screenshots use `/var/tmp/noema-inline-*`.
