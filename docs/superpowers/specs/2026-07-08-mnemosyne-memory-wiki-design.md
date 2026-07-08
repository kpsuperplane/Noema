# Mnemosyne Memory Wiki Design

## Summary

Replace the current plain Mnemosyne memory list with a playful but inspectable
memory page. The page should feel like a personal Wikipedia article with one
polished, wiki-native memory figure at the top. The structure should be funny in
its seriousness, but the content must remain human, readable, and trustworthy.

Final design rule:

> Wikipedia structure, human voice. Formal enough to be funny, never dehumanizing.

## Product Goals

- Make memory fun enough that people want to inspect it.
- Keep the actual memory contents highly readable and provenance-oriented.
- Avoid graph-canvas UI unless Noema later exposes real entity/triple data.
- Preserve the current Noema Core GraphQL boundary; the browser never talks
  directly to Mnemosyne.
- Make the page responsive enough for mobile and narrow desktop sidebars.

## Page Shape

The `/memory` page becomes a single article-style surface:

1. Wiki tabs
   - `Article`
   - `Sources`
   - `History`
   - `Search memory`
   - Only `Article` needs to be interactive in the first implementation. The
     others may be disabled or visually present as inactive labels
     if that better preserves the Wikipedia feel.

2. Lead figure
   - A full-width figure above the article title.
   - Light, desaturated, and table/diagram-like rather than Spotify-dark.
   - Captioned as a figure, for example:
     `Fig. 1. Prominent themes in Kevin's memory record, grouped by retrieval strength.`
   - Shows:
     - Primary memory pattern.
     - Short human-language summary.
     - Cluster table with strength bars.
     - Muted legend for stable roots, recent growth, interests, and open
       questions.

3. Article title and subtitle
   - Title should be the human display name when available, falling back to
     `Human memory`.
   - Subtitle: `From Noema, the local personal memory record`.

4. Floating infobox
   - Floats right on desktop, stacks on mobile.
   - Contains topic distribution and key facts:
     - Primary pattern.
     - Known memories.
     - Stable roots.
     - Fresh shoots.
     - Recurring motif.
     - Last updated.
   - Includes action links:
     - Tune recall.
     - Prune memory.
     - Export memory.
     - Open source conversations.
   - Actions can be disabled or stubbed if their backing APIs are not ready.

5. Floating contents box
   - Floats left on desktop after the lead paragraph, stacks on mobile.
   - Uses Wikipedia-like section numbering and blue links.

6. Article body
   - Reads like a biographical memory entry about the human, not Noema product
     documentation.
   - Initial sections:
     - Technical preferences.
     - Interface taste.
     - Working style.
     - Interests.
     - Recall behavior.
     - References.
   - Section headings include `[edit]` affordances, but editing may be
     non-functional in the first implementation.

7. Recall behavior widget
   - Embedded in the article body.
   - Lets the user test what Mnemosyne would recall for a query.
   - Shows top result and score.
   - Should use Noema Core GraphQL, backed by the existing Mnemosyne search path.

8. References
   - Lists source/provenance summaries.
   - The first implementation can use existing Noema provenance metadata from
     Mnemosyne rows when available, and generic source labels when it is not.

## Content Voice

Use human-language phrasing:

- Good: `Kevin tends to return to local-first systems.`
- Good: `Other recurring themes include agent workflow, compact interface design, build speed, and aviation.`
- Bad: `The subject's dominant technical preference...`

The page may be gently formal, but never clinical or dehumanizing.

## Data Model For First Slice

Use the current `memoryGraph` query as the first implementation boundary. Do
not rename GraphQL fields for this UI-only slice.

The backend currently adapts Mnemosyne memories into documents and entries. The
new UI should consume those entries and derive a display model:

- Human title:
  - Use the local human display name if GraphQL exposes it.
  - Otherwise fall back to `Human memory`.
- Counts:
  - Total entries from `pageInfo.total` when present or loaded entry count.
- Memory sections:
  - Use stable metadata when available.
  - When metadata is insufficient, render a generic `Memory entries` section
    rather than pretending Noema knows a semantic category.
  - Keep grouping conservative and transparent. Do not claim semantic certainty
    from brittle English phrase matching.
- Cluster figure:
  - Use entry counts and, when available, recall/search scores.
  - If there is not enough data, show a restrained empty/low-data state.
- Provenance:
  - Use `noemaConversationId`, `turnId`, `userItemId`, and source metadata when
    present.
  - Do not expose raw Mnemosyne internals or sidecar URLs.

## Responsive Behavior

Desktop:

- Lead figure spans the article width above the title.
- Infobox floats right.
- Contents floats left.
- Article text flows around both boxes like Wikipedia.

Tablet:

- Lead figure remains full-width.
- Infobox and contents may stack above the article body if there is not enough
  width for comfortable wrapping.

Mobile:

- Tabs scroll horizontally.
- Lead figure stacks internally.
- Infobox, contents, article, recall widget, and references are single-column.
- Text remains readable without horizontal scrolling.

## Out Of Scope

- Direct browser connection to Mnemosyne.
- Full memory editing.
- Deletion/pruning API implementation.
- Export implementation.
- Source/history tabs with complete interaction.
- Graph visualization.
- Automatic pre-turn recall changes.

## Testing

- Frontend unit tests are not required for this UI slice unless existing
  patterns make them cheap.
- Run frontend generation/build checks required by the repo for web changes.
- Verify the page manually at desktop and mobile widths.
- Backend tests are needed only if GraphQL shape or Mnemosyne client behavior
  changes.
