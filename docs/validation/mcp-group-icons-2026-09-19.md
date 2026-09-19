# MCP icons in call groups

Failed calls can lack the server metadata that supplies their favicon domain.
Each row previously looked only at its own result.

A tool-call group now shares known favicon domains by MCP tool namespace.
Each call keeps its own domain when available.
Expanded rows receive the same resolved domain as the group.
Error indicators, labels, and result details keep their existing behavior.
If the group has no icon information for that namespace, the plug icon remains.
No saved records change.

## Validation

The patch starts at `e2a36bc2`.

- `bun run check:generated`: passed. Schema and operation inputs remained unchanged after this check.
- `bun run lint`: blocked by the existing missing `enabled` input in `TaskModelPoolsSettings.tsx:100`.
- `../../scripts/with-build-limits bunx eslint src --max-warnings=0`: passed from `apps/web`.
- ESLint also passed for both changed files after the expanded-row correction.
- `bun run build`: passed after the correction.
- `git diff --check`: passed.

Browser review used the reported five-call group at widths of 1440 and 390 pixels.
All five Notion icons loaded. Both failed calls kept their error indicators.
Neither viewport had horizontal overflow.
A browser-only response change assigned successful calls to another MCP namespace.
The failed calls then kept their fallback icons, confirming that icon information did not cross namespaces.
No live product data changed during this check.

The private inspection socket lacks the favicon route.
Browser review supplied the PNG from the earlier production-handler check at its expected URL.
This checks icon rendering separately from favicon retrieval.

The patch adds 24 frontend lines and removes six.
No server code or generated artifacts change.
No frontend tests were added, and no backend checks were repeated.
