# Native Markdown Memory Exploration

## Status

This is the approved first-slice contract for replacing Mnemosyne with native
Markdown memory. It authorizes a memory-specific exception to the previous
storage contract and a destructive, migration-free cutover from Mnemosyne.

## First-Slice Contract

The first slice is deliberately narrow:

1. There is one open memory tree, scoped to and owned by `human:local`.
2. Markdown is canonical for memory prose and semantic metadata, including
   ownership, scope, provenance, and consolidation state.
3. SQLite FTS is a disposable search index rebuilt from Markdown.
4. The filesystem hierarchy is canonical for parent-child structure. Pages do
   not maintain links to their children.
5. Each memory page has a hard limit of 750 words.
6. Each memory job reads one conversation. The first slice reads only the local
   human's primary conversation and uses its existing `sequence_index` order.
7. Context compaction normally schedules memory consolidation, but context
   summarization and memory updating are separate model operations.
8. The Memory page has an `Update memory` action for processing every eligible
   message since the last completed consolidation.
9. Only one memory-update job may run at a time.
10. Direct filesystem editing is unsupported. Noema owns all files in the tree.
11. The memory subsystem retains no page history or versions.
12. All first-slice memory is open. Private chat and private memory are
    deferred together.
13. The first slice completely removes Mnemosyne and starts with an empty native
    tree. There is no import or compatibility path.

Project and workspace trees, shared ownership, cross-scope reconciliation,
private retrieval, Ghost mode, editing, history, and migration are outside this
slice.

## Verdict

The slice is practical once native memory becomes the only memory authority. A
bounded Markdown tree provides ambient context, deterministic navigation,
readable provenance, and simple backup without reviving the earlier
graph-oriented design.

The principal architecture change is intentional: semantic memory metadata
lives in Markdown frontmatter instead of canonical SQLite rows. The canonical
SQLite database continues to own conversations and source messages. A separate
SQLite database owns only the rebuildable FTS projection.

Existing designs support this shape:

