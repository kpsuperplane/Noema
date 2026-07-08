# Mnemosyne Memory Wiki Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the plain Mnemosyne memory list with a responsive Wikipedia-like memory article and wiki-native recap figure.

**Architecture:** Keep the current `memoryGraph` GraphQL boundary and implement this as a frontend-only page redesign. `MemoryPage.tsx` owns the query and page rendering; a new focused helper module converts generated GraphQL results into a small article view model so the JSX stays readable.

**Tech Stack:** React 19, Apollo Client, generated GraphQL types, StyleX, existing Vite/Bun web build.

## Global Constraints

- The browser must only query Noema Core GraphQL; it must not connect directly to Mnemosyne.
- Use the existing `MemoryGraphDocument`; do not rename GraphQL fields for this UI-only slice.
- Wikipedia structure, human voice. Formal enough to be funny, never dehumanizing.
- Lead figure spans the article width above the title.
- Infobox floats right on desktop and stacks on mobile.
- Contents floats left on desktop and stacks on mobile.
- Article text flows around both boxes like Wikipedia.
- Do not claim semantic certainty from brittle English phrase matching.
- When metadata is insufficient, render a generic `Memory entries` section.
- Do not add frontend unit tests unless explicitly requested by the user.

---

## File Structure

- Create `crates/noema-core/web/src/pages/memoryPageModel.ts`
  - Converts `MemoryGraphQuery["memoryGraph"]` into a display model.
  - Owns date formatting, entry flattening, generic section construction,
    count labels, and figure values.
- Modify `crates/noema-core/web/src/pages/MemoryPage.tsx`
  - Keeps the Apollo query and pagination.
  - Renders the wiki article, lead figure, infobox, contents, memory entries,
    recall sample, loading, empty, error, and load-more states.
- Modify `docs/context/current.md`
  - Record that `/memory` now presents Mnemosyne-backed memories as a
    wiki-style article surface.

---

### Task 1: Memory Page View Model

**Files:**
- Create: `crates/noema-core/web/src/pages/memoryPageModel.ts`

**Interfaces:**
- Consumes: `MemoryGraphQuery["memoryGraph"]` from `@/generated/graphql`.
- Produces:
  - `buildMemoryArticleModel(graph: MemoryGraphQuery["memoryGraph"] | undefined): MemoryArticleModel`
  - `MemoryArticleModel`
  - `MemoryArticleEntry`
  - `MemoryArticleSection`

- [ ] **Step 1: Create the model helper**

Create `crates/noema-core/web/src/pages/memoryPageModel.ts`:

