# README screenshots

Captured on 2026-09-27 from the running Noema development web app.
The screenshots use the real React interface and its current styles.
They are illustrative product images, not evidence of completed external actions.

## Content and access

Capture used Playwright through the authenticated, read-only inspection socket.
The inspection helper continued to reject mutations.
No messages, Tasks, account settings, or memory pages were changed on the server.

A temporary capture helper substituted sample GraphQL responses in the browser.
It applied the same substitution to live subscription messages.
Chat text, Task documents, memory prose, the assistant name, and the displayed account address use sample values.
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
| `chat.png` | `/` | 1440 × 960 |
| `task.png` | `/tasks/<selected-task>` | 1440 × 960 |
| `memory.png` | `/memory` | 1440 × 960 |
| `connections.png` | `/settings/tools/apis` | 1440 × 960 |
| `models.png` | `/settings/agents` | 1440 × 960 |
| `web.png` | `/settings/tools/web` | 1440 × 960 |
| `task-phone.png` | `/tasks/<selected-task>` | 390 × 844 |

The phone image shows the responsive web app, not the native iOS client.
Native desktop, native iOS, approval prompts, and artifact previews were not captured.

## Refresh and review

Follow the [browser inspection guide](../frontend/browser-inspection.md).
Use a fresh browser context with service workers blocked.
Prepare sample content before saving images for the repository.
Apply sample substitutions to both query responses and subscriptions.
Keep the read-only request checks in place.
Wait for each route to display its content before capture.

Review every image for private content, errors, loading states, and clipped controls.
Check the phone layout separately.
Keep the sample-content notice beside the README images.
Do not commit live response dumps, credentials, or private screenshots.

For this update, all seven final images received visual review.
Relative documentation links and image paths were checked.
Git whitespace checks passed. No application code changed in this unit.
Application builds and test suites were not required for these documentation changes.
