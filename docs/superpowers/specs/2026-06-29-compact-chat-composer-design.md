# Compact Chat Composer Design

Approved design for making the web chat composer lower profile while preserving
the current send behavior.

## Context

The React web chat composer currently renders a shared `Textarea` with
`rows={3}` and a text `Send` button. The shared textarea primitive has a
minimum height that suits multi-line forms, but the chat surface should feel
more compact until the user actually writes multiple lines.

Noema's first web surface is chat-led, restrained, and utilitarian. This change
keeps that surface focused by reducing the default composer height and replacing
the text send button with a familiar icon-only action.

## Goals

- Render the chat message box as a single line by default.
- Let the message box grow only when the draft contains line breaks.
- Keep `Enter` to send and `Shift+Enter` to insert a newline.
- Replace the visible `Send` text button with an icon-only send button.
- Keep the icon-only button accessible with an explicit label.
- Keep the change local to the chat composer so other textareas do not shrink.

## Non-Goals

- Do not change the shared textarea primitive globally.
- Do not change the GraphQL send flow or transcript behavior.
- Do not introduce a new icon library or design system primitive.
- Do not add browser-based UI inspection for this small tweak unless explicitly
  requested.

## Design

`Composer.tsx` should keep owning the chat-specific behavior. The shared
`Textarea` remains the general-purpose form control, while the composer passes
local sizing classes and `rows={1}` for the chat input. Tailwind override classes
should reduce the chat textarea's minimum height to one-line button height and
cap growth to a reasonable multi-line composer height.

The submit button should use the existing shadcn/base-rhea `Button` primitive
with the existing `icon` size and a `lucide-react` send icon. The button should
retain `type="submit"` and disabled state logic. It should expose an
`aria-label` that continues to distinguish idle and pending states.

The form should continue to align the composer and send control in one row on
desktop and mobile. This keeps the message box compact without introducing an
extra row solely for the send action.

## Testing

Focused component helper tests should cover:

- The submit state still disables unavailable or blank sends.
- The accessible send label changes between idle and pending states.
- The textarea row count is one line by default.
- The textarea class includes a local minimum-height override.

Validation should run from `crates/noema-core/web`:

```bash
bun test src/components/Composer.test.ts
bun run lint
bun run build
```
