# Native Markdown Memory Exploration

## Status

This is the durable working proposal for a first native Markdown memory slice.
It is not yet an approved replacement for Mnemosyne or the current project
storage contract. Decisions recorded here become authoritative only after the
proposal is approved and the relevant architecture documents are updated.

## First-Slice Decisions

The first slice is deliberately narrow:

1. There is one memory tree, scoped to the local human.
2. The local human owns every page in that tree.
3. Markdown is canonical for prose and semantic metadata, including ownership,
   scope, provenance, and the consolidation checkpoint.
4. SQLite is a disposable full-text search index that can be rebuilt from the
   Markdown tree.
5. The filesystem hierarchy is canonical for parent-child structure. No page
   maintains a list of links to its children.
6. Context compaction is the normal memory-update trigger, but memory updating
   is a separate subsystem and model operation from context summarization.
7. The Memory page has an `Update memory` action that processes every eligible
   message since the last successful update.
8. The filesystem is the only human editing interface. There is no in-product
   memory editor in this slice.
9. The memory subsystem keeps no page history or versions.
10. All first-slice memory is open. Private memory and private chat behavior are
   deferred together.
11. The first slice does not import Mnemosyne data or define a cutover or
   migration path.

Project and workspace trees, shared ownership, cross-scope reconciliation,
private retrieval, Ghost mode, in-product editing, history, and legacy-memory
transition are outside this slice.

## Verdict

This first slice is practical. A bounded Markdown tree can provide useful
ambient memory, directed retrieval, manual human correction, and readable
provenance without rebuilding Noema's earlier graph-oriented design.

The main architectural exception is intentional: memory metadata would live in
Markdown frontmatter rather than in SQLite. That differs from Noema's current
general storage contract, so implementation requires an explicit approval for
memory-specific file authority. Everything stored in SQLite remains derived
and recoverable.

## Evidence for the Design

Existing systems validate the main shape of the proposal:

