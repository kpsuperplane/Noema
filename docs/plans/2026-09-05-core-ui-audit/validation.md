# Validation

## UI audit and gallery

- Inspected the rendered development app through the private Unix socket.
- Desktop: 1440 × 1050. Inspected Tasks, capture, Agents, Providers, Notifications, Execution, Memory settings, and API accounts.
- Mobile: 390 × 844. Inspected Agents, execution-limit dialogs, and the task schedule popover.
- No live task or settings mutations were submitted.
- The workspace contained no tasks. Populated task and recurrence recommendations therefore use source evidence.
- Some settings returned access errors through socket access. Those errors are excluded from visual findings.
- Built all ten gallery views with the existing Astryx package, Noema theme, and shared list/settings components.
- Reviewed all ten views at 1440px and 390px widths. No page-level horizontal overflow occurred.
- The gallery produced no browser page errors during that review.
- Checked sample schedule pause feedback and modal opening. Menus, fields, and mock actions change local state only.
- Saved representative desktop and phone screenshots under `screenshots/`.
- The gallery is a visual proposal, not a complete scheduler, task runner, or settings client.
- Production UI source was not changed by this audit. Concurrent edits in the shared workspace were preserved.

## Development access

The user approved extending the existing private development socket.
The extension reuses the existing asset handler and GraphQL handler.
It adds app pages, static assets, auth status, and WebSocket upgrades.
It preserves socket permissions, the POST body limit, and public authentication.

- `CGO_ENABLED=0 go test ./internal/web`: passed.
- `CGO_ENABLED=0 go test ./cmd/... ./internal/...`: passed on the final run.
- `CGO_ENABLED=0 go vet ./cmd/... ./internal/...`: passed.
- One read-only review found that ordinary GET queries could reach the WebSocket path.
  The correction requires an Upgrade header. The shared transport validates the WebSocket handshake.
- The extended unit test checks permitted routes, excluded routes, permissions, body limits, and socket removal.
- Authored Go patch: 19 additions, 1 deletion; test patch: 39 additions, 2 deletions.
  Generated GraphQL patch: zero. Inclusive Go net change: +55 lines.

The user also requested durable browser-inspection instructions and implicit permission.
AGENTS.md, the UI skill, and product guidance now use the same permission rule.
`docs/frontend/browser-inspection.md` documents the process.
`scripts/route-browser-inspection.mjs` provides the read-only Playwright routing helper.

- JavaScript syntax checks passed for the helper and gallery builder.
- The helper loaded the real app and its assets through the Unix socket.
- An HTTP GraphQL mutation request returned 403 inside the helper.
- A WebSocket query returned data. A WebSocket mutation received a local error without reaching the server.
- No TCP authentication relay was created.
- `git diff --check`: passed.

The gallery server contains sample artifacts only. It is separate from authenticated development access.

Tracked Go size at completion: 78197 production, 24914 tests, 79612 generated; 182723 total.
Production and inclusive migration ratios remain below 80 percent.

## Mock refinement after user review

- Removed the separate Inbox/Settings detail header. Those controls now sit within the document body.
- Removed small disclosures from task metadata, task settings, schedule behavior, provider details, notifications, and external agent setup.
- Recurring history reuses ListCardButton and TaskStatusBadge with the existing task-card content arrangement.
- Capture Advanced occupies a full-width section above the footer. Its expanded state preserves button widths and horizontal alignment.
- Model, reasoning, Fast, and task-model enablement stay visible. Phone layouts put the model selector on its own line.
- Rebuilt the gallery and reviewed all ten views at 1440px and 390px widths.
- No horizontal overflow or browser page errors occurred during the gallery review.
- Product UI source remains unchanged. Updated screenshots and recommendations are saved with the mocks.

## Shared task detail refinement

- Removed visible composer labels. Both fields retain accessible names.
- Inbox and recurring mocks share title, timing, instruction, advanced-setting, and floating-bar components.
- Schedule and Reschedule use a calendar icon. Instructions use Edit in both views.
- The floating bar retains status on the left and compact icon controls on the right.
- Execution controls appear only in that bar. Starting the Inbox mock adds no confirmation step.
- Advanced settings stay visible after Instructions and before Run history.
- Inbox controls cover project, agent, and folder. Recurring controls cover missed runs and overlapping runs.
- Recurrence agent and folder overrides are not advertised. The current update contract does not support them.
- Inline controls save within the mock and show local feedback. There is no task-settings dialog or inline Save button.
- The Inbox schedule dialog starts with a single run. Its form does not show overlap controls until repetition is selected.
- The gallery build passed. Reviewed affected views at 1440px and 390px widths.
- Checked inline save feedback, scheduling, instruction-dialog opening, direct starting, pause, and accessible composer fields at both widths.
- Each task detail has one floating bar. No horizontal overflow or browser page errors occurred.
- Updated screenshots contain sample data only. Product UI source remains unchanged by this unit.

## Model label refinement

- Hid Model and Reasoning labels in Agent models and Task models. Accessible names remain available.
- Gallery build passed. Reviewed both groups at 1440px and 390px widths.
- All ten selectors remain available. No horizontal overflow or browser page errors occurred.

## Inbox info row refinement

- Moved Project and Task agent selectors into the info row below the title.
- Hid Working folder in task details and removed the empty Inbox Advanced settings section.
- Removed Inbox, added-time, source, creation-date, and revision metadata from the detail body.
- Recurring schedule settings remain visible before Run history.
- Reviewed Inbox, task options, and recurring details at 1440px and 390px widths.
- Both selectors retain accessible names and save feedback. Sample changes worked at both widths.
- The gallery build passed. No horizontal overflow or browser page errors occurred.

## Task cards in dialogs

- Replaced task-title subtitles with ListCardButton summaries at the start of task dialog bodies.
- Cards reuse the task list title, preview, and state arrangement, including TaskStatusBadge for ordinary tasks.
- The shared pattern covers Start, Cancel, End recurring task, scheduling, and instruction edits.
- Selecting a card opens its task mock. Dialog action buttons retain their existing behavior.
- Reviewed Start, Cancel, and Schedule at 1440px and 390px widths, including modal and drawer previews.
- Checked card presence in instruction edits and End recurring task dialogs at both widths.
- The gallery build passed. No horizontal overflow or browser page errors occurred.
