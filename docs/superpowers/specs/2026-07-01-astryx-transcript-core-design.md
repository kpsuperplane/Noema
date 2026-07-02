# Astryx Transcript Core Design

## Summary

Apply the useful core transcript techniques from Astryx's AI chat template to
Noema's existing transcript without replacing Noema's chat shell.

This is a targeted adoption of Astryx chat primitives, not a full `ChatLayout`
rewrite. Noema keeps ownership of transcript replay, streaming behavior,
arrival animation, bottom-follow scrolling, live-region accessibility, memory
markers, tool markers, A2UI cards, attachments, and the custom composer.

## Context

The current web transcript lives in
`crates/noema-core/web/src/components/transcript`. It already has Noema-specific
behavior that should remain authoritative:

- `Transcript` maps persisted and live conversation entries into renderable
  transcript rows.
- `renderModel.ts` groups memory extraction rows, memory proposal cards, and
  tool call/result activity into Noema-owned render entries.
- `TranscriptScroller` owns the scroll viewport, `role="log"` live-region
  behavior, scroll-to-end affordance, and scroll context used by
  `TranscriptBottomFollower`.
- `Message`, `TypingMessage`, `TranscriptRow`, `MemoryMarker`, `ToolMarker`,
  `ActivityRow`, `StructuredCard`, and attachment components express
  Noema-specific transcript semantics.
- The composer is a Noema-specific component and remains outside this slice.

Astryx's AI chat template demonstrates several patterns that fit Noema's
transcript direction:

- Sender-aware `ChatMessage` wrappers.
- Sender-aware `ChatMessageBubble` content containers.
- Message metadata slots.
- System/status rows through `ChatSystemMessage`.
- Tool-call presentation through `ChatToolCalls`.
- Grouped consecutive message bubbles.
- Markdown/code/table capable message bodies.
- A separate artifact preview panel.

Only the core transcript patterns are in scope for this design. The artifact
panel is intentionally deferred.

References:

- `https://github.com/facebook/astryx/tree/main/packages/cli/templates/pages/ai-chat`
- `@astryxdesign/core` local package docs and type definitions for
  `ChatMessage`, `ChatMessageBubble`, `ChatMessageMetadata`,
  `ChatSystemMessage`, `ChatToolCalls`, `ChatMessageList`, and `ChatLayout`.

## Goals

- Make user, assistant, streaming assistant, and typing rows use Astryx chat
  message primitives where they fit.
- Use Astryx `ChatSystemMessage` for true non-sender transcript notices:
  errors and generic non-interactive activity/status notices.
- Preserve Noema's current information architecture, transcript data flow, and
  composer.
- Keep Noema's scroll, replay, live-region, and animation behavior stable.
- Align memory, tool, activity, and structured-card rows visually with Astryx
  chat conventions while preserving their Noema-owned semantics.
- Keep implementation small enough for one agent to execute end to end after a
  separate implementation plan.

## Non-Goals

This slice does not:

- Replace `TranscriptScroller` with Astryx `ChatLayout`.
- Replace the Noema composer with Astryx `ChatComposer`.
- Add an artifact side panel or generated-document preview.
- Add new backend transcript item types.
- Change GraphQL operations, subscriptions, persistence, replay, or provider
  streaming behavior.
- Add UI tests unless explicitly requested.
- Use Tailwind, shadcn, Base UI, or old compatibility wrappers.
- Infer semantic intent from user text by direct phrase matching.

## Design

### Transcript Shell

Keep `Transcript`, `TranscriptScrollerProvider`, `TranscriptScroller`,
`TranscriptBottomFollower`, and the render model as Noema-owned infrastructure.

Astryx `ChatLayout` is not used in this slice because it expects to own the
chat scroll context and scroll-to-bottom behavior. Noema's current scroller
already carries product-specific behavior around live transcript accessibility,
arrival animation timing, replay suppression, and bottom-following during
streaming updates.

`ChatMessageList` may be evaluated during implementation, but it should only be
used if it remains purely presentational inside the existing `TranscriptScroller`
content area and does not interfere with spacing, alignment, live-region
behavior, or item-level scroll metadata.

### Message Rows

Refactor the local `Message` component into a Noema transcript message adapter
that renders:

- Astryx `ChatMessage` with `sender="user"` for human rows.
- Astryx `ChatMessage` with `sender="assistant"` for assistant and assistant
  stream rows.
- Astryx `ChatMessageBubble` for the visible bubble surface.
- Existing `AnimatedMessageText` inside the bubble so current streaming and
  arrival text behavior remains intact.