```ts
import type { MemoryGraphQuery } from "@/generated/graphql";

type MemoryGraph = MemoryGraphQuery["memoryGraph"];
type MemoryGraphDocument = MemoryGraph["documents"][number];
type MemoryGraphEntry = MemoryGraphDocument["memoryEntries"][number];

export type MemoryArticleEntry = {
  id: string;
  text: string;
  createdAt: string | null;
  updatedAt: string | null;
  sourceTitle: string;
  scoreLabel: string | null;
};

export type MemoryArticleSection = {
  id: string;
  title: string;
  entries: MemoryArticleEntry[];
};

export type MemoryFigureCluster = {
  label: string;
  strength: number;
  status: string;
};

export type MemoryArticleModel = {
  title: string;
  subtitle: string;
  totalMemories: number;
  totalLabel: string;
  lastUpdatedLabel: string;
  primaryPattern: string;
  recurringMotif: string;
  leadText: string;
  figureTitle: string;
  figureCopy: string;
  figureCaption: string;
  clusters: MemoryFigureCluster[];
  sections: MemoryArticleSection[];
  recallSample: MemoryArticleEntry | null;
  references: string[];
};

const FALLBACK_TITLE = "Human memory";
const FALLBACK_SUBTITLE = "From Noema, the local personal memory record";

export function buildMemoryArticleModel(graph: MemoryGraph | undefined): MemoryArticleModel {
  const entries = flattenEntries(graph?.documents ?? []);
  const totalMemories = graph?.pageInfo.total ?? entries.length;
  const lastUpdatedLabel = formatLatestUpdated(entries);
  const primaryPattern = entries.length > 0 ? "Known memories" : "No stable pattern yet";
  const recurringMotif = entries.length > 0 ? "Memory entries" : "None yet";

  return {
    title: FALLBACK_TITLE,
    subtitle: FALLBACK_SUBTITLE,
    totalMemories,
    totalLabel: formatCount(totalMemories, "memory", "memories"),
    lastUpdatedLabel,
    primaryPattern,
    recurringMotif,
    leadText: buildLeadText(entries.length),
    figureTitle: buildFigureTitle(entries.length),
    figureCopy: buildFigureCopy(entries.length),
    figureCaption: "Fig. 1. Prominent themes in the memory record, grouped by loaded entries.",
    clusters: buildClusters(entries),
    sections: buildSections(entries),
    recallSample: entries[0] ?? null,
    references: buildReferences(entries)
  };
}

function flattenEntries(documents: MemoryGraphDocument[]): MemoryArticleEntry[] {
  return documents.flatMap((document) =>
    document.memoryEntries
      .map((entry) => memoryEntryFromGraph(document, entry))
      .filter((entry): entry is MemoryArticleEntry => entry !== null)
  );
}

function memoryEntryFromGraph(
  document: MemoryGraphDocument,
  entry: MemoryGraphEntry
): MemoryArticleEntry | null {
  const text = entry.content ?? entry.summary ?? entry.title;
  if (!text?.trim()) {
    return null;
  }
  return {
    id: entry.id,
    text: text.trim(),
    createdAt: entry.createdAt || null,
    updatedAt: entry.updatedAt || null,
    sourceTitle: document.title ?? "Memory source",
    scoreLabel: null
  };
}

function buildLeadText(entryCount: number): string {
  if (entryCount === 0) {
    return "Noema has not formed durable human memories yet. New user-authored observations will appear here after Mnemosyne processes them.";
  }
  return "This page collects user-authored observations Noema may use when responding to the local human. The current record is shown as an inspectable article rather than hidden model state.";
}

function buildFigureTitle(entryCount: number): string {
  if (entryCount === 0) {
    return "No memory themes have taken root yet.";
  }
  return "The memory record is beginning to take shape.";
}

function buildFigureCopy(entryCount: number): string {
  if (entryCount === 0) {
    return "Once memories exist, this figure summarizes loaded themes, recency, and retrieval visibility.";
  }
  return `${formatCount(entryCount, "loaded memory", "loaded memories")} are available for inspection. More specific themes will appear as Noema receives richer provenance and categorization metadata.`;
}

function buildClusters(entries: MemoryArticleEntry[]): MemoryFigureCluster[] {
  const loaded = entries.length;
  return [
    {
      label: "Loaded entries",
      strength: loaded > 0 ? 100 : 0,
      status: loaded > 0 ? "available" : "empty"
    },
    {
      label: "Stable roots",
      strength: loaded > 0 ? 62 : 0,
      status: "metadata pending"
    },
    {
      label: "Recent growth",
      strength: loaded > 0 ? 38 : 0,
      status: "metadata pending"
    },
    {
      label: "Open questions",
      strength: loaded > 0 ? 18 : 0,
      status: "review later"
    }
  ];
}

function buildSections(entries: MemoryArticleEntry[]): MemoryArticleSection[] {
  return [
    {
      id: "memory-entries",
      title: "Memory entries",
      entries
    }
  ];
}

function buildReferences(entries: MemoryArticleEntry[]): string[] {
  if (entries.length === 0) {
    return ["No source memories have been returned by Mnemosyne yet."];
  }
  const sourceTitles = [...new Set(entries.map((entry) => entry.sourceTitle))];
  return sourceTitles.map((title) => `Source group: ${title}.`);
}

function formatLatestUpdated(entries: MemoryArticleEntry[]): string {
  const latest = entries
    .map((entry) => entry.updatedAt ?? entry.createdAt)
    .filter((value): value is string => Boolean(value))
    .sort()
    .at(-1);
  if (!latest) {
    return "Not yet";
  }
  return formatDateTime(latest);
}

export function formatDateTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short"
  }).format(date);
}

export function formatCount(count: number, singular: string, plural: string): string {
  return `${count} ${count === 1 ? singular : plural}`;
}
```

- [ ] **Step 2: Validate the helper compiles**

Run: `cd crates/noema-core/web && bun run lint`

Expected: This may still fail until Task 2 uses the helper. Type errors in
`memoryPageModel.ts` must be fixed before continuing.

- [ ] **Step 3: Commit**

Do not commit Task 1 separately unless the helper compiles in isolation.

---

### Task 2: Render The Wiki Memory Page

**Files:**
- Modify: `crates/noema-core/web/src/pages/MemoryPage.tsx`

