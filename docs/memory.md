# Memory Plan Index

This file is an index for the current memory direction, not a standalone
architecture plan.

Current authorities:

- [Project overview](project.md): product-level memory ownership and source of
  truth rules.
- [Current context](context/current.md): latest implemented memory state and
  open loops.
- [Frontend contract](frontend/current-contract.md): current memory settings
  route and first-slice visibility boundaries.
- [Harness memory/context integration](harness/memory-context.md): historical
  target context for runtime memory integration.

Current implementation direction:

- One Markdown tree under `memory/human/` is the durable memory authority for
  `human:local`; page frontmatter owns semantic metadata and the filesystem
  hierarchy owns parent-child structure.
- Page frontmatter stores a model-chosen Lucide `icon` key alongside the title.
  New and updated pages require a supported key; legacy iconless pages remain
  unchanged and read as `user` for `root.md` or `file-text` for child pages.
- Each file is written as a compact Wikipedia-style article: one generated
  title, a concise lead, coherent prose under distinct sections, inline
  footnotes, and collected source definitions. The root is the human's
  biographical overview; child files are focused topic articles. A developed
  root must contain at least two thematic sections and cannot publish as an
  unsectioned fact inventory.
- The update model supplies ordered source identifiers and matching numeric
  references. The runtime validates those identifiers, generates every
  footnote definition, and derives the stored source manifest from that list.
- Article citations render as superscript reference numbers whose hover/focus
  cards resolve to a bounded excerpt of the cited human message. Raw item ids
  remain implementation detail and there is no separate references appendix.
- Filesystem-derived child pages appear as compact Related Articles cards using
  their lead excerpts. Each card is a real link to the filesystem-derived
  `/memory/<article-path>` route; stable ids and relative paths remain valid API
  selectors, including generated ids that end in `.md`.
- Child pages expose their ordered filesystem ancestors so the shell title row
  can render a deeply nested, client-side breadcrumb using canonical titles.
- The canonical SQLite database stores the selected Memory model and source
  conversations. A separate SQLite FTS database under `system/indexes/` is a
  disposable projection rebuilt from Markdown.
- Every ordinary primary-conversation turn receives the bounded root page.
  `read_memory_page` and `search_memory` retrieve deeper pages on demand.
- Context compaction and the explicit Memory-page action schedule a separate,
  single-flight background update over the primary conversation's existing
  `sequence_index` order. The model receives every page body while the lean tree
  fits; beyond that point it receives a whole-tree catalog plus the root,
  FTS-ranked relevant pages, and their ancestors. Catalog-only page content
  cannot be mutated, while icon metadata and new pages remain available.
- The normal update response can use `metadata_updates` to change an existing
  page icon without reproducing its article body or provenance. The server
  merges each validated patch against the canonical page snapshot before the
  same atomic publication and corrective-retry path.
- Source arrivals and update transitions publish runtime invalidations; GraphQL
  subscriptions refill the authoritative tree/status snapshot without polling.
- A structurally invalid model response receives one corrective retry; a second
  invalid response fails the update without advancing the checkpoint.
- Direct editing, page history, private memory, additional scopes, and vectors
  are not part of this slice.

Canonical page frontmatter keeps the existing schema version:

```yaml
schema: noema.memory.page/v1
title: Career
icon: briefcase-business
```

Older broad memory proposals should be treated as historical target context only
when they agree with the current context and frontend contract above.
