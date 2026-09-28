# README screenshots

The September 28, 2026 refresh uses the running Noema app after commit `a181cdea`.
All displayed messages, tool calls, Task documents, and settings are real.
No browser response substitutions or invented results were used.

## Fresh session

The user requested another reset with accounts and settings preserved.
The server was stopped, and a complete backup was saved in a protected directory.
Chat, Tasks, memory, and related content were cleared.
Retained database tables and protected account files matched their values before the reset.
Database integrity and foreign-key checks passed.
The development launcher restarted the existing instance.

Short messages established a Portland weekend, a detailed itinerary, and a compact camera shortlist.
A follow-up asked about streetcar fares while the Tasks ran.
The CLI sent each message through the normal product API.
No bookings, purchases, external messages, or account changes were requested.
The resulting session remains in the development instance.

- Conversation: `conversation:a8973e4c69cf4f7a9147897861dacff4`.
- Itinerary: `task:280780b7acec2ed862e24303c7076784`.
- Camera shortlist: `task:a17ef50c91decda2bba8f8c68acebef6`.

Task browser requests received the restored human evidence.
Their reviews found substantive authorization and low risk, then allowed execution without another prompt.
The itinerary later encountered a separate browser-provider failure while opening Powell's public website.
Its recorded action outcome was uncertain, so Noema paused the Task.
The normal recovery action directed it to continue with web search and label unverified information.
No saved action or result was rewritten.
The itinerary then completed execution and passed review at 04:55 UTC.
The desktop and phone Task images show its actual result.
Both Tasks had completed when the compact-bar images were captured.
Chat is scrolled to their earlier delegation exchange.

The memory update action failed twice because generated pages cited ineligible sources.
The prior sample memory image has been removed from the README.
This refresh does not claim that the new preferences reached durable memory.
The subsequent [memory repair](../validation/memory-source-label-2026-09-28.md) resolved this failure and verified saved preferences in the instance.

## Capture

Playwright used the standard authenticated, read-only browser inspection helper.
The helper continued to reject mutations.
Only the local CLI performed the authorized Chat, recovery, and memory-update actions.
No public authentication setting changed.
The temporary browser context supplied a UUID function for the HTTP inspection origin.
No application code changed.

Chat is scrolled to the delegation exchange, with the itinerary transcript beside it.
The transcript shows an earlier successful TriMet browser call.
Later failures and recovery remain in the same transcript outside that frame.
Messages outside the viewport remain available in the conversation.
The connection image captures the policy and tool sections only.
The account address is outside that frame; no displayed text was replaced.

| Image | Route | Viewport |
| --- | --- | --- |
| `chat.png` | `/`, with the itinerary transcript open | 1600 × 1000 |
| `task.png` | `/tasks/<selected-task>` | 1440 × 960 |
| `connections.png` | `/settings/tools/apis`, cropped to policy and tools | 1440 × 1200 |
| `models.png` | `/settings/agents` | 1440 × 960 |
| `web.png` | `/settings/tools/web` | 1440 × 960 |
| `task-phone.png` | `/tasks/<selected-task>` | 390 × 844 |

The phone image shows the responsive web app, not the native iOS client.
Native desktop, native iOS, and artifact previews were not captured.

## Review

Follow the [browser inspection guide](../frontend/browser-inspection.md) for future captures.
Preserve actual content and review every image before publication.
Keep credentials, raw response dumps, and private account details outside committed artifacts.
Documentation and image changes need link, image, and Git whitespace checks rather than application builds.

All six replacement images received visual review.
The desktop and phone Task captures have no horizontal page overflow.
README links, image paths, and Git whitespace checks passed.
No application builds or test suites were required for this documentation-only change.

## Compact Task bar refresh

Chat, desktop Task, and phone Task images were captured again after reducing the shared bar height.
The bar measures 50 pixels, down from 58 pixels, at all three captured widths.
Its buttons and icons keep their existing sizes.
The new-task screen also received desktop and phone visual review.
These captures used existing data and made no product changes.

The compact-bar change passed frontend type checking and lint.
The main Vite bundle built successfully; its parent command stopped before the GraphiQL build.
The remaining `bun run build:graphiql` step passed when run separately.
`bun run check:generated` passed after committing the required model-settings input correction.
That correction preserves the existing enabled state when saving a model choice.
Both Tasks were complete, so this refresh did not capture a live running or approval state.
