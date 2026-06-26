# Primary Home Conversation Design

## Status

Approved design for making the web home chat load one durable primary
conversation for the local human. No implementation has been done in this spec.
The next step is an implementation plan.

## Problem

The current web chat opens a fresh daemon conversation whenever the page loads.
That makes the home chat feel temporary even though Noema now persists
conversation rows, turns, and transcript items in Postgres.

Noema needs a permanent home conversation for the primary human. The home page
should load that conversation, replay its durable transcript, and append future
turns to the same Noema conversation. The conversation must belong to Noema's
database model rather than to any provider-specific thread state, so future
provider switching can happen without changing the user's home thread identity.

## Decisions

- Store the home conversation pointer directly on the human row as
  `humans.primary_conversation_id`.
- Use the current default human, `human:local`, as the first owner of the home
  chat assignment.
- Treat Noema's `conversations` and `conversation_items` tables as the source of
  truth for conversation continuity.
- Remove `provider_thread_id` from the current conversation contract and schema.
- Do not rely on provider-native thread state to resume the home chat.
- Send future turns to the currently configured provider with context assembled
  from Noema-owned durable state.
- Keep future multiple-thread support simple: changing the home thread later is
  an update to `humans.primary_conversation_id`.

## Non-Goals

- Do not build the multiple-thread UI in this slice.
- Do not add provider switching controls in this slice.
- Do not preserve backwards compatibility for existing development databases.
  The project is pre-stable and schema changes may rewrite tables directly.
- Do not introduce a separate conversation-preferences table for this feature.
- Do not add provider-specific optimization metadata unless a later design shows
  it is useful.

## Architecture

Add a nullable `primary_conversation_id` column to `humans` with a foreign key to
`conversations(conversation_id)`. For `human:local`, this column is the durable
home chat assignment.

The daemon gains a start-or-resume-primary-conversation path for the web home
chat. This path ensures default actors exist, loads
`human:local.primary_conversation_id`, creates a local chat conversation when
the value is missing or invalid, stores the replacement id on the human row, and
returns the durable conversation id to the client.

The web home page stops blindly creating a fresh conversation. On load, it asks
for the primary conversation, renders replayed `conversation_items`, and sends
new turns against that same `conversation_id`.

The currently configured provider is an answer-generation adapter, not the owner
of conversation identity. The runtime may keep in-memory per-turn state while
the daemon is alive, but after restart the next turn should be assembled from
Noema's durable transcript and other governed context.

## Components

- `humans.primary_conversation_id`: stores the primary home conversation for a
  human.
- Conversation repository: creates, loads, validates, and updates the primary
  conversation assignment.
- Conversation replay repository methods: return visible `conversation_items`
  for the selected home conversation.
- Daemon runtime: registers the durable primary conversation as active without
  creating a second `conversations` row.
- Web protocol: adds a `primary_conversation_start` client message for primary
  home conversation start. The existing `conversation_start` message can remain
  for future explicit new-thread flows.
- Web app: requests the primary conversation on page load, renders replay, and
  sends turns to the returned conversation id.

## Data Flow

1. The home chat opens.
2. The daemon ensures `human:local` and `agent:primary` exist.
3. The daemon loads `human:local.primary_conversation_id`.
4. If the value is null, missing, deleted, or inaccessible, the daemon creates
   one local chat conversation and saves its id on `human:local`.
5. The daemon replays visible `conversation_items` for that conversation.
6. The web app renders the replayed transcript.
7. Future messages append new turns and items to the same Noema conversation.

## Error Handling And Edge Cases

If `primary_conversation_id` is null, create the home conversation and save it
before replay.

If `primary_conversation_id` points at a missing, deleted, or inaccessible
conversation, create a replacement and update the column. Because Noema is
pre-V1, no migration-style recovery UI is needed.

If two browser tabs initialize at the same time, the backend should avoid
creating two home conversations. The repository method should perform the
load-or-create assignment in a transaction.

If one replay item has a malformed payload, the backend should not make the
whole home chat unusable. It should skip only the malformed item and append a
visible warning row to the replay response.

If the currently configured provider cannot answer a turn, the durable home
conversation remains the same. The failure should be appended as an error item
when it meaningfully affected the turn.

Removing `provider_thread_id` means implementation must update Rust structs,
generated frontend types, tests, and docs that currently describe it as part of
the conversation contract.

## Testing

Backend tests should cover:

- Schema/bootstrap includes `humans.primary_conversation_id`.
- Schema/bootstrap no longer includes `conversations.provider_thread_id`.
- Creating or loading the primary conversation is idempotent for `human:local`.
- Primary conversation replay returns existing `conversation_items`.
- A restart-style flow: create primary conversation, append items, construct
  fresh runtime or web state, start the primary conversation again, and confirm
  the same `conversation_id` replays.
- Turns after reload append to the same Noema conversation.
- Frontend protocol round trips `primary_conversation_start` and no longer
  exposes `provider_thread_id`.

Frontend validation should cover:

- Generated TypeScript protocol updates cleanly.
- Home chat loads replay instead of showing the empty starter state when the
  primary conversation has history.
- Sending from the replayed home chat appends to the same conversation.
- `bun run gen:types`, `bun run lint`, and `bun run build` pass in
  `crates/noema-core/web`.
- The local app is visually checked at desktop and mobile sizes for the empty
  state, replayed transcript state, and composer behavior.

## Documentation Updates

Update frontend and persistence docs so they describe Noema-owned conversation
continuity:

- The default home chat loads `human:local.primary_conversation_id`.
- Durable history is reconstructed from `conversation_items`.
- Provider-native thread ids are not part of the current product contract.
- Future provider switching should preserve the Noema conversation id.
