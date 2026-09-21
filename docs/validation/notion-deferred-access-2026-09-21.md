# Notion access after deferred loading

Base revision: `bf1b4a9b`.

## Reproduction and cause

The user authorized sending Chat messages as them through noema-dev.
The exact capability question reproduced the reported denial.
A second request asked for an accessible Notion page without changes.
Noema again denied access without attempting a Notion tool.

Read-only inspection found Notion enabled, healthy, and authenticated.
The catalog contained 45 Notion tools, including search and fetch.
Their definitions were deferred. The callable-name list omitted these tools
and also omitted the provider's `tool_search` loader.

The instructions did not explicitly identify deferred entries as available
service access. The model treated the callable-name list as evidence of
disconnection. This was a regression in the previous loading change.

## Correction

Chat now lists provider-native `tool_search` when deferred tools exist.
Chat and Tasks explain that deferred entries establish access for their
owning service. They instruct the model to load definitions before use.
The instructions distinguish deferred loading from actual access failures.

The change does not alter connection setup, role filtering, source schemas,
authentication, or action review.

## Live verification

After the development server rebuilt, the exact original question produced
a positive answer limited to shared Notion content.
A follow-up requested one accessible item without changes.
Noema successfully called these tools:

- `notion-get-tool-access`
- `notion-list-recent-pages`
- `notion-fetch`

Each stored tool result reported success.
The returned item was a database. Noema correctly identified it as a database
and reported its title. No modifying tool ran.
The page content and identifiers are not copied into this repository.

The four authorized test messages remain in the primary Chat.
Local CLI event logs are under `/var/tmp/noema-notion-*-20260921.jsonl`.

## Validation

The focused prompt checks passed in 0.041 seconds:

```sh
TMPDIR=/var/n20 CGO_ENABLED=0 scripts/with-build-limits go test ./internal/runtime -run 'TestDeferredAccess|TestToolVisibility|TestTaskToolCatalog'
```

The regression verifies native loader visibility, service ownership, and
access instructions. It also checks that non-native and eager catalogs do
not falsely advertise native tool search.

Frontend rebuild watchers were temporarily stopped to release retained build
memory. The server and its authenticated inspection socket remained available.

Broad static checks passed without diagnostics:

```sh
TMPDIR=/var/n20 CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...
```

## Size and review

The patch adds 15 production lines and 20 test lines.
The budget was 40 production lines and 35 test lines.
Generated GraphQL is unchanged. No database migration was added.

Tracked Go totals are 96,600 production, 69,980 tests, and 78,646 generated lines.
The inclusive total is 245,226.
Production remains below the migration limit.
The inclusive total retains the existing repository breach of that limit.

Inline review checked loader visibility and current service ownership.
The model still needs actual tool results before claiming a successful read.
The live verification establishes that result for the connected Notion account.

The full server tests passed:

```sh
TMPDIR=/var/n20 CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...
```

These results cover the final production and test patch on `bf1b4a9b`.
Documentation updates reuse those results.
Logs are under `/var/tmp/noema-notion-*-20260921.log`.
The existing development supervisor restarted frontend builds after validation.
