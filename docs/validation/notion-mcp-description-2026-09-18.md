# Notion MCP connection failure

Notion accepted browser consent and returned a token. Noema then rejected the
provider's tool metadata. The callback showed a generic connection error.
Temporary diagnostics confirmed successful authorization followed by
`MCP tool metadata is invalid`. Those diagnostics were removed after diagnosis.

The description limit was 8,192 bytes in discovery and SQLite. Discovery now
accepts descriptions up to 65,536 bytes. Migration 39 applies the same limit
without changing existing tool identities, schemas, or permissions.
Descriptions remain intact. Existing response and tool-count limits remain.

## Validation

Base revision: `99876a97`.

- `CGO_ENABLED=0 scripts/with-build-limits go test ./internal/mcp ./internal/store`
  passed for the current code changes.
- The existing authorization test now completes with a 18,200-byte description.
  It verifies exact stored text after the callback completes.
- The description boundary test accepts 65,536 bytes and rejects one extra byte.
- The migration test upgrades version 38 with an existing disabled tool and a
  human permission choice. It verifies preserved values, fresh-schema agreement,
  the byte limit, and foreign keys.
- The old boundary assertion now uses the new limit.
- The development database reached version 39.

`CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`
passed for the same code changes. The check reused cached passing package results.
The initial concurrent vet run was stopped because compilation competed with
asset watchers for memory. The asset watchers were restarted through their
existing development supervisor. The authenticated server remained available.
`CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`
then passed for the same code changes. No application source changed after
these checks.

The real Notion connection completed at 19:26 UTC on September 18, 2026.
The saved connection is authenticated and contains 45 tools. Two stored tool
descriptions exceeded the old limit: `notion-create-pages` has 8,685 bytes, and
`notion-update-page` has 9,356 bytes. This confirms the live cause and recovery.
No Notion content was changed during verification.

## Review and size

A local review checked the table copy, indexes, foreign keys, byte counts, and
unchanged permission fields. No further correction was required.

The patch adds 28 production lines and 85 test lines, net. Generated GraphQL
has no changes. The inclusive increase is 113 lines. Two tests are new.
The estimates were 100 production lines and 80 test lines. Both remain within
the permitted estimate margin.

Current tracked Go totals, including the two new files:

| Class | Lines |
| --- | ---: |
| Production | 95,068 |
| Tests | 69,235 |
| Generated GraphQL | 79,566 |
| Inclusive | 243,869 |

Production is 54.64% of the fixed Rust baseline. The inclusive ratio is 101.69%.
The inclusive budget already failed at the base revision: 243,756 lines against
an upper limit of 191,860. This patch does not resolve that existing budget issue.
