# Native Markdown Memory Exploration

## Status

This document is an exploratory proposal, not an approved replacement for the
current Mnemosyne memory subsystem. It organizes an initial design direction so
that its tradeoffs and unresolved questions can be evaluated more clearly.

## Summary

Noema may be better served by a native memory layer built from a navigable tree
of Markdown documents. The root document would remain in the agent's context,
while links to progressively more specific child documents would let the agent
retrieve detail only when needed.

This structure aims to make memory:

- compact enough to provide useful ambient context;
- understandable and editable by the human;
- broad and inexpensive enough to maintain continuously;
- capable of distinguishing ordinary, private, and ephemeral information;
- traceable to the messages from which each memory was derived.

The proposal revisits native memory because the original graph-oriented design
was too ambitious and difficult for a human to inspect. Mnemosyne simplified
that problem and has worked well, but a Markdown-native system may fit Noema's
local-first, human-owned model more naturally.

## Requirements

### 1. Bounded ambient context and directed retrieval

The agent should always receive enough memory context to understand what it
knows and where to look next, without loading the full memory corpus into every
turn.

### 2. Human inspection and editing

The human should be able to browse, understand, correct, and back up memory
without needing to interpret a graph, vector index, or internal database
representation.

### 3. Coherent and efficient maintenance

The system should be able to incorporate a broad range of new information,
update existing beliefs, and reorganize memory without excessive runtime or
token use. Finding where a memory belongs is part of the same navigation
problem as retrieving it later.

### 4. Sensitivity-aware handling

Noema is designed around a continuous conversation, so it needs an explicit
way to distinguish information that may be remembered and reused freely from
information that requires stronger handling or must not persist at all.

## Proposed Storage Model

Memory would be stored as a tree of Markdown documents.

```text
root.md
├── people.md
│   ├── family.md
│   └── collaborators.md
├── preferences.md
│   ├── technical.md
│   └── personal.md
└── projects.md
    └── noema.md
```

The example structure is illustrative rather than prescribed. The memory
process should be able to evolve the tree as the human's information changes.

The structure would follow several invariants:

1. Memory documents form a tree with one root document.
2. Every parent document links to each of its immediate children.
3. The root document is compact enough to remain in the agent's context.
4. Every document has a word limit, tentatively around 750 words.
5. Memory claims cite the source messages that support them.
6. Sensitivity boundaries survive retrieval, rewriting, splitting, and
   compaction.

## Retrieval

The root document would be supplied to the agent on every turn. It would
summarize the highest-value memory and act as a table of contents for the rest
of the tree.

When the current task needs more detail, the agent can follow links from the
root into increasingly specific documents. This creates a Wikipedia-like
navigation model: the agent begins with a compact overview, recognizes which
branch may contain the answer, and retrieves only the relevant pages.

The same structure is directly visible to the human. There is no separate
machine representation that must be reconstructed into readable prose before
the human can inspect it.

## Memory Updates

### Update triggers

Context compaction is the primary proposed trigger for background memory
maintenance. At each compaction boundary, the memory process would examine the
messages since the previous boundary and update the relevant documents.

Long intervals could be processed in batches or overlapping windows. Other
durable boundaries, such as the completion of a task run, could trigger the
same process when useful.

### Update flow

The background process would:

1. Read the new source messages.
2. Navigate the existing memory tree to find the relevant documents.
3. Reconcile the new information with existing claims.
4. Update, remove, or relocate content as needed.
5. Preserve citations and sensitivity labels.
6. Enforce the document size and tree-linking invariants.

A well-prompted model should be capable of editing Markdown coherently, but the
structural and sensitivity invariants should be enforced by the surrounding
system rather than entrusted solely to the model.

## Revision, Forgetting, and Tree Growth

### Revising beliefs

New information may supersede an existing memory. For example, a prior memory
that the human is afraid of heights should be revised if the human later says
that skydiving changed that preference. During an update, the model would
retrieve the relevant document, compare the old and new evidence, and rewrite
the claim rather than accumulating contradictory facts indefinitely.

