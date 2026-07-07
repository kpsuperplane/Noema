# Chat Boot Transcript Design

## Problem

The live Chrome trace from `Trace-20260706T200843.json.gz` shows Noema Web
taking about one second to show real chat messages. The dominant issue in that
trace is a startup GraphQL waterfall:

- app assets finish around 91 ms after navigation
- `LocalStatus` completes around 236 ms
- `OnboardingStatus` completes around 311 ms
- `PrimaryConversation` completes around 510 ms
- `ConversationTranscriptPage` completes around 785 ms
- transcript content becomes the LCP candidate around 966 ms

The current frontend structure contributes to this because `AppContent` starts
with separate boot queries, then waits for `primaryConversation` before it can
request the latest transcript page.

## Goals

- Make the common already-onboarded chat route show existing messages sooner.
- Keep frontend boot code straightforward.
- Preserve the existing general-purpose transcript pagination query for older
  pages and retry paths.
- Avoid Apollo cache field-policy complexity for this slice.
- Keep first-run onboarding behavior correct.

## Non-Goals

- Do not implement asset compression, route code splitting, or cache headers in
  this slice.
- Do not redesign transcript rendering or TanStack Virtual behavior here.
- Do not create a broad "app boot" mega object that owns all future startup
  concerns.
- Do not add migrations or backwards compatibility layers.

## API Design

Extend `PrimaryConversation` with a nested latest transcript page field:

```graphql
type PrimaryConversation {
  conversationId: String!
  provider: String!
  latestTranscriptPage(limit: Int): ConversationTranscriptPage!
}
```

The existing query remains the general pagination API:

```graphql
type Query {
  conversationTranscriptPage(input: ConversationTranscriptPageInput!): ConversationTranscriptPage!
}
```

`latestTranscriptPage(limit:)` should use the same backend page implementation
as `conversationTranscriptPage(input:)`, passing the parent conversation id and
no cursor. The limit should follow the same validation and default behavior as
the standalone page query.

This keeps conversation identity and transcript concerns related but separate:
the root `primaryConversation` field answers "which conversation?", while the
nested field answers "what is the latest visible page for this conversation?"

## Frontend GraphQL Operations

Introduce a shared transcript page fragment:

```graphql
fragment ConversationTranscriptPageFields on ConversationTranscriptPage {
  items {
    ...ConversationItemFields
  }
  pageInfo {
    beforeCursor
    hasMoreBefore
    limit
  }
}
```

Use it from both the boot operation and the standalone pagination operation:

```graphql
query ChatBoot($transcriptLimit: Int = 80) {
  localStatus {
    localService
    assistantConnection
    memoryStorage
    primaryAgentDisplayName
  }
  onboardingStatus {
    isUserOnboarded
    steps {
      id
      status
      providerKind
      providerAccountId
      accountKey
      displayName
      providerAccountStatus
      authMethod
    }
  }
  primaryConversation {
    conversationId
    provider
    latestTranscriptPage(limit: $transcriptLimit) {
      ...ConversationTranscriptPageFields
    }
  }
}

query ConversationTranscriptPage($input: ConversationTranscriptPageInput!) {
  conversationTranscriptPage(input: $input) {
    ...ConversationTranscriptPageFields
  }
}
```

Apollo can fetch all three root fields in one operation. The nested
`latestTranscriptPage` solves the part GraphQL cannot express with independent
root fields: feeding `primaryConversation.conversationId` into a sibling
`conversationTranscriptPage` root field.

## Frontend State Flow

Replace the current boot waterfall with one `ChatBoot` query in `AppContent`.
The frontend should initialize existing state from that result:

- `status` comes from `data.localStatus`.
- `onboarding` comes from `data.onboardingStatus`.
- If `primaryConversation` exists, call the same acceptance path used today to
  set `conversationId`, `socketState: "ready"`, and `agentStatus: "IDLE"`.
- If `primaryConversation.latestTranscriptPage` exists, merge its items into
  the `TranscriptWindowState` with placement `latest`.
- Mark that conversation's latest transcript as loaded so the existing effect
  does not immediately re-fetch the same page.

The existing `ensurePrimaryConversation` mutation remains the fallback for the
rare onboarded state where no primary conversation exists. After ensuring a
conversation, the app may use the existing latest transcript loading path. That
path is not trace-critical because it should be uncommon.

The existing `conversationTranscriptPage` query remains responsible for older
history reads and retrying transcript page loads after boot.

## Apollo Cache Position

Do not rely on Apollo cache hydration for this slice.

Although both fields return the same `ConversationTranscriptPage` GraphQL type,
Apollo stores `primaryConversation.latestTranscriptPage(limit:)` and
`Query.conversationTranscriptPage(input:)` under different field paths and
argument sets. Making the nested field transparently hydrate the standalone
pagination query would require custom cache field policies or manual
`cache.writeQuery` calls.

That complexity is not needed because the transcript window model is already
the source of truth for rendering, optimistic entries, subscription races, and
older-page merging. The boot page should hydrate the transcript window directly.

## Error Handling

- If `ChatBoot` fails entirely, keep using the existing app boot error boundary.
- If the user is not onboarded, the setup flow should render from
  `onboardingStatus` as it does today. A missing primary conversation should be
  represented as `primaryConversation: null`.
- If the user is onboarded but no primary conversation exists, render the chat
  startup skeleton and run `ensurePrimaryConversation`.
- If `latestTranscriptPage` fails, the whole boot query will fail. That is
  acceptable for the first implementation because the page is local store data;
  partial boot failure handling can be added later if real failures appear.

## Testing

Backend tests:

- `primaryConversation` can return `latestTranscriptPage(limit:)` using the
  same item order and page info as `conversationTranscriptPage`.
- `primaryConversation` remains nullable when no primary conversation exists.
- `latestTranscriptPage(limit:)` applies the same limit bounds as the standalone
  page query.

Frontend tests:

- The boot operation reuses `ConversationTranscriptPageFields`.
- Boot data initializes the transcript window without issuing an immediate
  standalone latest-page query.
- Older-page loading still uses `ConversationTranscriptPageDocument` with the
  boot page's `beforeCursor`.

Validation:

- `bun run gen:types`
- `bun test src`
- `bun run lint`
- Targeted browser/network verification against `:3737`: a returning chat boot
  should issue one `ChatBoot` POST before messages render, not the previous
  status/onboarding/primary/transcript sequence.

## Expected Impact

The trace-critical request should move from the fourth GraphQL POST to the first
GraphQL POST. In the captured trace, that means transcript data could begin
loading around the time `LocalStatus` currently starts, rather than after
`PrimaryConversation` resolves. The exact LCP improvement depends on server
response time and client render cost, but this removes the largest avoidable
waterfall without making frontend state ownership more complex.
