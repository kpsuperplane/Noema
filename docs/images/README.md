# README screenshots

The screenshots use the running Noema web app and its current styles.
The opening Chat image was refreshed on 2026-09-28. The other images were captured on 2026-09-27.

## Opening Chat image

The user requested a fresh instance with accounts and settings preserved.
The server was stopped, and a complete protected backup was saved before the reset.
Chat, Tasks, memory, and their related content were cleared.
Retained database settings and protected account files were checked for exact preservation before restart.
Database integrity and foreign-key checks passed.

The new conversation uses short messages about a Portland weekend and compact travel cameras.
Each message adds one question or preference.
Noema performed real public web research and created real background Tasks.
Messages, tool results, Task documents, statuses, and links were not substituted.
The Chat is scrolled to the delegation exchange, with the routes Task transcript open beside it.
Earlier and later messages remain in the conversation outside the captured viewport.
Capture used the standard read-only browser inspection helper.
The CLI sent the conversation messages through the normal product API.
No external messages or bookings were requested.
The fresh conversation and its Tasks remain in the development instance.

The image records the state at capture. It does not claim that every research Task had finished.
The routes Task had a pending public-web approval. The camera Task was also active.
A later attempt to add a camera budget failed because the Task update action was unavailable.
That failed attempt remains in Chat, below the captured exchange; the image does not imply it succeeded.

Recorded Tasks:

- `task:1b07663351150183d9358d4c263d63d7`: Plan Portland car-free routes and costs.
- `task:f252dfe554d49350b95254e69d8799c5`: Research compact travel cameras.

## Other screenshots: sample content

Capture used Playwright through the authenticated, read-only inspection socket.
The inspection helper continued to reject mutations.
The September 27 capture did not change messages, Tasks, account settings, or memory pages on the server.

A temporary capture helper substituted sample GraphQL responses in the browser.
It applied the same substitution to live subscription messages.
Task documents, memory prose, the assistant name, and the displayed account address use sample values.
The original sample Chat image has been replaced by the real conversation described above.
The Task list contains one example. Task files and citations are omitted from that example.
The Task status comes from an existing completed Task, but the travel result is illustrative.
Model names, controls, connection policy choices, and browser settings retain the rendered configuration.
The connection screen shows an unknown health state; it does not prove service availability.

The HTTP inspection origin lacks some secure-context browser APIs.
The temporary capture context supplied a UUID function for that origin.
No application code or public authentication setting changed.

## Images

| Image | Route | Viewport |
| --- | --- | --- |
| `chat.png` | `/`, with the routes Task transcript open | 1600 × 1000 |
| `task.png` | `/tasks/<selected-task>` | 1440 × 960 |
| `memory.png` | `/memory` | 1440 × 960 |
| `connections.png` | `/settings/tools/apis` | 1440 × 960 |
| `models.png` | `/settings/agents` | 1440 × 960 |
| `web.png` | `/settings/tools/web` | 1440 × 960 |
| `task-phone.png` | `/tasks/<selected-task>` | 390 × 844 |

The phone image shows the responsive web app, not the native iOS client.
Native desktop, native iOS, and artifact previews were not captured.
The opening image includes a real Task approval prompt.

## Refresh and review

Follow the [browser inspection guide](../frontend/browser-inspection.md).
Use a fresh browser context with service workers blocked.
For the opening image, use the real conversation and preserve its content.
For sample images, prepare sample content before saving images for the repository.
Apply sample substitutions to both query responses and subscriptions.
Keep the read-only request checks in place.
Wait for each route to display its content before capture.

Review every image for private content, errors, loading states, and clipped controls.
Check the phone layout separately.
Keep the sample-content notice beside the README images.
Do not commit live response dumps, credentials, or private screenshots.

The six retained sample images received visual review on September 27.
The refreshed opening image received a separate visual review on September 28.
It now shows the updated browser permission prompt with the website, URL, and stated reason.
Relative documentation links and image paths were checked.
The original documentation changes required no application builds or test suites.
The permission prompt update received separate frontend checks and desktop and phone review.
