# Chat Transcript Pagination And Virtualization Design

## Goal

Make the web chat transcript efficient for long-running conversations by loading
only the newest durable transcript window at startup, fetching older history
transparently as the user scrolls upward, and rendering loaded rows through a
virtualized list. This slice does not evict loaded transcript pages from memory;
eviction is a future policy layer after pagination and virtualization are stable.

## Context

Noema's home chat is a single durable primary conversation for `human:local`.
Durable history is reconstructed from SurrealDB-backed `conversation_items`,
while live turn updates arrive through the `conversationEvents` GraphQL
subscription. The current web UI calls `startPrimaryConversation`, receives an
unbounded `replay` array, stores it in React state, transforms it into renderable
transcript rows, and renders every row into the DOM.

That model is not viable once a transcript may span months or years. The
existing store already records a monotonic `sequence_index` for each
conversation item and indexes `(conversation_id, sequence_index)`, making it the
right internal source for durable transcript pagination cursors.

## Non-Goals

- Do not implement transcript page eviction in this slice.
- Do not migrate the app to TanStack Router, TanStack Query, or a broader
  TanStack application stack.
- Do not redesign transcript visuals.
- Do not change provider context assembly or memory compaction behavior.
- Do not keep compatibility wrappers for pre-V1 GraphQL operation names that
  this design replaces.

## Architecture

Replace `startPrimaryConversation` with a clean conversation-state and
transcript-history split.

`primaryConversation` is a read-only query for durable primary conversation
identity and chat readiness metadata. It returns the primary `conversationId`
and any small provider or model fields the shell needs, but it does not return
transcript items and it does not create state.

`ensurePrimaryConversation` is the explicit write operation for the rare path
where the local human has no primary conversation yet. It creates or attaches the
primary conversation and may create the initial onboarding assistant item for an
empty conversation. Normal chat boot should not call this mutation when
`primaryConversation` already returns a conversation.

`conversationTranscriptPage` is the only GraphQL replay read for transcript
items. It accepts a conversation id, a nullable cursor, and a bounded limit. A
null cursor loads the newest visible items. A non-null cursor loads items before
the current oldest loaded cursor position. Results are returned in ascending
transcript order so the frontend can merge pages directly into its normal
append-order model.

`conversationEvents` remains the live source for current turn updates. Durable
conversation item events include an opaque item `cursor` so live appends can
dedupe and align correctly against paged replay without exposing the store's
raw append-order index to clients.

The frontend keeps Apollo for GraphQL queries, mutations, subscriptions, codegen,
and cache transport. Add `@tanstack/react-virtual` only for transcript row
virtualization. TanStack Virtual is headless, so Noema keeps its Astryx and
StyleX transcript components while delegating dynamic row measurement, visible
range calculation, and prepend/bottom anchoring mechanics to the virtualizer.

## Backend Contract

Add a read-only primary conversation result:

```graphql
type PrimaryConversation {
  conversationId: String!
  provider: String!
}
```

Expose it as:

```graphql
type Query {
  primaryConversation: PrimaryConversation
}

type Mutation {
  ensurePrimaryConversation(cwd: String): PrimaryConversation!
}
```

Add `cursor: String!` to `ConversationItem` and durable `ConversationItemEvent`.
The first cursor implementation may encode the store's `sequence_index`, but
clients must treat the value as opaque.

Add a transcript page result:

```graphql
type ConversationTranscriptPage {
  items: [ConversationItem!]!
  pageInfo: ConversationTranscriptPageInfo!
}

type ConversationTranscriptPageInfo {
  beforeCursor: String
  hasMoreBefore: Boolean!
  limit: Int!
}
```

Add an input shape equivalent to:

```graphql
input ConversationTranscriptPageInput {
  conversationId: String!
  cursor: String
  limit: Int
}
```

The exact GraphQL spelling can be refined during implementation, but the
semantics are fixed:

- `cursor: null` fetches the newest visible items with `ORDER BY sequence_index DESC`
  internally, then reverse them before returning.
- non-null cursor reads fetch visible items before the decoded cursor position,
  also returning ascending order.