### Forgetting and document limits

Each document would have a fixed word limit. When an update would exceed that
limit, the memory process must choose among three actions:

- remove information that is no longer useful;
- compress existing material;
- move still-useful detail into a new or existing child document.

This turns document size into a recurring decision point. Low-value details can
fade, while important but less immediate information can move deeper into the
tree. The corpus may grow without forcing every memory into active context.

The exact forgetting policy remains unresolved. A model may be good at making
these editorial decisions, but Noema still needs to define which information it
must never discard automatically and how a human can recover or audit changes.

## Provenance and Source Deletion

Memory claims should use citations to the source conversation items that
support them. Citations provide a readable provenance model and allow the human
to move from a synthesized claim back to its evidence.

Deleting a source message should signal that Noema must reconsider memories
derived from it. The system could find claims carrying that citation and remove
or regenerate them from any remaining sources. This requires clear semantics
for claims supported by multiple messages and for documents that have been
rewritten since the original extraction.

## Sensitivity Model

### Memory documents

The simpler proposed approach is to classify whole documents as either open or
private. Private documents would be excluded from ordinary retrieval and made
available only through a defined relevance or authorization gate.

An alternative is inline redaction, where individual passages remain hidden
until a verifier decides they are relevant. This would allow mixed-sensitivity
documents, but it introduces substantially more machinery and makes editing,
splitting, and citation handling harder to reason about.

Document-level sensitivity appears easier to maintain, but the proposal does
not yet define what unlocks a private document or what the model is allowed to
learn about hidden documents during retrieval.

### Chat modes

The user-facing model would expose three modes within a conversation:

| Mode | Persistence | Memory behavior | Context behavior |
| --- | --- | --- | --- |
| Open | Messages persist normally. | Messages may update open memory. | Messages remain available to ordinary conversation context and compaction. |
| Private | Messages remain visible and persisted. | Messages may update private memory, isolated from the open-memory update path. | The private segment is removed from active model context when the human exits the mode. |
| Ghost | Messages are ephemeral. | No memories are created. | The segment is removed from context and deleted from durable storage when the human exits the mode. |

Messages should retain their mode individually so the boundary remains visible
and deterministic after later mode changes.

### Mode transitions and compaction

Leaving Ghost mode would discard the ghost segment from both durable storage
and active context. Leaving Private mode would keep the messages visible to the
human, remove the segment from subsequent ordinary model context, and trigger a
private-memory update.

At context compaction, Noema could partition unprocessed messages by mode. Open
messages would feed the open-memory update, while private messages would feed a
separate private-memory update. Ghost messages would never reach either path.

This design may invalidate part of the model's prompt cache when a segment is
truncated, but truncating the tail may keep that cost manageable.

## Sensitivity Assistance

A lightweight classifier could inspect each new human message for potentially
sensitive content. When it detects a likely private message, Noema could ask
whether the human wants to switch to Private mode and retroactively mark that
message as the first message in the private segment.

The classifier would be advisory. The mode selected by the human, together with
deterministic storage and context rules, would remain authoritative.

## Open Questions

- What exactly authorizes retrieval of a private memory document?
- Should sensitivity apply only at the document level, or is inline redaction
  worth the added complexity?
- Which memories may be compressed or forgotten automatically, and which must
  be retained until the human removes them?
- How should citations behave when several sources support one claim or when a
  source message is deleted after several rewrites?
- Should document histories be retained so the human can inspect or reverse
  automatic memory changes?
- Is context compaction sufficient as the primary update boundary, or do chat
  volume and task completion require additional triggers?
- How should the system recover when an update fails partway through a tree
  rewrite?
- Which classifier, if any, can suggest Private mode cheaply without producing
  disruptive false positives?
- How would existing Mnemosyne memory be imported, validated, or retired if
  this proposal replaces the sidecar?

## Scope of This Exploration

This proposal defines a conceptual storage and interaction model. It does not
yet define the on-disk transaction format, model prompts, retrieval tools,
private-memory authorization policy, migration path, or implementation plan.
