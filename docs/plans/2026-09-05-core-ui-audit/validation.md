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

## Shared inline task editor

- Capture, Inbox, and recurring details share the title field and production MarkdownInlineEditor.
- Removed the instruction Edit buttons and instruction-edit dialogs. Both fields are directly editable.
- Existing task edits save locally on blur. Blank titles show an inline error.
- Delayed Markdown updates still save after focus leaves the editor.
- Capture retains drafts until submission. Switching to Markdown source waits for pending editor updates.
- The gallery now includes the existing editor bundle and app styles. Product source remains unchanged.
- Reviewed all ten gallery views at 1440px and 390px widths. No horizontal overflow or browser page errors occurred.
- Checked title edits, instruction edits, save feedback, blank-title errors, and Markdown source switching at both widths.
- An immediate source switch preserved the latest typed instructions at both widths.
- The gallery build and diff checks passed. Screenshots contain sample data only.

Production application must preserve revision checks, available-action restrictions, and stale-draft recovery through the existing task commands.
The gallery keeps edits within the selected mock; it does not implement a task store or server persistence.

## Task model enablement removal

- Removed the enabled state and enable switches from task-model mocks.
- The implementation plan removes this capability from configuration and model selection.
- Model, reasoning, and Fast controls remain available.
- The gallery build passed. Reviewed desktop and phone widths with ten selectors and five Fast switches.
- No enable switches, horizontal overflow, or browser page errors occurred.

## Cancel task simplification

- Removed the optional reason field from the Cancel task mock and implementation recommendations.
- The dialog retains the task card, consequence, Keep task, and Cancel task.
- The gallery build passed. Reviewed desktop and phone layouts and submitted the sample modal at both widths.
- No text fields, horizontal overflow, or browser page errors occurred.

## Capture editor height

- New task instructions fill the remaining pane height above Advanced and the action row.
- Rich text and Markdown source both stretch through the existing editor wrappers.
- Clicking the blank editor area focuses the text editor.
- Reviewed desktop and phone widths with standard and taller panes, plus expanded Advanced controls.
- Increasing pane height from 545px to 800px increased editor height by 255px at both widths.
- Action rows retained their bottom spacing. No horizontal overflow or browser page errors occurred.
- The gallery build passed. Product UI source remains unchanged.

## Actual shell and task continuity

- Re-inspected the live app through the private socket at 1440 × 900 and 390 × 900.
- Compared Tasks, New task, Agents, and Providers with the production shell source.
- All ten mocks now import AppShell. Task and provider panes use the existing responsive layout components.
- Settings reuse the production page track and section header. Iframe viewports apply the real media-query breakpoints.
- Removed separate Task and Recurring task header rows. The close control shares the actual task title row.
- Checked all ten views at both widths. No page errors or horizontal page overflow remained.
- Checked mobile navigation reveal, primary navigation, provider selection, drawer closure, and gallery hash synchronization.
- Checked cancel dismissal and submission within the mock. No cancellation reason field was added.
- Checked rich and source editors with Advanced open. Capture fills the remaining pane height at both widths.
- At 900 pixels tall, the rich editor measured 610 pixels on desktop and 587 pixels on phone.
- The capture actions remained 16 pixels above the detail body's bottom edge.
- Added a gallery-only Inbox, Queued, and Running state control for the same mounted task.
- Measured the shared sticky header and action-bar bounds before and after both state changes.
- Both regions retained identical bounds at both widths. Edited titles and instructions survived the transitions.
- Removed the Progress box. Task options and scheduling scroll beneath the title, close control, and view tabs.
- Reduced header top padding to 12 pixels and its internal gap to 4 pixels.
- Both desktop titles begin at y=65. The detail header remained fixed during a 350-pixel scroll at both widths.
- Task and provider close controls use the production rail's 28-pixel Button treatment.
- Both close buttons matched their list action's vertical center exactly. Provider details also retain a compact sticky title row.
- Checked Workspace and Transcript selection in the running sample.
- The running transcript is sample prose. Full run rendering and very wide TaskBody composition remain production implementation concerns.
- Browser request inspection recorded no GraphQL calls from the sample task views.
- The shell uses an in-memory project response. Unsupported project changes fail locally; no server transport is configured.
- Rebuilt the static gallery with its Vite build script. The bundle-size warning remains advisory for this static gallery.
- No production UI files or UI tests were added or changed. Live task data was not modified.