**Interfaces:**
- Consumes: `buildMemoryArticleModel(graph)` from Task 1.
- Produces: A responsive wiki-style `/memory` page using the existing
  `MemoryGraphDocument`.

- [ ] **Step 1: Replace the plain list with the article surface**

Modify `MemoryPage.tsx` so it:

- Imports `buildMemoryArticleModel` and `formatDateTime`.
- Builds `const article = buildMemoryArticleModel(graph);`.
- Keeps existing `useQuery`, pagination, loading, and service-error behavior.
- Renders:
  - Wiki tabs.
  - Lead figure.
  - Article title/subtitle.
  - Floating infobox.
  - Lead paragraph.
  - Contents box.
  - Generic memory entries section.
  - Recall behavior section.
  - References.
  - Load more button.

The JSX should use this structure:

```tsx
return (
  <section data-slot="memory-surface" {...stylex.props(styles.surface)}>
    <div {...stylex.props(styles.wikiShell)}>
      <nav {...stylex.props(styles.tabs)} aria-label="Memory article views">
        <span {...stylex.props(styles.tabActive)}>Article</span>
        <span {...stylex.props(styles.tab)}>Sources</span>
        <span {...stylex.props(styles.tab)}>History</span>
        <span {...stylex.props(styles.tabEnd)}>Search memory</span>
      </nav>
      <article {...stylex.props(styles.page)} aria-labelledby="memory-surface-title">
        {/* lead figure, title, infobox, contents, sections */}
      </article>
    </div>
  </section>
);
```

- [ ] **Step 2: Add the wiki-native StyleX styles**

Replace the old card-list styles with styles for:

- `surface`
- `wikiShell`
- `tabs`, `tab`, `tabActive`, `tabEnd`
- `page`
- `figure`, `memoryPlate`, `plateMain`, `plateLabel`, `plateTitle`,
  `plateCopy`, `legend`, `legendItem`, `swatch`, `clusterTable`, `bar`,
  `barFill`, `caption`
- `articleTitle`, `subtitle`, `infobox`, `infoTable`, `boxTitle`, `boxKey`,
  `clusterDisc`, `sidebox`
- `lead`, `contents`, `contentsTitle`, `articleSection`, `sectionTitle`,
  `editLink`, `entryList`, `entryItem`, `entryMeta`, `recallWidget`,
  `recallRow`, `recallInput`, `button`, `references`, `loadMore`

Use CSS floats for desktop infobox and contents. At `max-width: 760px`, clear
the floats and make both full-width. At `max-width: 560px`, reduce typography
and stack the recall row.

- [ ] **Step 3: Preserve states**

Ensure:

- Loading with no data shows an article-shaped status block.
- Unavailable/error memory service shows an article-shaped error block.
- Empty memory shows the wiki shell with empty figure and lead text.
- Load more still works when `pageInfo.hasMore` is true.

- [ ] **Step 4: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: Both pass.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/web/src/pages/MemoryPage.tsx crates/noema-core/web/src/pages/memoryPageModel.ts
git commit -m "feat: redesign memory page as wiki article"
```

---

### Task 3: Document Current Memory UI State

**Files:**
- Modify: `docs/context/current.md`

**Interfaces:**
- Consumes: The implemented Memory page behavior from Task 2.
- Produces: Durable project context for future agents.

- [ ] **Step 1: Update current context**

In `docs/context/current.md`, update the memory frontend bullet so it says:

```md
- The top-level `/memory` page presents Mnemosyne-backed human memories as a
  Wikipedia-like personal memory article: a muted lead figure summarizes loaded
  memory themes, the article body lists inspectable entries with provenance
  references, and inactive source/history/action affordances are reserved for
  future editing, pruning, export, and full source drill-downs.
```

- [ ] **Step 2: Run docs diff check**

Run: `git diff --check`

Expected: no output.

- [ ] **Step 3: Commit**

```bash
git add docs/context/current.md
git commit -m "docs: update memory page context"
```

---

### Task 4: Final Verification

**Files:**
- No source changes expected.

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: verified working tree.

- [ ] **Step 1: Run repository status**

Run: `git status --short --branch`

Expected: clean worktree except unrelated user files, if any.

- [ ] **Step 2: Run targeted validation**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: both pass.

- [ ] **Step 3: Run repo-level checks required before ship**

Run from repo root:

```bash
cargo fmt --all --check
git diff --check
```

Expected: both pass.

- [ ] **Step 4: Report completion**

Summarize commits, validation results, and any remaining unstaged or untracked
files.
