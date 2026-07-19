# Native Markdown Memory Exploration

## Status

This is a durable exploration document for iteration. It is not yet an
approved replacement for Mnemosyne or the current project storage contract.
Decisions recorded here are working decisions for the proposal; they become
authoritative only after the proposal is approved and the relevant product and
architecture documents are updated.

## Verdict

The core design is practical. A bounded Markdown tree can provide ambient
memory, directed retrieval, human editing, and understandable provenance
without rebuilding the graph-oriented system that Noema deliberately moved
away from.

The design is strongest when Markdown owns both prose and semantic metadata,
while SQLite is a disposable search index. Context compaction should be the
normal consolidation boundary because it already identifies a coherent batch
of messages whose verbatim form is about to leave active context. It should not
be the only trigger or the transaction boundary for memory writes.

## Working Decisions

These choices currently shape the proposal:

1. **Markdown is canonical.** Memory content, stable identifiers, ownership,
   scope, parentage, sensitivity, provenance, and consolidation checkpoints
   live in Markdown files.
2. **SQLite is derived infrastructure.** SQLite provides full-text search and
   may cache parsed fields needed to filter results, but deleting the database
   and rescanning the Markdown corpus must restore the system.
3. **Memory is a forest.** Each governable scope, such as a human, project, or
   workspace, owns a separate memory tree with its own root and policy.
4. **Compaction is the primary consolidation trigger.** A successful context
   compaction schedules a memory update over the corresponding unprocessed
   source range. Explicit memory requests, task completion, and recovery are
   secondary triggers.
5. **Retrieval combines navigation and search.** Root documents provide
   ambient context and links, while read and search tools recover information
   when the correct branch is not obvious.
6. **Models propose; deterministic code commits.** A model can decide how to
   reconcile and organize prose, but code validates paths, hashes, links,
   citations, budgets, scope boundaries, and sensitivity before publishing an
   update.

## Goals

The memory system should be:

- compact enough to provide useful ambient context on every relevant turn;
- readable, editable, portable, and recoverable without a database browser;
- broad and inexpensive enough to maintain continuously;
- scoped so unrelated projects and principals do not share an undifferentiated
  global memory;
- able to distinguish open, private, and ephemeral conversation;
- traceable to the conversation items that support each synthesized claim;
- resilient to interrupted updates, source deletion, and external file edits.

## Why This Is Feasible

Several existing systems validate the main shape of the design:

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

Noema already has useful seams for an incremental implementation:

- `crates/noema-memory/src/operations.rs` centralizes the current add, search,
  and list operations.
- `crates/noema-runtime/src/daemon/runtime/context_compaction.rs` already
  identifies compaction boundaries and durable source ranges.
- `crates/noema-runtime/src/daemon/runtime/turn/provider_request.rs` persists
  conversation items before asynchronous observation work.
- `crates/noema-runtime/src/daemon/runtime/model_context.rs` assembles explicit
  model-context sections where active memory roots can be injected.
- `crates/noema-runtime/src/daemon/runtime/artifact_writes.rs` already contains
  hash and version concepts that can inform safe file updates.

The proposal does depart from the current general storage contract, which puts
semantic metadata and relationships in SQLite. For memory specifically, that
metadata would move into Markdown frontmatter so a copied tree remains
self-describing. That exception needs an explicit architecture decision before
implementation.

## Storage Model

### A forest of scoped trees

Memory is organized as one tree per governable scope, not one global tree.
For example:

```text
memory/
├── humans/
│   └── human_local/
│       ├── root.md
│       ├── preferences.md
│       └── people/
│           └── collaborators.md
├── projects/
│   └── project_noema/
│       ├── root.md
│       ├── product.md
│       └── architecture/
│           └── memory.md
└── workspaces/
    └── workspace_example/
        └── root.md
```

The active human root and active scope roots are eligible for ambient context.
Activation must come from explicit product state, such as the current project
or workspace, rather than guessing scope from English phrases in a message.
Private roots are injected only when their authorization rule succeeds.

Ownership and scope are separate concepts. A project tree can be owned by the
human while still having project-local content and policies. Cross-scope facts
should normally live in the narrowest authoritative tree and be linked from
other trees rather than copied into several places.

### Canonical Markdown metadata

Every page carries machine-readable frontmatter. A tentative page format is:

```markdown
---
schema: noema.memory.page/v1
id: memory_page:project_noema:architecture
owner:
  type: human
  id: human:local
scope:
  type: project
  id: project:noema
parent: memory_page:project_noema:root
sensitivity: open
created_at: 2026-07-19T12:00:00-07:00
updated_at: 2026-07-19T12:00:00-07:00
sources:
  - conversation_item:item_123
---

# Architecture

Noema favors human-readable durable state.[^item-123]

[^item-123]: `conversation_item:item_123`
```

