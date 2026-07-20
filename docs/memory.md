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
- Each file is written as a compact Wikipedia-style article: one generated
  title, a concise lead, coherent prose under distinct sections, inline
  footnotes, and collected source definitions. The root is the human's
  biographical overview; child files are focused topic articles.
- The canonical SQLite database stores the selected Memory model and source
  conversations. A separate SQLite FTS database under `system/indexes/` is a
  disposable projection rebuilt from Markdown.
- Every ordinary primary-conversation turn receives the bounded root page.
  `read_memory_page` and `search_memory` retrieve deeper pages on demand.
- Context compaction and the explicit Memory-page action schedule a separate,
  single-flight background update over the primary conversation's existing
  `sequence_index` order.
- Direct editing, page history, private memory, additional scopes, and vectors
  are not part of this slice.

Older broad memory proposals should be treated as historical target context only
when they agree with the current context and frontend contract above.