- [Claude Code memory](https://code.claude.com/docs/en/memory) uses a bounded
  root file with topic files loaded on demand.
- [OpenClaw memory](https://docs.openclaw.ai/concepts/memory) treats Markdown as
  canonical and builds derived search infrastructure around it.
- [Letta's context hierarchy](https://docs.letta.com/guides/core-concepts/memory/context-hierarchy)
  separates small always-visible memory from retrieved detail.
- [SQLite FTS5](https://www.sqlite.org/fts5.html) provides the rebuildable
  lexical index needed by the first slice.

Noema already has relevant seams in context compaction, provider selection,
model-context assembly, conversation-item persistence, and the Memory page.
The existing `MemoryOperations` and Mnemosyne-specific seams are replacement
targets, not compatibility layers to preserve.

## One Conversation Source Stream

Every memory job is bound to exactly one `conversation_id`. In the first slice,
that conversation is the local human's active primary conversation. A job never
reads, merges, or orders items from another conversation.

`conversation_items.sequence_index` already provides the required durable order
inside that conversation. An update captures the conversation's current head
and performs the range query:

```sql
SELECT ...
FROM conversation_items
WHERE conversation_id = :conversation_id
  AND sequence_index > :last_consolidated_sequence
  AND sequence_index <= :captured_head_sequence
ORDER BY sequence_index ASC;
```

The updater uses completed human-authored messages as memory evidence.
Assistant messages inside the selected range may provide bounded context but
are not independent evidence. The captured head makes the job finite; items
appended while it runs remain pending for the next update.

Large ranges are processed in source-order chunks sized to the selected memory
model's context budget. A single UI job continues chunking until it reaches its
captured head. Each committed chunk advances the checkpoint, so a failed job
can resume without repeating earlier chunks.

If later slices allow other conversations to contribute to the same human
tree, each conversation gets its own job and checkpoint. They still do not
create a cross-conversation source query. That extension is outside the first
slice.

## Storage Model

### One local-human tree

```text
memory/
└── human/
    ├── root.md
    ├── .state.md
    ├── .pending/
    ├── preferences.md
    ├── people.md
    └── people/
        └── collaborators.md
```

`root.md` is supplied to the agent on every ordinary turn. Topic pages contain
details retrieved on demand. `.state.md` is machine-managed consolidation
state. `.pending/` holds only the current crash-recovery operation and is
deleted after a successful commit; it is transaction staging, not history.

### Filesystem-derived hierarchy

Directory enumeration supplies navigation. `root.md` has every non-hidden
Markdown file beside it as an immediate child, excluding itself. A page such as
`people.md` has the Markdown files inside the matching `people/` directory as
its children. If the directory does not exist, the page has no children.

The runtime returns child filenames in deterministic lexical order together
with stable page IDs. Page bodies may contain ordinary prose links, but those
links do not define the tree. The updater may split or move pages while
preserving stable IDs.

### Canonical page metadata

```markdown
---
schema: noema.memory.page/v1
id: memory_page:human_local:preferences
owner: human:local
scope: human:local
created_at: 2026-07-19T12:00:00-07:00
updated_at: 2026-07-19T12:00:00-07:00
sources:
  - conversation_item:item_123
---

# Preferences

The human prefers readable durable state.[^item-123]

[^item-123]: `conversation_item:item_123`
```

Frontmatter is authoritative for page identity, ownership, scope, timestamps,
and the page-level source manifest. The relative path defines parentage.
Footnotes attach provenance to individual claims, and the manifest is validated
against citations in the body.

### The 750-word limit

Every memory page body, including headings and citation footnotes but excluding
frontmatter, is limited to 750 words using one deterministic Unicode word-count
implementation. `root.md` has the same limit as every topic page. Machine state
and temporary recovery manifests are exempt because they never enter model
retrieval.

When a proposed change would exceed 750 words, the updater must compress the
page, remove stale low-value content allowed by retention policy, or create a
child page. Validation rejects an over-limit model proposal before publication.

### Durable consolidation state

```markdown
---
schema: noema.memory.state/v1
conversation_id: conversation:primary_123
last_consolidated_sequence: 42
last_consolidated_item: item:item_123
updated_at: 2026-07-19T12:00:00-07:00
---

# Memory State

This file is managed by Noema and is not part of model retrieval.
```

`conversation_id` binds the state to the primary conversation.
`last_consolidated_sequence` is that conversation's query cursor. The item ID
is retained for diagnostics and provenance rather than ordering. Initial state
uses the current primary conversation, sequence zero, and no item ID.

## Derived SQLite Search

The rebuildable index lives separately from canonical Noema state, for example
at `${NOEMA_HOME}/system/indexes/memory.sqlite3`. Deleting this file and
rescanning Markdown must restore equivalent search results without affecting
conversations or consolidation state.

The index contains only derived fields:

- FTS-indexed headings and body text;
- relative path and stable page ID;
- cached owner and scope values;
- parsed citations when useful for reverse lookup;
- content hashes and indexing diagnostics.

Because direct filesystem editing is unsupported, Noema updates the index only
as part of its own memory transaction. There is no first-slice file watcher or
external-edit reconciliation path.

The first slice uses lexical FTS only. Embeddings remain deferred until an
evaluation demonstrates retrieval failures that hierarchy plus FTS cannot
handle.

## Retrieval

The runtime supplies the bounded `root.md` body and its derived immediate-child
listing as a model-context section on every ordinary turn.

Two model tools provide deeper retrieval:

1. **Read a memory page.** Open a returned page ID and receive its body plus a
   freshly derived list of immediate child filenames and IDs.
2. **Search memory.** Search FTS and receive ranked snippets and page IDs when
   the relevant branch is not apparent from the hierarchy.

Persisted tool results retain only the page ID and content hash. Resolved page
text is supplied to the current model continuation but is not copied into the
durable transcript, so later memory corrections do not compete with old
transcript copies.

Only the `human:local` scope is advertised by the first-slice tools. Existing
conversation and project memory-scope instructions are removed with the
Mnemosyne integration.

## Consolidation

### Triggers and model separation

Context compaction is the normal trigger because it identifies a coherent batch
as its verbatim messages leave active model context. A successful compaction
schedules that conversation's captured source head but does not wait for
memory.

The memory subsystem independently reads the durable source range, selects the
configured Memory model, and sends a background-priority generation request.
It shares no model output or transaction with context summarization.

The Memory-page action and startup recovery may schedule the same operation.
Only one update job runs at once. A trigger received during an active job does
not start concurrent work; newer messages remain pending after the captured
head and can be handled by the next job.

### Updater instructions

The updater reconciles new evidence with existing pages rather than appending
contradictions. It returns a structured change set containing expected page
hashes, citations, and proposed creates, updates, moves, or deletions.

The updater prompt explicitly tells the model not to store literal passwords,
API tokens, private keys, session cookies, recovery codes, or similar secrets.
It may retain a non-secret fact such as which service the human uses. This is
advisory in the first slice, not a deterministic sensitivity guarantee.

Deterministic validation covers Noema and model behavior: schema, stable IDs,
ownership, scope, citations, the 750-word limit, and relative paths that remain
inside the memory root. It does not attempt to make unsupported external file
edits safe.

### Crash-recoverable publication

For each bounded source chunk, the updater:

1. Reads the current pages and obtains a complete model-proposed change set.
2. Validates the proposed final tree and file hashes.
3. Writes desired file bytes and a manifest under `.pending/<operation_id>/`,
   then makes the staging data durable.
4. Acquires the single tree read/write lock and publishes the staged files in a
   deterministic order.
5. Updates the derived FTS index from the final published tree.
6. Atomically replaces `.state.md` with the conversation ID, the chunk's final
   `sequence_index`, and its item ID.
7. Removes the pending operation and releases the lock.

Memory reads wait while publication holds the lock. If Noema stops after
staging or during publication, startup completes the pending operation forward
before enabling retrieval or another update. Desired content hashes make this
recovery idempotent. No old page generation is retained after the operation
finishes.

## Memory Page Update Action

The existing Memory-page action area contains one `Update memory` control next
to the last successful update state. It does not add a modal, editor, or second
action panel.

| State | Button | Supporting status |
| --- | --- | --- |
| No pending messages | Disabled `Update memory` | `Memory is up to date` |
| Messages pending and no job active | Enabled `Update memory` | Pending message count and last successful update time |
| Job queued or running | Disabled `Updating memory...` | Loading state until the one active job finishes |
| Last job failed and no job active | Enabled `Retry update` | Concise inline error and previous successful update time |

Clicking the enabled action captures the primary conversation's current head
and starts one background job for that conversation. The button disables
immediately. The server independently enforces the single-job rule, so a
duplicate mutation cannot create another job. The job continues if the human
leaves the page.

On success, the page refreshes memory, the pending count, and the last-update
state. Messages appended after the captured head remain pending and may make
the button enabled again. Failure leaves the last completed chunk checkpoint
authoritative, and retry continues from there.

## Unsupported Direct Editing

The files are readable and backup-friendly, but direct human or external-tool
editing is an illegal operation in the first slice. Noema does not watch for,
merge, protect against, or recover from such edits. It may overwrite them, fail
validation, return stale search results, or leave memory unavailable.

This deliberately removes file-watcher, external conflict, symlink/hard-link,
and stale-index handling from the first slice. Noema still validates paths
originating from its model updater so the model cannot intentionally write
outside the memory root.

There is no in-product editor, page history, snapshot directory, version table,
or undo mechanism.

## Mnemosyne Removal and Cutover

Native Markdown memory becomes the only memory authority in this slice. The
same implementation unit removes:

- the managed Mnemosyne process and lifecycle;
- Mnemosyne clients, adapters, packages, and dependencies;
- per-message Mnemosyne observation submission;
- managed/external Mnemosyne configuration and readiness settings;
- the Mnemosyne model proxy and Mnemosyne-specific search result shapes;
- article generation and graph adapters that read Mnemosyne;
- obsolete tests, fixtures, documentation, and home-layout references.

The Memory model preference remains and is used directly by the native updater.
The existing Memory page is repointed to native Markdown and its derived
citations. `search_memory` is repointed to the native FTS/read path.

Existing Mnemosyne facts are not imported. The cutover starts with an empty
Markdown tree and removes `${NOEMA_HOME}/mnemosyne`; there is no dual-running
period, compatibility mode, or rollback to Mnemosyne.

## Implementation Sequence

1. Add bounded source-range reads over the primary conversation's existing
   `sequence_index`.
2. Implement the Markdown page, tree, 750-word, state, and recovery-manifest
   contracts.
3. Implement the separate rebuildable FTS database.
4. Replace memory root injection and read/search operations with the native
   human tree.
5. Implement background consolidation, chunking, crash recovery, and automatic
   scheduling after compaction.
6. Replace the Memory-page query and article action with native display state
   and the single-flight `Update memory` operation.
7. Delete Mnemosyne code, dependencies, configuration, runtime state, APIs,
   tests, and stored data; update authoritative project documentation.
8. Evaluate recall quality, unsupported claims, citation accuracy, page churn,
   retrieval misses, secret-copy behavior, and root growth.

The slice is complete only when the repository and runtime have one memory
authority. Intermediate implementation commits may not ship with both systems
active.

## Acceptance Criteria

- A fresh or cut-over installation has one empty open tree owned by and scoped
  to `human:local`, with no Mnemosyne process, code path, or stored data.
- Every job selects eligible source items from only the primary conversation
  using its existing sequence order.
- A large source range is processed in ordered bounded chunks through the
  captured head.
- Every retrievable page body contains at most 750 words.
- Root and page reads derive immediate child filenames from the filesystem in
  deterministic lexical order.
- Deleting the separate FTS database and rescanning Markdown restores
  equivalent native search results.
- Context compaction and the Memory-page action schedule a separate
  background-priority memory operation.
- `Update memory` is disabled and loading while the one job is active; no
  second job can start.
- Crash recovery completes a staged chunk before retrieval resumes and advances
  the checkpoint only after pages and FTS are current.
- The model updater cannot propose files outside the memory root.
- Persisted memory tool results contain page references and hashes rather than
  copied page bodies.
- The implementation contains no editor, external-edit reconciliation, page
  history, private memory, additional scope, migration, or Mnemosyne fallback.

Focused tests should cover conversation-bound range selection and captured
heads, bounded chunk recovery, page/tree validation and the 750-word limit,
deterministic child enumeration, FTS deletion and rebuild, staged-operation
crash recovery, single-flight update state, replay-ephemeral page reads, and
the absence of a Mnemosyne runtime path. Each test protects a distinct
data-integrity, retrieval, or cutover risk.

## Remaining Questions

1. **Forgetting:** Which unpinned low-value claims may the updater remove?
2. **Pinning:** How does Noema mark a claim as protected from automatic removal
   without adding an editing surface?
3. **Source deletion:** Must deleting a conversation item retract derived
   memory in the first slice, or is that explicitly deferred?
4. **Child breadth:** What maximum number or byte budget applies to the derived
   child listing appended to a page?
5. **Automatic scheduling:** Is compaction plus startup recovery sufficient, or
   should task completion also request an update?

## Non-goals

This proposal does not select final prompts or models, add vectors, implement
additional scopes, define privacy modes, support direct editing, retain page
versions, or preserve Mnemosyne data. Its purpose is to define a single,
coherent native memory authority that can be evaluated through implementation.