- Noema `IdentityAvatar` as the `avatar` slot so actor identity remains
  deterministic and aligned with the rest of the product.

The adapter should preserve the current `Message` API as much as practical:
`role`, `text`, `animate`, and `showAvatar`. If Astryx handles avatar visibility
cleanly through a slot, use that directly. If not, keep a small StyleX wrapper
around the avatar slot rather than changing transcript grouping behavior.

User and assistant bubble styling should come from Astryx plus the Noema Astryx
theme wherever possible. StyleX should only handle Noema-specific constraints
such as maximum width, wrapping, streaming text behavior, and integration with
the existing scroller.

### Grouping

Keep Noema's current lane-based avatar grouping as the first step:

- Consecutive rows in the same lane still suppress repeated avatars.
- Existing memory/tool marker clustering remains unchanged.
- Bubble corner grouping through Astryx `ChatMessageBubble`'s `group` prop may
  be added only if it can be derived from adjacent rendered transcript entries
  without changing render identity or scroll behavior.

If implementation adds bubble grouping, it should be purely visual and limited
to adjacent text-message rows from the same sender. It must not group across
memory markers, tool markers, activity rows, structured cards, errors, or
typing indicators.

### Typing Row

Refactor `TypingMessage` to use the same Astryx message/bubble structure as
assistant text rows while keeping Noema's current typing indicator dots and
`role="status"` / `aria-label` behavior.

The typing row should still participate in Noema's reveal-after-arrival logic
and should not become a separate system message.

### Tool Rows

Noema's grouped `tool_call` and `tool_result` marker model remains
authoritative.

During implementation, evaluate mapping `ToolMarker` to Astryx `ChatToolCalls`:

- `tool_call` maps to a call with `status="running"` or `pending`.
- `tool_result` maps to `status="complete"` or `error` depending on Noema
  activity status.
- Existing `ToolDetailAttachment` remains the detail view unless Astryx's
  built-in detail handling can host the same content without losing
  accessibility or Noema metadata.

If the mapping is awkward or hides Noema-specific details, keep the current
`ToolMarker` data flow and only update its visual frame to sit better next to
Astryx message bubbles. Tool markers must remain expandable and must preserve
their current labels, pending state, and error tone.

### System Notices, Memory, And Activity Rows

Introduce a Noema-owned system notice adapter around Astryx
`ChatSystemMessage`. The adapter owns Noema tone mapping and accessibility
roles, while Astryx owns the centered system-message presentation.

Use this adapter for:

- Error notices, preserving `role="alert"` for non-recoverable errors and
  `role="status"` for recoverable notices.
- Generic non-interactive activity/status notices that are not memory-specific,
  tool-specific, structured-card content, or sender content.

Do not use `ChatSystemMessage` where it would hide Noema domain meaning. In
particular:

- Memory markers stay expandable and continue to expose saved, proposed,
  reinforced, failed, and reviewable memory details.
- Tool markers stay expandable and continue to expose call/result detail
  through the Astryx tool-call path.
- Memory-specific activity rows such as explicit memory saves stay Noema-owned
  detail cards unless they are later folded into the memory marker model.
- Structured cards keep schema-specific rendering.

This slice should not invent date dividers or new transcript item types.

### Markdown And Code

Plain transcript text should remain plain text in this slice. The design does
not add markdown/code parsing to ordinary assistant output.

Markdown/code rendering can be a future slice once Noema decides which
transcript item types are trusted markdown and how code blocks should interact
with copying, redaction, provenance, and accessibility.

### Styling

Use Astryx and the current Noema Neutral-derived theme for commodity chat
surfaces. Use StyleX for:

- Transcript layout constraints inside Noema's scroller.
- Message width and wrapping behavior.
- Avatar visibility if Astryx slots need adaptation.
- Streaming/typing state.
- Noema-specific marker, card, and attachment states.

Do not introduce Tailwind classes, shadcn-compatible wrappers, or generic
primitive buckets.

## Validation

Implementation should validate with:

- `git status --short --branch`
- `git diff --check`
- `bun run lint` from `crates/noema-core/web`
- `bun run build` from `crates/noema-core/web`

Per current project guidance, do not add UI tests or inspect with browser tools
unless explicitly requested.

## Open Follow-Ups

- Decide in a later slice whether trusted assistant markdown/code rendering
  should use Astryx `Markdown` and `CodeBlock`.
- Decide in a later slice whether A2UI artifacts should open an Astryx-style
  artifact side panel.
- Revisit `ChatLayout` only if Noema deliberately chooses to transfer scroll
  ownership from the product transcript to Astryx.
