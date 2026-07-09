# Chat Detail Rail Artifact Viewer Design

## Summary

Noema should add a generic, read-only chat detail rail for contextual objects opened
from the transcript. The first supported detail type is a local artifact version
rendered as Markdown. This is a viewer, not an editor: artifact rows open the rail,
the rail renders content when possible, and file actions such as download live in
the rail header.

The first slice intentionally avoids URL routing. Detail rail state is owned by the
chat surface, so navigating away from chat unmounts the rail and discards its state.

## Goals

- Let users read Markdown artifacts in Noema without needing an external Markdown
  viewer.
- Keep artifact cards calm: clicking a viewable artifact opens the rail instead of
  downloading immediately.
- Keep download as a secondary action in the rail, not the primary transcript
  affordance.
- Establish a generic detail rail contract that can later support tool results,
  memory provenance, tasks, citations, and other contextual surfaces.
- Avoid editor affordances, version management, route persistence, and broad file
  preview scope in the first slice.

## Non-Goals

- No Markdown editing, title editing, comments, diffing, or save workflow.
- No URL query params, deep links, back/forward integration, or restoration after
  refresh.
- No inline transcript expansion for long documents.
- No general-purpose file viewer yet. The only rendered format in this slice is
  Markdown.
- No browser QA requirement unless explicitly requested.

## Detail Ownership

Detail state belongs to `ChatSurface`, not `AppRoot` or the global shell. This
keeps the rail contextual to chat:

```ts
type ChatDetailTarget = {
  type: "artifact";
  version: string;
};
```

`ChatSurface` owns `detailTarget` and clears it when the rail closes. Navigating to
settings, memory, or any future non-chat surface unmounts `ChatSurface`, so the
detail rail disappears naturally.

## UI Model

The chat surface renders as the transcript/composer area plus an optional detail
rail.

On desktop, the rail appears on the right side of the chat surface. It should feel
like a contextual inspector, not a second navigation sidebar. On mobile or narrow
widths, the same rail state can render as a full-screen panel or sheet.

The rail has a compact header with:

- title
- close action
- download action when the loaded detail has a local download URL

The body renders the selected detail. For the first slice:

- Markdown artifact versions render as read-only Markdown.
- Unsupported or non-Markdown versions show a plain preview-unavailable state with
  download when available.
- Missing or unreadable versions show an error state.

## Transcript Behavior

`Transcript` receives an `onOpenDetail` callback and passes it to
`ArtifactReferenceCard`.

Artifact rows should behave as follows:

- Local Markdown artifact version: click opens the detail rail.
- Local non-Markdown artifact version: click may still open the rail to show the
  unsupported state and offer download.
- External artifact: keep external opening behavior for now; do not route it
  through the local detail rail in this slice.
- Artifact without a version id or linkable content: render disabled/unavailable.

The artifact card should not expose a primary download affordance. Download belongs
in the viewer header.

The card's action affordance sits in its bottom-right corner and uses an icon rather
than text:

- `PanelRightOpen` for opening the chat detail rail.
- `Download` for a direct local download when no detail target is available.
- `ExternalLink` for an external artifact.
- No action icon for an unavailable artifact.

On devices with hover, the icon is hidden at rest and fades in when the card is
hovered or contains keyboard focus. On devices without hover, the icon remains
visible. The card reserves enough inline space for the icon so it cannot cover the
title or description. Each icon has a concise tooltip or equivalent accessible
label, while the whole card remains the interaction target.

## Data Flow

The frontend should fetch artifact-version detail data when the rail opens. The
backend should expose enough information to render or gracefully reject the preview:

- artifact id
- artifact version id
- title
- artifact kind
- storage kind
- media type
- local download URL when available
- Markdown text for renderable local Markdown versions

The GraphQL read should be version-oriented, because the rail target is an artifact
version. The server remains responsible for reading local artifact bytes safely
through existing governed artifact helpers and path checks.

## Rendering

The first renderer is Markdown. It should reuse existing Markdown presentation
patterns where possible, but the detail rail can define its own restrained prose
styles for document reading.

Rendering rules:

- Content is read-only.
- Links inside Markdown should use normal browser behavior and safe attributes where
  appropriate.
- Large Markdown files should remain scrollable inside the rail body.
- If Markdown content cannot be loaded as UTF-8 text, show the unsupported/error
  state rather than attempting binary rendering.

Future renderers can be added behind the same `type: "artifact"` target by
selecting a renderer from media type and artifact metadata after load.

## Error And Unsupported States

- Missing artifact version: show "Artifact unavailable".
- Local file read failure: show a recoverable error and download when the URL is
  known.
- Unsupported media type: show "Preview unavailable" and download when available.
- External artifact: open externally from the card for now.

These states should be quiet and utilitarian. They should not ask the user to edit,
convert, or create a new artifact.

## Accessibility

- The rail close action must be keyboard reachable and labeled.
- Opening the rail should move focus to the rail header or first meaningful control.
- Closing the rail should return focus to the originating artifact card when
  practical.
- On mobile/full-screen presentation, the rail should be announced as a dialog-like
  contextual panel.
- The transcript action icon must also become visible when its card receives
  keyboard focus, and its action must be available without hover.

## Validation

Backend validation:

- GraphQL test for fetching Markdown artifact-version detail content.
- GraphQL or store-level test for unsupported/non-Markdown behavior.
- Existing artifact path safety and symlink protections remain authoritative.

Frontend validation:

- Model/helper tests for identifying viewable Markdown artifact references if a
  helper is introduced.
- `bun run lint`.

Standard Rust validation for the implementation:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --no-fail-fast`

## Open Decisions Resolved

- The rail is generic from day one.
- The first detail target is artifact version, not artifact collection or renderer
  type.
- The first slice has no URL routing.
- State is owned by the chat surface, not app root.
- Download is secondary and belongs in the rail header.
