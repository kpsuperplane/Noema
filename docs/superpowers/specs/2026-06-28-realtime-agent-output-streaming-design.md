# Realtime Agent Output Streaming Design

## Purpose

Stream assistant text into the web transcript while the provider is still
generating, without storing token deltas as durable conversation history.

The current system already has two pieces of the path:

- The Codex Responses provider sends upstream requests with `stream: true`.
- GraphQL subscriptions already deliver live conversation events to the web UI.

The missing piece is that provider SSE deltas are currently buffered into a
terminal `GenerateResponse`, and the runtime only publishes the final durable
assistant text item.

The product rule is:

> Live text deltas are runtime state. Final assistant text is durable history.

## Current Findings

The frontend uses GraphQL subscriptions over `/graphql/ws` for conversation
events. The subscription currently delivers durable conversation item events,
agent status events, and turn completion events.

The runtime publishes items through `TurnStreamEvent`. Despite the name, this is
item streaming, not realtime text streaming. A provider call completes first;
then the runtime persists and publishes final `AssistantText`, tool calls,
memory proposals, cards, and related activity.

The Codex provider already requests the Responses API with streaming enabled.
`ResponsesTransport::send_stream` currently reads the full HTTP response body,
parses all Server-Sent Events afterward, concatenates
`response.output_text.delta` values, and returns one `ResponsesResponse`.

This means realtime assistant output is feasible without changing providers or
the web transport, but the provider, runtime, GraphQL, and frontend contracts
need one ephemeral delta event path.

## Goals

- Show assistant text in the web transcript as it is generated.
- Keep replay deterministic from Postgres.
- Persist one final durable `assistant_text` item per assistant text output.
- Do not persist deltas as `conversation_items`.
- Preserve existing durable tool, memory, card, approval, status, and error
  events.
- Support both the initial provider response and a later local-tool
  continuation response.
- Keep the first slice focused on assistant text deltas only.

## Non-Goals

- Do not store token-level deltas in Postgres.
- Do not add conversation item update semantics in this slice.
- Do not stream tool calls, cards, memory proposals, or approval payloads in the
  first slice.
- Do not attempt cross-process replay of in-progress text after reconnect.
- Do not change the durable transcript schema for final assistant text.

## Architecture

Use ephemeral streaming events for live text and keep durable transcript history
unchanged.

Provider layer:

- Add a streaming-capable provider path alongside the existing terminal
  `generate` contract.
- Keep `GenerateResponse` as the terminal result used for persistence and
  structured output handling.
- Add provider-neutral events for assistant text deltas, for example
  `GenerateStreamEvent::AssistantTextDelta { delta }`.
- Codex Responses parses upstream SSE incrementally and emits a delta event for
  each `response.output_text.delta`.
- The provider still collects terminal output items from
  `response.output_item.done` and `response.completed`.

Runtime layer:

- Add `TurnStreamEvent::AssistantTextDelta`.
- Forward text deltas immediately over the existing runtime event channel.
- Persist final assistant text only after the terminal provider response is
  parsed.
- Use stable stream ids per provider generation segment:
  - `assistant_stream:<turn_id>:initial`
  - `assistant_stream:<turn_id>:continuation`
- Store the matching `stream_id` in the final durable assistant item's
  metadata so frontend reconciliation does not depend on inference.

GraphQL layer:

- Add `GraphqlAssistantTextDeltaEvent` to `GraphqlConversationEvent`.
- Include `conversationId`, `turnId`, `streamId`, and `delta`.
- Keep `GraphqlConversationItemEvent` as the only durable transcript item event.

Frontend layer:

- Add an ephemeral streaming assistant transcript entry keyed by `streamId`.
- Append incoming deltas to that entry.
- Render the ephemeral entry with the normal assistant bubble path.
- When the durable final assistant text arrives, replace the matching ephemeral
  entry with the durable item.

## Data Flow

For a simple response:

1. Human sends a turn.
2. Runtime persists and publishes the durable user item.
3. Runtime calls provider streaming generation with a stream id such as
   `assistant_stream:<turn_id>:initial`.
