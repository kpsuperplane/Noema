# Shell Surface Visibility Design

## Context

Noema's web shell uses a layered sidebar deck. The sidebar is the ground layer,
and the active route content sits above it as the moving deck. Mobile and
collapsed desktop navigation reveal the sidebar by moving the content deck aside.

Chat currently has a focus race with that motion. The composer wants to focus
when chat becomes ready, but chat does not know whether shell-owned deck motion
has finished. Previous fixes either delayed route navigation or inferred focus
permission from route source. Both approaches leak shell timing into the wrong
layer.

## Goal

Create a formal shell-to-surface API that tells the active surface how visible
it is from the surface's point of view. Surfaces can then run visible side
effects, such as focusing the chat composer, only when the shell says the
surface is fully visible.

## Non-Goals

- Do not add a route lifecycle such as `routePhase`; route entry is not the
  primitive causing the bug.
- Do not add a `visibleEpoch` or similar counter; React effects can respond to
  `visibility` changes directly.
- Do not delay route navigation with arbitrary timeouts.
- Do not make `Composer` know about shell navigation, sidebar state, or deck
  transitions.

## API

Add a small shell surface context near the shell components:

```ts
export type ShellSurfaceVisibility = "visible" | "hiding" | "hidden" | "showing";

export type ShellSurfaceState = {
  visibility: ShellSurfaceVisibility;
};

export function useShellSurface(): ShellSurfaceState;
```

The default context value is `{ visibility: "visible" }`. That keeps isolated
surface tests and future non-shell render paths simple.

`AppShell` provides this context around the route content area only:

```tsx
<ShellSurfaceProvider value={{ visibility }}>
  <div data-slot="shell-route-content">{children}</div>
</ShellSurfaceProvider>
```

The shell header, sidebar, and backdrop are outside the surface context because
they are shell controls rather than routed surfaces.

## Visibility Semantics

Visibility names are from the active surface's point of view:

- `visible`: the content deck is fully covering the surface area and route
  content may run visible side effects such as focus, measurement, and scroll.
- `hiding`: the deck is moving away from full visibility to reveal navigation.
- `hidden`: navigation is open; route content is parked and non-interactive.
- `showing`: the deck is moving back toward full visibility.

This hides the shell implementation details from surfaces. Internally the shell
may derive these states from `navOpen`, deck transition events, reduced-motion
settings, wide-viewport synchronization, or future animation changes.

## Shell State Flow

The deck navigation reducer should continue to own raw navigation state such as
`navOpen` and `sidebarCollapsed`. It should also represent surface visibility or
provide enough transition actions for `AppShell` to derive it.

Expected transitions:

```text
open navigation:
  visible -> hiding -> hidden

close navigation:
  hidden -> showing -> visible
```

If reduced motion is active, transition support is unavailable, or a state
change has no animated deck movement, the shell may settle synchronously:

```text
visible -> hidden
hidden -> visible
```

Only transition events from the content deck itself should settle visibility.
Transition events bubbling from children are ignored.

## Chat Focus Behavior

Chat consumes shell visibility, not route source:

```ts
const { visibility } = useShellSurface();

React.useEffect(() => {
  if (!ready || visibility !== "visible") {
    return;
  }

  composerRef.current?.focus();
}, [ready, visibility]);
```

This handles both known bug paths:

- Home already loaded, then Memory -> Home: chat may already be ready, but focus
  waits until shell visibility returns to `visible`.
- Fresh `/memory`, then Home while chat starts: readiness may flip during
  `showing`, but focus waits until visibility reaches `visible`.

The `Composer` component should not use native `autoFocus`. It should expose a
normal focus path to chat, either via a forwarded ref or a narrowly scoped
callback owned by the chat surface.

## Route Navigation Behavior

Sidebar navigation should update the route immediately and close navigation
immediately. The visibility contract, not route deferral, prevents focus and
other visible side effects from racing the deck animation.

The route content remains non-interactive while navigation is open or moving,
matching the existing pointer-events behavior.

## Edge Cases

- Escape key, backdrop click, sidebar item click, desktop collapse/expand, and
  wide-viewport synchronization all use the same visibility lifecycle.
- A resize from mobile open navigation to a wide viewport should settle to
  `visible`.
- Reduced-motion users should not be forced through artificial timing; the
  state may settle synchronously.
- If a transition event is irrelevant, such as `box-shadow` or a child element's
  transition, it must not advance visibility.

## Tests

Add focused unit tests for:

- Visibility reducer or mapping:
  - opening navigation produces `hiding` then `hidden`;
  - closing navigation produces `showing` then `visible`;
  - reduced-motion or no-transition paths settle directly.
- `AppShell`:
  - route content is wrapped in `ShellSurfaceProvider`;
  - hidden and moving states still keep route content pointer-disabled;
  - child transition events do not settle visibility.
- Chat/composer:
  - `Composer` does not render native `autoFocus`;
  - chat focuses only when `ready && visibility === "visible"`;
  - ready-before-visible and visible-before-ready both focus once visibility
    and readiness are true.

Regression coverage should explicitly model both observed paths:

- Home already loaded -> Memory -> Home.
- Hard reload `/memory` -> Home while chat starts.

## Implementation Scope

The implementation should be limited to the web shell and chat surface:

- Add `ShellSurfaceContext`.
- Extend deck navigation state or helper functions to produce surface
  visibility.
- Wrap route content in the provider.
- Move chat focus behavior out of native composer autofocus and behind shell
  visibility.
- Remove any remaining route-deferral or route-source focus policy from aborted
  experiments.

Generated web assets should be rebuilt after source changes.