Frontmatter is authoritative for page identity, ownership, scope, parentage,
sensitivity, timestamps, and the page-level source manifest. Footnotes attach
provenance to specific claims. The source manifest can be validated against
the citations in the body.

A tree also owns a small machine-managed Markdown state file, excluded from
retrieval, that records the last source item consolidated for each memory mode.
Keeping this checkpoint outside SQLite means deleting the search index cannot
cause the updater to forget its position.

### Tree invariants

1. Every tree has exactly one root.
2. Every non-root page names one parent, and every parent links to its direct
   children.
3. Stable IDs survive file moves and title changes.
4. A page belongs to exactly one scope and sensitivity class.
5. Root pages obey a tighter context budget than topic pages.
6. Claims cite durable source item IDs.
7. A page and its descendants cannot silently cross ownership or sensitivity
   boundaries.

The earlier 750-word limit is useful as an editorial target, but a deterministic
character or token budget is safer for multilingual content and prompt
assembly. Exceeding a budget triggers compression, pruning, or a child-page
split; it does not automatically justify deleting useful information.

## Derived SQLite Search

SQLite stores only data that can be rebuilt from the Markdown forest:

- FTS-indexed headings and body text;
- page path and stable page ID;
- duplicated scope, owner, and sensitivity fields needed to filter candidates;
- parsed outbound links and citations when they make lookup faster;
- file content hashes and indexing diagnostics.

The duplicated fields remain caches, not authority. Search results are checked
against the current frontmatter before content is returned, which prevents a
stale index from bypassing a scope or sensitivity boundary.

The first implementation should use lexical FTS only. Embeddings can be added
later as another rebuildable index if measured retrieval failures justify their
cost and complexity.

## Retrieval

The runtime supplies the human root and the roots of explicitly active scopes
as bounded context sections. Each root summarizes durable high-value knowledge
and acts as a table of contents for its tree.

Two tools complete the retrieval path:

1. **Read a memory page.** The model follows a page ID or link when a root
   points toward a likely branch.
2. **Search memory.** FTS searches only authorized active scopes and returns
   ranked snippets plus page IDs when tree navigation is uncertain.

Links alone are too brittle because the root cannot anticipate every future
wording of a query. Search alone loses the useful hierarchy and ambient
orientation. Using both keeps ordinary retrieval cheap while preserving a
fallback.

Full memory payloads should not be copied permanently into every conversation
transcript. Tool results can persist page references and content hashes while
the resolved text remains replay-ephemeral. This makes later correction or
deletion of memory meaningful instead of leaving old copies embedded in every
turn.

## Consolidation Lifecycle

### Why context compaction is the primary trigger

Context compaction is the natural default boundary for four reasons:

1. It identifies a coherent batch that the model has just processed.
2. The batch's verbatim messages are about to leave active model context, even
   though their durable conversation items remain available.
3. Consolidating once per compaction amortizes model work instead of extracting
   memory from every message.
4. The same bounded source selection can feed the conversation summary and the
   memory updater, avoiding duplicated transcript assembly.

Memory maintenance should run after or alongside compaction, not inside its
success transaction. A failed memory update must not block the conversation or
invalidate a valid context summary. The checkpoint advances only after all
validated Markdown changes are published, so a failure simply causes the same
source range to be retried.

Compaction cannot be the exclusive trigger. It is provider- and model-specific,
large-context conversations may run for a long time without it, and another
task may need a new memory before the next compaction. Secondary triggers are:

- an explicit request to remember something;
- completion of a task or other durable scope boundary;
- an idle or volume threshold for unusually long intervals;
- startup recovery when persisted source items are newer than the checkpoint.

The design should initially share the source batch while keeping conversation
summary generation and memory editing as separately validated outputs. A later
experiment can ask one model call to produce both outputs. Combining them would
save model work, but it should be adopted only if memory quality remains good
and it does not couple foreground compaction latency to multi-page retrieval
and editing.

### Update flow

For each affected scope, the updater:

1. Reads durable source items after the tree's checkpoint.
2. Applies the deterministic conversation-mode and scope policy.
3. Loads the tree root, then follows links or searches for relevant pages.
4. Asks the model for a structured change set with expected page hashes,
   citations, and any proposed splits or moves.
5. Validates IDs, frontmatter, paths, links, budgets, citations, sensitivity,
   and expected hashes.
6. Acquires a per-tree write lock and stages the file changes.
7. Publishes child pages before parent links, then writes the checkpoint last.
8. Refreshes the derived search index from the committed Markdown.

