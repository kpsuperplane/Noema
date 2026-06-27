# shadcn/ui Foundation Design

## Summary

Noema should adopt shadcn/ui as the foundation for the web UI while keeping
Noema's product architecture and domain components owned locally.

shadcn/ui should provide accessible, local UI primitives: buttons, inputs,
dialogs, sheets, tabs, menus, forms, popovers, command palette pieces, toasts,
and chat-oriented primitives where they fit. Noema should continue to own the
application shell, information architecture, GraphQL data flow, transcript
model, memory/provenance surfaces, approvals, tools, runs, and object detail
patterns.

The goal is to avoid rebuilding commodity UI infrastructure while preserving
Noema's chat-led, object-backed product model.

## Context

The current web frontend is a small React shell in `crates/noema-core/web`.
It uses Apollo Client, GraphQL over `/graphql`, subscriptions over
`/graphql/ws`, and locally owned components such as `Transcript`,
`Composer`, `Onboarding`, and `StatusCluster`.

The frontend IA docs define Noema as chat-led and object-backed:

- Chat is the primary surface.
- Inline activity rows reveal memory, context, tool, approval, and run state.
- Durable product truth belongs to Postgres and object-owned files.
- The frontend displays projections, explains decisions, and submits actions.

The latest shadcn/ui direction includes chat-related components and AI-oriented
examples, while AI Elements provides broader agent/chat patterns. These are
useful inputs, but Noema should not adopt an SDK-shaped UI architecture where
transport, persistence, or model state is implied by a component library.

References:

- shadcn/ui components: https://ui.shadcn.com/docs/components
- shadcn/ui changelog: https://ui.shadcn.com/docs/changelog
- AI Elements overview: https://elements.ai-sdk.dev/

## Decision

Adopt shadcn/ui as Noema's overall web UI foundation.

Use shadcn/ui in three layers:

1. Foundation primitives.
   Local, source-owned UI components generated into the Noema web app and
   themed to the Noema visual language.

2. Noema shell components.
   App frame, chat layout, onboarding shell, side panels, settings surfaces,
   object pages, and inspection layouts built from the foundation primitives.

3. Noema domain components.
   Transcript entries, memory activity rows, context-used panels, provenance
   cards, approval cards, tool-access cards, run timelines, review queues, and
   object detail views. These components express Noema concepts and must not be
   delegated to a generic AI chat template.

## Boundaries

shadcn/ui may own:

- Accessible primitive behavior.
- Keyboard and focus management through Radix-backed components.
- Common controls and layout primitives.
- Visual consistency for buttons, fields, dialogs, sheets, tabs, menus, badges,
  cards, toasts, and command surfaces.
- Chat primitives when they solve local UI problems, especially scrolling,
  message grouping, attachments, and compact message presentation.

Noema must own:

- GraphQL operations, subscriptions, and generated types.
- Conversation replay from `conversation_items`.
- Live turn state from GraphQL subscriptions.
- Memory rows, provenance, policy explanation, access previews, approvals,
  tools, runs, and object detail semantics.
- The chat-led IA and progressive disclosure rules in `docs/frontend`.
- Redaction, permission, and owner/admin inspection boundaries.

AI Elements may be used as a reference for interaction patterns, but should not
be adopted as an architectural dependency unless a later spec proves that it
does not conflict with Noema's GraphQL and Postgres contracts.

## Architecture

The web UI should move toward this structure:

```text
crates/noema-core/web/src/
  components/
    ui/               # shadcn/ui primitives, locally owned
    shell/            # Noema layout and navigation surfaces
    chat/             # transcript, composer, message, activity row components
    memory/           # memory cards, review rows, detail panels
    inspection/       # provenance, context, graph, and admin inspectors
  graphql/            # Apollo client and operations
  generated/          # generated GraphQL schema/types
  styles.css          # Noema tokens and global application styling
```

This split keeps generated or vendored-style primitives separate from Noema
domain components. It also gives each domain area a clear place to grow without
turning the root component directory into a mixed pile of primitives and
product-specific cards.