- [Claude Code memory](https://code.claude.com/docs/en/memory) uses a concise
  `MEMORY.md` index with linked topic files and loads only a bounded portion
  automatically.
- [OpenClaw memory](https://docs.openclaw.ai/concepts/memory) treats plain
  Markdown as canonical and builds derived search infrastructure around it;
  its [built-in memory implementation](https://docs.openclaw.ai/concepts/memory-builtin)
  combines file watching, SQLite full-text search, and optional vectors.
- [Letta's context hierarchy](https://docs.letta.com/guides/core-concepts/memory/context-hierarchy)
  separates small always-visible memory blocks from larger material retrieved
  on demand.
- Anthropic's [memory tool guidance](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)
  makes the application responsible for path safety, size limits, and
  sensitive-data handling rather than trusting model-written files directly.
- [SQLite FTS5](https://www.sqlite.org/fts5.html) supports a rebuildable search
  layer without making the database the source of truth.

Noema also has useful implementation seams:

- `crates/noema-memory/src/operations.rs` centralizes current memory
  operations.
- `crates/noema-runtime/src/daemon/runtime/context_compaction.rs` identifies
  compaction boundaries and durable source ranges.
- `crates/noema-runtime/src/daemon/runtime/turn/provider_request.rs` persists
  conversation items before asynchronous observation work.
- `crates/noema-runtime/src/daemon/runtime/model_context.rs` assembles explicit
  context sections where the memory root can be injected.
- `crates/noema-runtime/src/daemon/runtime/artifact_writes.rs` contains hash
  and version concepts that can inform conflict-safe file writes, even though
  the memory subsystem will not retain page versions.

## First-Slice Storage Model

### One local-human tree

The initial corpus has one root and can grow topic pages as needed:

```text
memory/
└── human/
    ├── root.md
    ├── .state.md
    ├── preferences.md
    ├── people.md
    └── people/
        └── collaborators.md
```

`root.md` is a compact summary supplied to the agent on every ordinary turn.
Topic pages contain details retrieved on demand. `.state.md` is machine-managed
Markdown and is excluded from retrieval.

Directory enumeration supplies the table of contents. `root.md` has every
non-hidden Markdown file beside it as an immediate child, excluding `root.md`
itself. A page such as `people.md` has the Markdown files inside the matching
`people/` directory as its children. If that directory does not exist, the page
has no children.

The process may reorganize topic pages as the corpus changes, but it must
preserve stable page IDs. It never writes child-link lists into page bodies.

### Canonical page metadata

Every memory page carries machine-readable frontmatter. A tentative format is:

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
and the page-level source manifest. The relative file path is authoritative for
parentage. Footnotes attach provenance to specific claims. The manifest is
validated against citations in the body.

Ownership and scope look redundant in the first slice because both identify
the local human. They express distinct facts and keep the copied Markdown tree
self-describing: ownership answers who controls the data, while scope answers
whose memory the page describes.

### Durable consolidation state

The tree checkpoint also remains in Markdown:

```markdown
---
schema: noema.memory.state/v1
last_consolidated_item: conversation_item:item_123
updated_at: 2026-07-19T12:00:00-07:00
---

# Memory State

This file is managed by Noema and is not part of model retrieval.
```

Keeping this checkpoint outside SQLite means deleting the search database
cannot make the updater forget its durable position. The checkpoint is written
only after every page change for the source range has been published.

### Tree invariants

1. The tree has exactly one root.
2. A page's immediate children are derived from the matching filesystem
   directory; page-authored child lists have no structural meaning.
3. Every non-root directory containing memory pages has a corresponding parent
   Markdown file beside that directory.
4. Hidden files and `.state.md` never appear as memory children.
5. Stable page IDs survive file moves and title changes.
6. Every page is owned by and scoped to `human:local`.
7. Root and topic pages obey deterministic context budgets.
8. Synthesized claims cite durable conversation item IDs.
9. The state file is never included in retrieval or search results.

The earlier 750-word limit remains a useful editorial target, but a
deterministic character or token budget is safer for multilingual text and
prompt assembly. Exceeding a budget triggers compression, removal of stale
low-value material, or a child-page split. It does not by itself justify
removing a fact the human has explicitly pinned.

## Derived SQLite Search

SQLite stores only data that can be reconstructed by scanning the Markdown
tree:

- FTS-indexed headings and body text;
- page path and stable page ID;
- cached owner and scope values used to validate results;
- parsed citations when they make reverse lookup faster;
- content hashes and indexing diagnostics.

The duplicated metadata is a cache, not authority. Before returning content,
the retrieval layer checks the current frontmatter rather than trusting an
index row that may be stale.

The first slice uses lexical FTS only. Embeddings remain unnecessary until an
evaluation demonstrates retrieval failures that FTS plus tree navigation
cannot handle.

## Retrieval

The runtime supplies `root.md` as a bounded context section on every ordinary
turn. It appends a deterministic listing of the root's immediate child
filenames and page IDs, derived from the filesystem rather than maintained in
the root's prose.

Two model tools complete retrieval:

1. **Read a memory page.** The model opens a page ID returned by the filesystem
   listing. The result contains the page body plus a fresh listing of its
   immediate child filenames and page IDs.
2. **Search memory.** FTS returns ranked snippets and page IDs when the correct
   branch is not apparent from the root.

Filesystem enumeration makes hierarchy deterministic and frees the model from
keeping navigational links synchronized. Search remains necessary because
filenames and the immediate tree structure cannot anticipate every future
query. Using both keeps ordinary navigation cheap while preserving a semantic
fallback.

Full page content should not be copied permanently into every conversation
transcript. Persisted tool results can retain page references and content
hashes while resolved text remains replay-ephemeral. Later corrections to a
memory page then take effect instead of competing with old transcript copies.

## Memory Page Update Action

The Memory page supports the human job of making the visible memory current on
demand. It reuses the page's existing action area and keeps the control beside
the last-update state rather than adding another card, modal, or editing
surface.

The control is labeled `Update memory`. Clicking it captures the latest durable
conversation-item boundary and schedules the same consolidation operation used
by automatic triggers. The job processes every eligible human message after
`last_consolidated_item` through that captured boundary. Intervening assistant
messages may provide bounded context but are not independent memory evidence.
Messages committed after the captured boundary remain pending for the next
update.

The action has four visible states:

| State | Button | Supporting status |
| --- | --- | --- |
| No pending messages | Disabled `Update memory` | `Memory is up to date` |
| Messages pending | Enabled `Update memory` | The pending message count and last successful update time |
| Queued or running | Disabled `Updating memory...` | Progress remains visible if the human stays on the page |
| Failed | Enabled `Retry update` | A concise inline error; the previous successful update time remains authoritative |

A successful update refreshes the rendered memory and its last-update state.
The job continues if the human leaves the page. Repeated clicks cannot start
parallel work: while a job is queued or running, the server returns the same
in-flight operation. Failure leaves the Markdown checkpoint unchanged, so
retrying covers the same unconsolidated range.

## Consolidation Lifecycle

### Context compaction as the normal trigger

Context compaction is the best default boundary because it identifies a
coherent source batch just as its verbatim messages leave active model context.
The underlying conversation items remain durable, so the update does not need
to block compaction to prevent data loss. Processing one batch per compaction
also avoids paying for a memory-model call on every message.

The context summarizer and memory subsystem remain operationally separate:

- compaction commits its conversation summary without waiting for memory;
- a successful compaction emits or schedules a source boundary for memory;
- the memory subsystem reads the durable source items independently;
- the memory subsystem uses its own model request and validation path;
- memory failure cannot invalidate or delay a valid context summary.

The two subsystems may share source-range identifiers, but they do not share a
model output or transaction. This preserves independent model selection,
quality evaluation, retries, and failure handling.

Compaction is the normal trigger, not the only way to schedule the same memory
job. The Memory-page action, an explicit request to remember something, task
completion, or startup recovery may schedule an update when persisted items
are newer than the checkpoint. These paths do not introduce a second update
mechanism; they feed the same single-flight consolidation queue.

### Update transaction

The updater:

1. Reads durable conversation items after `last_consolidated_item` through the
   scheduled boundary.
2. Loads the root, then enumerates child files or searches for relevant pages.
3. Asks the memory model for a structured change set containing expected page
   hashes, citations, and any proposed page splits or moves.
4. Validates IDs, frontmatter, paths, filesystem parentage, budgets, citations,
   ownership, scope, and expected hashes.
5. Acquires the tree write lock and stages the file changes.
6. Publishes page files and directory changes, then writes `.state.md` last.
7. Rebuilds the affected derived FTS entries from committed Markdown.

Writing the checkpoint last gives the process at-least-once behavior. A crash
may repeat a source batch but cannot mark an incomplete memory update as
finished. Expected hashes prevent a background update from overwriting a
concurrent manual edit, while stable source citations let retries reconcile
content instead of blindly duplicating it.

## Manual Editing

The filesystem is the first-slice editing interface. The human may edit, move,
add, or remove Markdown pages directly. Noema watches or rescans the tree,
validates changed files, and refreshes the derived FTS entries.

An invalid edit remains on disk and produces a diagnostic, but it is excluded
from model retrieval until corrected. Noema does not silently repair or
overwrite the human's invalid file. If a background update encounters a changed
page hash, it abandons that proposed change and retries against the new file.

There is no page history, snapshot directory, memory-specific version table,
undo mechanism, or in-product editor. General filesystem backups or Git may
version the files outside this subsystem, but the memory design does not depend
on them.

## Revision and Source Deletion

New evidence should rewrite a belief rather than append contradictions
indefinitely. When a page reaches its budget, the updater chooses among
compression, removing stale low-value material, and moving useful details into
a child page. The exact pinning and forgetting policy remains unresolved.

Claims cite the durable conversation items that support them. If source
deletion must also remove derived memory, the system will need to find pages
citing that item and regenerate the affected claims from their remaining
sources. The first slice should not claim source-deletion propagation until
that behavior and its failure cases are explicitly included in scope.

## Explicitly Deferred Design

### Additional scopes

Project and workspace memory trees are deferred. The first slice has no scope
activation rules, cross-scope links, or duplication policy because
`human:local` is the only available scope.

### Private memory

The intended future rule is that private pages can be read only while the chat
is in a private context. The first slice has no private-chat context, so it also
creates no private pages, private index entries, authorization metadata, or
private consolidation path. All first-slice pages are open.

Ghost mode and automatic sensitivity classification are separate future
runtime features.

### Existing memory

The first slice starts with a fresh Markdown tree. It does not import existing
Mnemosyne data and does not specify how native memory eventually replaces,
coexists with, or retires Mnemosyne. That transition is a later product and
architecture decision.

## Proposed Implementation Sequence

1. **Define the file contract.** Implement frontmatter types, Markdown parsing,
   filesystem-derived child enumeration, tree validation, deterministic
   budgets, and `.state.md` checkpoint parsing.
2. **Build the derived index.** Scan the tree into SQLite FTS, update changed
   files, exclude invalid files, and prove a deleted database can be rebuilt.
3. **Add retrieval.** Inject the bounded root and expose read/search operations
   against the single human tree.
4. **Add consolidation.** Consume a scheduled source range, navigate relevant
   pages, validate a model-proposed change set, publish files, and advance the
   checkpoint last.
5. **Add the manual update action.** Put `Update memory` in the existing Memory
   page action area and expose pending, running, success, and retry states from
   the same single-flight consolidation operation.
6. **Connect automatic triggers.** Schedule the separate memory job after
   compaction and through explicit remember, task-completion, and
   startup-recovery paths.
7. **Evaluate the complete loop.** Measure useful recall, unsupported claims,
   citation accuracy, page churn, retrieval misses, and root growth against
   representative conversations.

## First-Slice Acceptance Criteria

- A fresh installation creates one open tree owned by and scoped to
  `human:local`.
- Deleting SQLite and rescanning Markdown restores equivalent search results.
- The bounded root appears in ordinary model context, and the model can read or
  search topic pages on demand.
- Root and page reads surface immediate child filenames from the filesystem;
  parent pages contain no required child-link list or `parent` metadata.
- A successful compaction schedules a separate memory-model operation without
  coupling summary success or latency to memory success.
- `Update memory` processes the eligible range through the click-time durable
  boundary, exposes single-flight status, and refreshes memory on success.
- A failed or interrupted update does not advance the Markdown checkpoint.
- Manual file edits are detected and reindexed without an in-product editor.
- Concurrent human edits are not overwritten by stale model proposals.
- The subsystem creates no page history and no private, project, or workspace
  memory artifacts.
- The implementation contains no Mnemosyne import, cutover, or migration path.

Focused tests should cover frontmatter and tree validation, deterministic child
enumeration, index rebuilding, root budgeting, checkpoint-last recovery,
click-time boundary and single-flight behavior, citation validation, and
stale-hash conflict handling. These tests each protect a distinct
data-integrity, update, or retrieval risk.

## Remaining Questions

The first slice still needs decisions on:

1. **Page budgets:** What token or character budgets should apply to the root
   and to topic pages?
2. **Forgetting:** Can the updater remove any unpinned low-value claim, or should
   deletion require an explicit confidence, age, or utility threshold?
3. **Pinning:** How does a human mark a claim as protected from automatic
   removal using Markdown alone?
4. **Source deletion:** Is propagation of conversation-item deletion required
   before the first slice is considered safe to use, or explicitly deferred?
5. **Scheduling:** Should task completion always enqueue consolidation, or only
   when the uncompacted source range exceeds a small threshold?
6. **Invalid edits:** Is exclusion plus a runtime diagnostic sufficient without
   an in-product editing or repair surface?

## Non-goals

This proposal does not approve the memory-specific storage-contract exception,
select final prompts or models, add vectors, implement additional scopes,
define privacy modes, build a memory editor, retain page versions, or change
the lifecycle of Mnemosyne. Its purpose is to define a small enough native
Markdown memory slice to evaluate through implementation.
