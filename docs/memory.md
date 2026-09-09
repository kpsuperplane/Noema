# Memory Contract Index

This file is an index for the current memory direction, not a standalone
architecture plan.

Current authorities:

- [Project overview](project.md): product-level memory ownership and source of
  truth rules.
- [Current context](context/current.md): latest implemented memory state and
  open loops.
- [Frontend contract](frontend/current-contract.md): current memory settings
  route and visibility boundaries.

Current implementation direction:

- One Markdown tree under `memory/human/` is the durable memory authority for
  `human:local`; page frontmatter owns semantic metadata, generated footnotes
  own evidence groups, and the filesystem hierarchy owns parent-child structure.
- Page frontmatter stores a model-chosen Lucide `icon` key alongside the title.
  Every page requires a supported key.
- Each file is written as a compact Wikipedia-style article: one generated
  title, a concise lead, coherent prose under distinct sections, inline
  footnotes, and collected source definitions. The root is the human's
  biographical overview; child files are focused topic articles. A developed
  root must contain at least two thematic sections and cannot publish as an
  unsectioned fact inventory.
- Keep facts in the current article until the content would exceed the 750-word
  limit, including its title and generated footnotes. First remove repetition
  and combine related claims. Then move a coherent section into a child article
  and keep an overview in the parent. Apply this rule at every depth.
  A new topic alone does not require a file. Merge small child articles into
  their parent when the combined article fits. Preserve facts and evidence.
  The 650-word body target reserves space; it is not a split threshold.
- The update model supplies ordered citation groups and matching numeric
  references. Each group contains the smallest direct set of exact evidence
  identifiers for one nearby claim. The runtime validates those identifiers
  and generates every footnote definition.
- Human messages and exact saved tool results can provide evidence. Assistant
  messages remain context and cannot become evidence.
- Current connector state, tool counts, temporary failures, task history,
  project validation, and external research stay with their live objects,
  Tasks, projects, documents, or artifacts. Human memory stores stable human
  facts, preferences, relationships, and durable decisions.
- Article citations render as superscript reference numbers. Each hover or
  focus card shows the evidence type, date, bounded excerpt, and exact source
  identifier for every source in that citation group. There is no separate
  references appendix.
- Filesystem-derived child pages appear as compact Related Articles cards using
  their lead excerpts. Each card is a real link to the filesystem-derived
  `/memory/<article-path>` route; stable ids and relative paths remain valid API
  selectors, including generated ids that end in `.md`.
- Child pages expose their ordered filesystem ancestors so the shell title row
  can render a deeply nested, client-side breadcrumb using source titles.
- SQLite stores the selected Memory model and source
  conversations. Search uses a lexical index in process memory. The server
  rebuilds it from Markdown at startup and after publication.
- Every ordinary primary-conversation turn receives the bounded root page.
  `read_memory_page` and `search_memory` retrieve deeper pages on demand.
- Context compaction and the explicit Memory-page action schedule a separate,
  single-flight background update over the primary conversation's existing
  `sequence_index` order. The model receives every page body while the lean tree
  fits; beyond that point it receives a whole-tree catalog plus the root,
  pages ranked by matching terms, and their ancestors. Catalog-only page content
  cannot be mutated, while icon metadata and new pages remain available.
- The normal update response can use `metadata_updates` to change an existing
  page icon without reproducing its article body or provenance. The server
  merges each validated patch against the source page snapshot before the
  same atomic publication and corrective-retry path.
- Source arrivals and update transitions publish runtime invalidations; GraphQL
  subscriptions refill the authoritative tree/status snapshot without polling.
- A structurally invalid model response receives one corrective retry; a second
  invalid response fails the update without advancing the checkpoint.
- Direct editing, page history, private memory, additional scopes, and vectors
  are not implemented.

Source page frontmatter uses schema version 2:

```yaml
schema: noema.memory.page/v2
title: Career
icon: briefcase-business
```

Generated evidence definitions follow the article. One marker can have several
exact sources:

```markdown
Kevin prefers default reminders. [^1]

[^1]: item:human-message item:tool-result
```

Git history preserves older memory proposals. The current authorities above
define implemented behavior.