Publishing the checkpoint last gives the process at-least-once behavior. A
crash may repeat a batch but cannot mark uncommitted memory as complete.
Expected hashes prevent a background update from overwriting a concurrent human
edit, and stable citations let retries reconcile content instead of duplicating
it blindly.

## Revision, History, and Source Deletion

New evidence should rewrite a belief rather than append contradictions
indefinitely. When a page grows past its budget, the updater chooses among
compression, removal of stale low-value material, and moving detail into a
child page. Policy must distinguish editorial compression from facts that the
human has pinned against automatic removal.

Source deletion is a regeneration problem, not a string-deletion problem. The
system finds pages citing the deleted item, rebuilds the affected claims from
remaining cited sources, and removes a claim only when it is no longer
supported. The FTS index can accelerate the reverse lookup, but Markdown
citations remain the durable evidence.

Optional page history should also remain file-native, for example as immutable
snapshots under a hidden tree-local history directory. Whether that history is
enabled by default depends on the expected storage volume and the semantics of
deleting sensitive source material.

External edits are expected because human inspectability is a core goal. A file
watcher should validate and reindex valid changes. An invalid edit should remain
on disk with a visible diagnostic and be excluded from model retrieval until
fixed; the system should not silently overwrite the human's file.

## Sensitivity and Conversation Modes

Whole-page sensitivity is the proposed first implementation. It keeps
retrieval, splitting, editing, and auditing understandable. Inline redaction
would allow mixed-sensitivity pages, but it introduces a second content model
and should wait for a demonstrated need.

| Mode | Durable messages | Memory behavior | Later ordinary context |
| --- | --- | --- | --- |
| Open | Persisted | May update open scoped trees | Available normally |
| Private | Persisted | May update separately authorized private trees | Removed after leaving Private mode |
| Ghost | Deleted on exit | Never updates memory | Removed after leaving Ghost mode |

Each source item carries its mode, making later partitioning deterministic.
Open and private ranges use separate checkpoints and consolidation jobs. Ghost
items never enter either memory path.

A lightweight classifier may suggest switching to Private mode, but it cannot
authorize persistence or retrieval. Human-selected mode and deterministic
runtime policy remain authoritative. Ghost mode is a broader runtime and
storage feature, so it should not be smuggled into the first Markdown-memory
slice merely because the concepts interact.

## Proposed Implementation Slices

1. **Offline evaluation.** Run candidate prompts against representative saved
   conversations and measure useful recall, unsupported claims, citation
   accuracy, page churn, and root growth before replacing Mnemosyne.
2. **Open human and project memory.** Add Markdown parsing and validation,
   scoped roots, FTS indexing, root context injection, read/search tools, and a
   compaction-centered updater with durable checkpoints.
3. **Human editing and repair.** Add file watching, conflict diagnostics,
   history, source-deletion regeneration, and explicit pin/forget controls.
4. **Private scoped trees.** Define the authorization contract, isolate search
   and context injection, and add mode-aware consolidation.
5. **Ghost runtime behavior.** Implement ephemeral storage and context removal
   as its own end-to-end privacy feature.

The first shippable slice should avoid dual-running two authoritative memory
systems indefinitely. The offline evaluation can read existing Mnemosyne data,
but a production cutover needs a clear import or retirement decision.

## Questions to Resolve Next

The highest-leverage product and architecture questions are:

1. **Scope activation:** Which explicit product state activates a human,
   project, or workspace tree for a turn, and can more than one project tree be
   active at once?
2. **Ownership:** Is every tree initially owned by the local human, or must the
   first version represent shared/team ownership and permissions?
3. **Cross-scope knowledge:** When a fact applies to both a human and a project,
   should one tree link to the other, or should the updater maintain a deliberate
   summarized copy?
4. **Compaction output:** Should the first prototype use one model call for both
   context summary and memory changes, or separate calls over the same source
   batch so their quality and failure behavior can be evaluated independently?
5. **Human editing:** Is filesystem editing sufficient for the first slice, or
   is an in-product memory browser/editor part of the minimum useful experience?
6. **Retention:** Which claims can the updater compress or forget, which can the
   human pin, and how long should page history survive source deletion?
7. **Private authorization:** What explicit action permits a private root or
   page to enter model context, and what metadata about hidden pages may open
   search reveal?
8. **Cutover:** Should existing Mnemosyne memories be imported as cited draft
   pages, retained as a read-only archive, or discarded after evaluation?

## Non-goals of This Exploration

This document does not approve a storage-contract exception, select final
prompts or models, define private-memory authorization, or authorize removal of
Mnemosyne. Its purpose is to make the candidate architecture concrete enough
to evaluate and turn into an implementation proposal.
