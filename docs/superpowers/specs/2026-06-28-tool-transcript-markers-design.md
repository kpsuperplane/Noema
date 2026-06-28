# Tool Transcript Markers Design

## Purpose

Make transcript tool calls behave like memory markers: compact assistant-side
markers that can expand into a structured card. This keeps tool activity visible
and auditable without letting raw activity rows interrupt the conversation
layout.

The product rule is:

> Tool activity is assistant-originated transcript context, so it uses the
> assistant lane and marker/card surfaces.

## Current Findings

The daemon already persists and replays tool calls, tool results, and approval
events as `TurnTranscriptItem.Activity` values. The frontend receives the right
raw data, including `activity_kind`, `status`, `title`, `summary`, and
`metadata`.

The current transcript renderer treats memory events specially, grouping them
into expandable memory markers, but it renders tool activities through the
generic `ActivityRow` card path. That creates a separate visual language for
tool events even though they are the same kind of assistant-side context as
memory markers.

No backend schema change is needed for the first pass. This is a frontend
normalization and rendering cleanup.

## Goals

- Render tool calls and results as assistant-lane markers.
- Group adjacent tool call and tool result activities from the same turn into a
  single expandable marker when possible.
- Keep call-only and result-only cases visible as standalone markers.
- Use the shared `Marker` component for the compact row.
- Use the shared transcript card surface for expanded details.
- Preserve red/error styling for failed tool results.
- Keep non-tool activity rows working through the existing generic activity
  renderer unless they are intentionally converted later.

## Non-Goals

- Do not change the persisted transcript model or GraphQL schema.
- Do not redesign all activity, approval, or card rendering in this slice.
- Do not special-case `search_memory` as a memory marker. Memory extraction and
  memory proposal markers remain separate from agent tool-use markers.
- Do not implement human file attachments in this slice, though the shared card
  surface should remain compatible with future human-originated cards.

## Architecture

Keep the existing frontend boundary:

- `transcript.ts` continues normalizing GraphQL replay and subscription events
  into transcript entries.
- `Transcript.tsx` owns presentation grouping because grouping depends on local
  adjacency within rendered transcript order.

Add a tool marker render item beside the existing memory marker render item:

```ts
type RenderTranscriptEntry =
  | { kind: "entry"; entry: TranscriptEntry }
  | { kind: "typing" }
  | { kind: "memory_marker"; marker: MemoryMarkerGroup }
  | { kind: "tool_marker"; marker: ToolMarkerGroup };
```

`ToolMarkerGroup` should hold the original activity entries instead of copying
metadata into a second model:

```ts
type ToolMarkerGroup = {
  id: string;
  call?: TranscriptEntryActivity;
  result?: TranscriptEntryActivity;
};
```

This keeps the renderer close to the replay data and reduces the chance that
future activity metadata is dropped during grouping.

## Grouping Rules

Tool grouping runs during the same render-entry preparation step as memory
grouping.

1. When an activity has `activity_kind === "tool_call"`, create a tool marker.
2. If the immediately following item in the same turn is a
   `tool_result` activity, fold that result into the same marker.
3. If a `tool_result` has no adjacent call, render it as a standalone marker.
4. Call-only markers remain visible while a turn is in progress or if a result
   never arrives.
5. Grouping does not cross human messages, assistant messages, errors, memory
   markers, structured cards, or turn boundaries.

The first pass should use adjacency rather than a looser id-based search. That
matches the current persisted replay order, keeps behavior predictable, and
avoids accidentally merging unrelated repeated tool calls.

## Rendering

Every tool marker renders inside `TranscriptRow` with the assistant lane.

Collapsed marker:

- Uses `Marker`.
- Uses a neutral tone for pending or successful calls.
- Uses an error tone when the result status indicates failure.
- Uses a short label derived from the best available data:
  - `Used <tool name>` when metadata has an action/tool name.
  - Existing activity title when no tool name can be derived.
  - `Tool call` or `Tool result` as the final fallback.

Expanded card:

- Uses the shared transcript card or attachment surface.
- Shows call details when present, including tool name and arguments/metadata.
- Shows result details when present, including summary, output metadata, status,
  and failure text if available.
- Keeps raw JSON readable for development, but formats it compactly so a large
  payload does not make the transcript unusable.

Memory markers and tool markers should feel like siblings: same lane, spacing,
expand control, and marker treatment, with content-specific icons and labels.

## Error Handling

Malformed or incomplete activity data must not break the transcript.

- Missing metadata falls back to `title`, then `summary`, then a generic label.
- Unknown tool status uses the neutral marker tone.
- Failed tool results use the error marker tone and keep error details in the
  expanded card.
- Result-only failures still render in the assistant lane as expandable error
  markers.

Transcript-level errors continue to use the existing error marker path. Tool
execution failures are tool markers because they belong to the tool event.

## Testing And Validation

Add focused frontend tests around the rendering contract:

- Adjacent `tool_call` and `tool_result` activities group into one
  `tool_marker`.
- Call-only and result-only tool activities render as markers.
- Failed tool results produce an error-tone marker.
- Tool markers classify to the assistant lane.
- Non-tool activity rows still use the existing activity renderer.
- Existing memory marker grouping still works after adding the tool grouping
  pass.

Run frontend validation from `crates/noema-core/web`:

- `bun run gen:types` if generated GraphQL types change.
- `bun run lint`.
- `bun run build`.

Because this is transcript layout work, implementation should also run the web
UI locally and inspect desktop and mobile views for alignment, overflow, and
expand/collapse behavior.

## Acceptance Criteria

- Tool calls appear as assistant-lane markers rather than generic inline
  activity cards.
- Adjacent tool call/result pairs collapse into one expandable marker.
- Standalone call-only or result-only tool events remain visible.
- Failed tool results remain red/error-toned.
- Expanded tool details use the shared transcript card surface.
- Memory markers keep their existing behavior and visual consistency.
- Non-tool activities are not accidentally converted or hidden.