- clients must pass cursors back unchanged rather than parse them.
- `pageInfo.beforeCursor` is the cursor to pass when fetching the next older
  page; it is null when the returned page has no items.
- limits are clamped server-side to a small safe range.
- `hasMoreBefore` is computed by fetching one extra row or an equivalent bounded
  existence check.
- all reads use `ReplayMode::Visible`; audit replay remains separate.

## Frontend Data Flow

On chat boot:

1. Query `primaryConversation`.
2. If no primary conversation exists and onboarding is complete, call
   `ensurePrimaryConversation`.
3. Once `conversationId` is known, start `conversationEvents`.
4. Query the latest `conversationTranscriptPage`.
5. Merge latest replay rows with any durable live rows that arrived first.
6. Render the loaded transcript window through the virtualized transcript.

Older pages load when the virtualized viewport approaches the first rendered
row. The request uses the current oldest page cursor. The loaded page is
prepended into the transcript window, deduped by durable `itemId`, and kept in
server-returned transcript order.

Live subscription events append to the same transcript window. Durable live
items are deduped by `itemId`; optimistic user entries continue to be replaced
through the existing `clientMessageId` path.

The transcript window model owns:

- loaded durable replay entries
- durable live entries
- optimistic local entries
- oldest page cursor
- latest-page loading state
- older-page loading and retry state
- `hasMoreBefore`

## Virtualized Transcript

`Transcript` remains the product render surface. `TranscriptScroller` becomes
TanStack Virtual-backed and renders only the visible row range plus overscan.

The virtualizer uses dynamic measurement because Noema transcript rows have
variable height:

- assistant text can be long
- assistant stream text grows while visible
- activity rows expand and collapse
- memory/tool markers can change status
- A2UI cards can have custom payload-dependent size
- the composer dock and mobile visual viewport change available height

Bottom-following remains a product behavior:

- if the user is near the end, live appends and streaming growth keep the
  viewport at the end
- if the user has scrolled upward, new live items do not yank the viewport
  downward
- prepending older pages preserves the user's visual position instead of jumping
  to the newly loaded rows

Existing arrival and text animation rules continue to use replay-vs-live source
metadata so fetched history does not animate as new work.

## Error Handling

If `primaryConversation` or `ensurePrimaryConversation` fails because onboarding
is incomplete, the existing setup and onboarding flow remains in charge.

If latest transcript page loading fails, chat should show a recoverable
transcript error notice or compact unavailable state without pretending provider
runtime resume is a substitute for Noema replay.

If older-page loading fails, the current visible transcript remains usable and
the top loading affordance becomes retryable.

Overlapping latest, older, and live data is expected. The merge model dedupes by
durable `itemId`, preserves optimistic replacement by `clientMessageId`, and
uses server-returned page order plus live append order as the client-side order
authority. The backend remains responsible for deriving that order from durable
conversation item append order.

If a subscription item arrives before the latest page finishes loading, the
window model preserves that live item and merges it once replay arrives.

## Testing

Rust store tests:

- latest visible page returns ascending items
- cursor page returns ascending items before the decoded cursor position
- deleted items are excluded from visible pages
- limits are clamped
- `hasMoreBefore` is correct at the beginning and middle of history

GraphQL tests:

- `primaryConversation` returns identity without transcript items
- `ensurePrimaryConversation` creates or attaches the primary conversation
  without returning transcript items
- `conversationTranscriptPage` supports null-cursor latest reads and non-null
  cursor older-page reads
- invalid limits and malformed cursors are rejected
- `cursor` appears on transcript page items
- durable subscription item events expose `cursor`

Frontend model tests:

- pages merge in server-returned transcript order
- overlapping pages dedupe by `itemId`
- durable live items replace optimistic entries through `clientMessageId`
- live-before-replay data is preserved and merged
- oldest page cursor and `hasMoreBefore` update correctly

Frontend validation:

```bash
bun run gen:types
bun run lint
bun run build
```

Per project instructions, browser inspection is not part of this UI slice unless
explicitly requested.