## Theming

Noema should theme shadcn/ui to match the existing product stance:

- Quiet, polished, information-dense, and work-focused.
- Chat-first rather than dashboard-first.
- Restrained surfaces with clear hierarchy.
- No generic SaaS landing-page treatment.
- No one-note palette or decorative visual noise.

The initial theme should translate the current CSS variables into shadcn-style
tokens rather than replacing Noema's visual identity with a default theme.
Future visual redesigns can happen on top of the foundation.

## Chat And Agent Components

Adopt chat-related shadcn/ui primitives selectively.

High-value candidates:

- Message scrolling behavior for live streaming, restored history, and
  prepended older messages.
- Message and bubble primitives where they improve accessibility or layout.
- Attachment and marker patterns when Noema supports durable artifacts and
  transcript markers.

Do not adopt any component in a way that assumes:

- Vercel AI SDK is the state owner.
- Client-side message arrays are the durable record.
- Provider runtime IDs are product conversation IDs.
- Tool calls, approvals, memory, or agent activity are generic chat metadata
  rather than governed Noema objects.

Noema's `Transcript` should remain a domain component. It can be rebuilt using
foundation primitives, but it should continue to render typed Noema transcript
entries: user text, assistant text, activity rows, structured cards, errors,
and turn completion events.

## Data Flow

The UI foundation does not change Noema's product data flow:

```text
Postgres conversation_items
  -> GraphQL query/replay
  -> Noema transcript model
  -> Noema domain components
  -> shadcn/ui primitives

GraphQL subscription event
  -> transcript reducer
  -> Noema domain components
  -> shadcn/ui primitives
```

The component library should never become a persistence boundary. It should
only render local state derived from Noema's GraphQL contract.

## Migration Strategy

Adopt shadcn/ui incrementally.

1. Add the shadcn/ui toolchain and minimal dependencies needed for primitives.
2. Create `components/ui` and port a small primitive set first: button, input,
   textarea, badge, card, separator, dialog or sheet, tabs, dropdown/menu, and
   toast if needed.
3. Retheme the current chat/onboarding shell with those primitives while
   preserving existing GraphQL behavior.
4. Replace transcript scrolling/message layout with chat primitives only after
   the base UI layer is stable.
5. Use future Noema specs to introduce memory detail, review, inspection, and
   approval surfaces on top of the same foundation.

This avoids a big-bang frontend rewrite and keeps each phase verifiable.

## Error Handling

Component adoption must preserve current user-facing error states:

- Provider auth failures remain onboarding errors.
- GraphQL query and subscription failures remain chat or setup notices.
- Conversation start and send failures remain recoverable transcript notices
  where possible.
- Missing backend support for future surfaces must render unavailable states,
  not fake controls.

shadcn/ui primitives may improve presentation, but they must not hide or
normalize trust-critical errors.

## Testing And Validation

Implementation plans should include:

- `bun run gen:types`
- `bun run lint`
- `bun run build`
- Local web server verification
- Desktop and mobile screenshots for UI changes
- Manual checks for transcript scrolling, composer behavior, onboarding,
  overflow, focus states, and keyboard operation

For each migration phase, verify that GraphQL behavior and replayed transcript
history remain unchanged unless the phase explicitly changes that behavior.

## Non-Goals

This design does not:

- Switch Noema to the Vercel AI SDK.
- Replace Apollo Client.
- Change the GraphQL product API.
- Change Postgres persistence or conversation replay semantics.
- Build the full task, tools, approvals, or multi-agent UI.
- Create a new visual redesign beyond theming the adopted primitives.

## Open Questions For Implementation Planning

- Whether to use Tailwind as shadcn/ui expects, or adapt generated components
  to the existing CSS-variable approach.
- Which shadcn/ui registry components should be added in the first phase.
- Whether chat primitives should be adopted immediately or after base controls
  are in place.
- How much of the current `styles.css` should become design tokens versus
  component-local styling.
