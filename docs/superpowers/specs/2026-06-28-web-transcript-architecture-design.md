# Web Transcript Architecture Design

## Purpose

Clean up the Noema web transcript so every item is naturally and consistently
placed. The immediate problem is that error messages, memory markers, and
activity rows do not align with assistant chat bubbles because each renderer
currently owns its own transcript offset.

The desired product rule is:

> Placement is based on origin, not surface type.

Human-originated items appear in the human lane. Everything not originated by
the human appears in the assistant lane. A card, marker, or bubble is only a
surface choice; it does not decide lane placement.

## Current Findings

The current React transcript has the right raw ingredients but mixes placement
and visual styling:

- Assistant and human text use the `Message` primitive, which creates a stable
  avatar gutter and content column.
- `ErrorMarker` renders a bare `Marker`, so transcript errors do not inherit the
  assistant lane.
- `MemoryMarker` manually uses `ml-10` and a narrower width, which approximates
  the assistant content offset but creates a second alignment system.
- `ActivityRow` and structured cards render attachments directly, so they rely
  on local max-width and placement classes.
- `Marker`, `Attachment`, and `Bubble` are reusable primitives, but transcript
  code still creates one-off visual rules around them.

This makes new transcript event types likely to drift unless each renderer
remembers to duplicate the same offsets.

## Design Principles

1. The transcript has two lanes: human and assistant.
2. Origin determines lane. Surface type does not.
3. The row frame owns placement. Visual primitives own appearance.
4. There should be one standard `Bubble`, one standard `Marker`, and one
   standard card or attachment surface for transcript content.
5. Error styling remains red, but errors are still assistant-originated
   transcript markers unless the backend later models a human-originated error.
6. Human file attachments must be able to render as human-lane cards later.

## Architecture

Keep the current data normalization boundary:

- `transcript.ts` continues translating GraphQL replay and subscription events
  into `TranscriptEntry` values.
- `Transcript.tsx` handles rendering and grouping.

Add a small transcript layout boundary inside the renderer:

- `TranscriptRow` owns lane alignment, avatar gutter, responsive width, and
  content column placement.
- `AssistantTranscriptRow` is used for Noema-originated text, typing, errors,
  activity rows, memory markers, structured cards, tool events, approval events,
  and future system notices.
- `HumanTranscriptRow` is used for human text now and human-originated cards or
  file attachments later.

The row frame should not know the details of memory, tools, approvals, or error
copy. It should only know origin, avatar, alignment, and sizing.

## Surface Model

Transcript content should be composed from a small, standardized visual stack:

| Surface | Use | Lane source |
| --- | --- | --- |
| `Bubble` | Conversational text and typing indicator | Origin |
| `Marker` | Compact status, notice, memory update, and error lines | Origin |
| Card / `Attachment` | Structured object and event details, including future file attachments | Origin |

Marker variants should express tone without changing layout:

- `default`: neutral status.
- `success`: completed or saved state.
- `warning`: recoverable concern or needs attention.
- `error`: failed or unrecoverable state, red.
- `muted`: low-emphasis metadata if needed.

`ErrorMarker` can either become a small wrapper around `Marker variant="error"`
or disappear if the call sites are clearer using `Marker` directly.

## Placement Rules

Use these rules during rendering:

| Entry or rendered item | Lane | Surface |
| --- | --- | --- |
| Human text | Human | Bubble |
| Future human file attachment | Human | Card / Attachment |
| Assistant text | Assistant | Bubble |
| Typing indicator | Assistant | Bubble |
| Error notice | Assistant | Marker with error variant |
| Activity row | Assistant | Card / Attachment |
| Memory marker | Assistant | Marker, with optional expanded card |
| Structured assistant card | Assistant | Card / Attachment |
| Future tool or approval event | Assistant | Marker or card, depending on density |

The current user request specifically sets the default: anything not from the
human should appear as if it is coming from the assistant.

## Component Changes

The implementation should keep the first pass modest:

- Add the transcript row/frame abstraction in or near `Transcript.tsx`.
- Route every current non-user transcript item through the assistant row.
- Remove manual transcript placement classes such as memory marker `ml-10` and
  width calculations that duplicate the assistant gutter.
- Extend `Marker` with tone variants so errors, memory updates, and notices use
  the same component.
- Keep `Attachment` as the current card-like surface unless a clearer `Card`
  wrapper emerges naturally from existing usage.
- Do not redesign the full chat shell, composer, onboarding flow, GraphQL
  protocol, or persisted transcript model in this cleanup.

If `Transcript.tsx` becomes harder to reason about, extract only the layout
pieces or repeated content renderers needed to keep the file focused. Do not
perform a broad frontend refactor as part of this slice.

## Error Handling

Transcript errors should share assistant-lane placement:

- Recoverable errors render with `role="status"`, an assistant lane, and a
  notice-style label.
- Non-recoverable errors render with `role="alert"`, an assistant lane, and the
  `error` marker variant.
- Both preserve red error styling where appropriate.

This makes errors feel like Noema reporting a problem in the conversation
rather than an unplaced UI fragment.

## Testing And Validation

Add focused unit coverage around the rendering contract:

- User entries classify to the human lane.
- Assistant entries classify to the assistant lane.
- Error, activity, structured card, typing, and grouped memory marker entries
  classify to the assistant lane.
- Existing typing indicator behavior remains covered.

Run frontend validation from `crates/noema-core/web`:

- `bun run gen:types` if generated GraphQL types are affected.
- `bun run lint`.
- `bun run build`.

Because this is layout work, implementation should also run the local web UI and
capture desktop and mobile screenshots. Inspect that assistant text, errors,
memory markers, activity cards, typing, and user bubbles align consistently and
do not overflow.

## Non-Goals

- No backend transcript schema changes.
- No migration or persisted conversation compatibility layer.
- No redesign of onboarding, composer, header, memory settings, or inspection
  routes.
- No full design-system rewrite beyond the marker standardization needed for
  transcript consistency.
- No implementation of human file attachments in this slice; only preserve the
  architecture so human-lane cards can be added later.

## Acceptance Criteria

- Every current non-human transcript item renders in the assistant lane.
- User text still renders in the human lane.
- The transcript has one placement abstraction instead of one-off offsets per
  item type.
- `Marker` has standardized tone variants, and errors use the red error tone.
- Memory markers no longer depend on manual assistant-offset classes.
- Future human-originated cards can use the human lane without changing the card
  primitive itself.
- Desktop and mobile smoke checks show aligned transcript rows with no obvious
  overlap or overflow.
