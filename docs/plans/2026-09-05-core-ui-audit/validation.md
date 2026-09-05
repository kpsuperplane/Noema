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