4. Provider emits text deltas as the upstream SSE stream arrives.
5. Runtime publishes `AssistantTextDelta` events.
6. GraphQL subscription forwards those deltas.
7. Frontend creates or updates one ephemeral assistant bubble.
8. Provider returns terminal `GenerateResponse`.
9. Runtime parses structured output, persists final assistant text, and
   publishes the durable assistant item.
10. Frontend replaces the ephemeral bubble with the durable assistant entry.
11. Runtime publishes turn completion.

For a local `search_memory` continuation:

1. Initial provider response may stream text such as "I will check memory."
2. Runtime persists initial provider outputs and local tool result items.
3. Runtime starts a second provider streaming generation with
   `assistant_stream:<turn_id>:continuation`.
4. Continuation deltas update a second ephemeral assistant bubble.
5. Final continuation text is persisted and replaces the continuation bubble.
6. Memory extraction/proposal handling continues to use final durable text.

## Frontend Reconciliation

The frontend should prefer durable data whenever both ephemeral and durable data
exist.

- Delta event with unknown `streamId`: append a new ephemeral assistant entry.
- Delta event with known `streamId`: append `delta` to the existing ephemeral
  entry.
- Durable assistant item whose metadata `stream_id` matches an ephemeral entry:
  replace the ephemeral entry.
- Durable assistant item without a matching ephemeral entry: append normally.
- Final durable text differing from accumulated deltas: use durable text.
- Turn error before final text: keep the live draft for the current session and
  render the error marker, but do not expect replay to contain the draft.

The durable assistant item should carry `stream_id` in item metadata. This is
not a transcript schema change because item metadata already exists, and it
keeps reconciliation deterministic.

## Error Handling

Provider stream fails before a terminal response:

- Publish an error notice and turn completion.
- Do not persist partial assistant text.
- Keep any ephemeral draft visible in the current web session, followed by the
  error marker.

Provider deltas arrive but terminal structured parsing fails:

- Treat the durable turn as failed.
- Do not persist the partial draft as assistant history.
- Publish the runtime error through the existing error path.

GraphQL reconnects mid-turn:

- Replay does not include ephemeral deltas.
- The UI may temporarily lose the in-progress draft until the final durable
  item arrives.
- This is acceptable for the first slice because persistence remains clean and
  deterministic.

High-frequency deltas:

- The provider/runtime can forward raw deltas first.
- The frontend should remain free to batch render updates if React rendering
  becomes noisy.
- A later slice can coalesce deltas server-side if needed.

## Testing And Validation

Backend tests:

- Responses SSE parser emits deltas before terminal response.
- Codex provider streaming path forwards `response.output_text.delta` events.
- Runtime publishes `AssistantTextDelta` before final `AssistantText`.
- Runtime does not persist delta events as conversation items.
- Runtime still persists final assistant text exactly once.
- Local `search_memory` continuation can stream its continuation answer.
- Provider failure after deltas publishes an error and does not persist partial
  assistant text.

GraphQL tests:

- Schema exposes `GraphqlAssistantTextDeltaEvent`.
- Subscription delivers delta events in order.
- Existing conversation item, status, and completion events still work.

Frontend tests:

- Delta events create and append to an ephemeral assistant entry.
- Final durable assistant item replaces the matching ephemeral entry.
- Durable mismatch prefers durable text.
- Replay does not require ephemeral events.
- Sticky-bottom behavior continues while live text grows, unless the user has
  scrolled away from the bottom.

Validation:

- `cargo fmt --all --check`.
- `cargo check --workspace`.
- Relevant runtime/provider/GraphQL tests.
- `bun run gen:types`, `bun run lint`, and `bun run build` from
  `crates/noema-core/web`.
- Local web smoke check that assistant text appears before turn completion.

## Acceptance Criteria

- Assistant text appears in the web transcript while the provider is still
  generating.
- Final replay contains one durable assistant text item, not token chunks.
- Deltas are never stored as standalone conversation items.
- Existing durable tool, memory, card, approval, status, and error behavior
  remains intact.
- Local tool continuation responses can stream.
- GraphQL reconnect/replay remains deterministic from Postgres.
- On stream failure, partial text is session-only and the durable transcript
  records the error through the existing error path.
