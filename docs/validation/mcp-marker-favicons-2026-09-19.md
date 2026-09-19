# MCP marker favicons

MCP markers use the shared favicon service with the server website domain.
If that URL is absent or invalid, they use the server icon domain.
If no usable domain or favicon is available, they show the plug icon.

Notion's developer homepage exceeded the favicon source limit.
Page discovery now reads only a bounded prefix and preserves the image size limit.
The handler returned a 32-pixel PNG for `developers.notion.com`.

## Validation

The following checks cover the changes based on `288545c3`:

- `bun run check:generated`: passed. Generated inputs stayed unchanged, so this result was reused.
- `bun run lint`: blocked by the existing missing `enabled` input in `TaskModelPoolsSettings.tsx:100`.
- `../../scripts/with-build-limits bunx eslint src --max-warnings=0`: passed from `apps/web`.
- `bun run build`: passed from `apps/web`.
- `CGO_ENABLED=0 scripts/with-build-limits go test ./internal/web`: passed.
- `CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`: passed.
- `CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`: passed.
- `git diff --check`: passed.

One regression test covers large page discovery and oversized image rejection.
It covers responses with and without a declared content length.
No frontend tests were added.

Browser review covered `/chat` at widths of 1440 and 390 pixels.
The icon loaded, groups expanded, and neither viewport had horizontal overflow.
The private inspection socket lacks the favicon route.
Browser review supplied the PNG returned by the real handler at the expected favicon URL.
This verifies rendering and retrieval separately, not the authenticated public route.

## Size

Frontend code changed by 16 added lines and 20 removed lines.
Go production changed by 20 added lines and nine removed lines.
Go tests added 26 lines. Generated Go did not change.
The inclusive Go increase is 37 lines.

Tracked Go totals are 96,056 production, 69,489 test, and 78,646 generated lines.
The inclusive total is 244,191 lines.
The migration ratios are 55.21 percent production and 101.82 percent inclusive.
The inclusive total already exceeded the 80-percent limit before this change.
This bounded fix does not resolve that existing repository limit.
