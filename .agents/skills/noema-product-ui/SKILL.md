---
name: noema-product-ui
description: Design, build, audit, or refine Noema product UI with task-first information hierarchy, semantic grouping, restrained density, Astryx spacing tokens, shared surface reuse, responsive behavior, and visual review. Use for frontend work that changes layout, spacing, padding, margins, hierarchy, cards, panels, rails, navigation, information disclosure, responsive composition, or visual prominence in `apps/web`. Do not use for marketing or brand-only pages.
---

# Noema Product UI

Design Noema as a restrained, information-dense product interface. Prevent
component-first layouts, arbitrary whitespace, flat hierarchy, duplicated
surfaces, and overexposed implementation detail.

## Load the contract

Before editing:

- Read `docs/frontend/product-design.md` completely.
- Read `docs/project.md`, `docs/context/current.md`, and the closest surface
  contract.
- Inspect the existing theme, Astryx components, spacing tokens, and the
  nearest comparable Noema surface.
- Preserve unrelated worktree changes. Reuse a current surface or primitive
  when it represents the same concept.

If another design skill is active, this skill governs Noema's product
hierarchy, density, component foundation, and validation. Marketing-oriented
instructions to add large whitespace, decorative cards, bento grids, imagery,
or cinematic motion do not apply.

## Establish the design model

Make this compact working brief before choosing components or CSS. Keep it
private unless the direction needs confirmation or the user asks for the
rationale.

```text
Human: the specific person and context
Job: the verb they came here to perform
Focal point: the one action or content that must win
Now: information required immediately
Next: supporting context required to decide or proceed
Later: evidence and detail available on demand
Omit: implementation detail or duplication that serves no current job
Grouping: why each proposed group belongs together
Density: compact, standard, or reading; why
Reference: the closest existing Noema or real-world product pattern
```

Do not implement until the brief identifies a focal point and an information
order. When the product direction is genuinely ambiguous, show one
recommendation and ask one concise question.

## Compose from meaning

- Rank visible content before laying it out: current action or state, decision
  context, supporting evidence, then internals.
- Keep a control with the object or question it changes. A response field and
  submit action belong with the decision prompt; they are not a detached
  action strip.
- Create a card, panel, divider, or heading only when it communicates an
  independent object, state, action boundary, or navigation landmark.
- Use proximity as semantics. Related items sit closer to one another than to
  the next group. Do not use equal gaps everywhere.
- Default detail rails and constrained task surfaces to one column. Add columns
  only for real comparison or parallel scanning, not to fill width.
- Show the minimum needed for the current job. Keep transparency available
  through a clear disclosure or drill-in instead of flattening every field
  into the default view.
- Remove repeated labels, self-evident headings, decorative icons, duplicated
  status, and metadata that does not change the user's next action.
- Make one element visually dominant. Build hierarchy with order, weight,
  color, and spacing before adding containers, borders, or larger type.
- Keep responsive layouts semantically equivalent. Mobile may change
  composition, but it must preserve the focal action, essential context, and
  logical order.

## Control spacing and density

Use Astryx `spacingVars` or `var(--spacing-*)`; its compact scale starts with
`--spacing-0-5` through `--spacing-6` for 2, 4, 6, 8, 12, 16, 20, and 24
pixels. Prefer these semantic roles:

- `2px`: optical correction only.
- `4px` to `6px`: icon/text and tightly coupled metadata.
- `8px`: ordinary related elements inside a compact component.
- `12px`: distinct subgroups inside one component or dense region.
- `16px`: component padding or separation between sections in a constrained
  surface.
- `24px+`: major page regions only; do not use by default inside rails, cards,
  dialogs, tables, or task management surfaces.

Use a smaller gap within a group and a larger gap between groups. Check for
accumulated whitespace from nested parent padding, child padding, component
defaults, and scroll insets. Put padding inside an animated surface when the
surface must travel edge to edge.

Do not introduce raw spacing values outside the Astryx scale except for a
justified 1–2px optical correction or a deliberate layout dimension. When
touching existing arbitrary values, normalize the affected cluster without
starting an unrelated global rewrite.

## Design dialogs as bounded decisions

Apply the dialog contract in `docs/frontend/product-design.md` before choosing
width, fields, or footer treatment. Write one sentence in this form:

```text
To [immediate outcome], the human needs [minimum input or decision]; dismissing means [consequence].
```

If the sentence needs multiple outcomes, navigation, substantial reference
material, or several stages, use a route, rail, or full-screen flow instead.

- Compose a dialog as header, body groups, owned feedback, then actions. Use a
  subtitle only when it changes how the person acts.
- Order required input before optional enrichment, and disclose advanced or
  conditional choices instead of flattening them into the form.
- Make the primary action label match the immediate outcome. Keep actions with
  the form, and add an explicit Cancel only when abandoning work benefits from
  a separate choice.
- Assign one owner to every header/body, field/field, and body/action boundary.
  Inspect the rendered total so nested primitive padding does not double it.
- Let content determine width and height. Size text areas for likely input,
  keep a short empty form compact, and use a scrollable body or full-screen
  flow when mobile keyboard space makes the standard dialog unusable.

## Implement with existing structure

- Use Astryx props and components first, then Noema domain components, then
  token-backed StyleX. Hand-roll behavior only when the existing foundation
  cannot express it.
- Search for the same semantic object across chat, Work, settings, and detail
  surfaces. Consolidate or extend the shared abstraction rather than
  introducing a second rendering.
- Keep source and visual order aligned. Preserve keyboard, focus, loading,
  empty, error, stale, and narrow-screen behavior.
- Avoid unrelated visual restyling. A small feature should not silently
  redesign the surrounding product.

## Review before completion

Run a static design review before build validation:

- Can the human identify the focal action or content within three seconds?
- Does every visible item support the current job, a needed decision, or an
  explicit drill-in?
- Does each group have a semantic reason, with tighter internal spacing than
  external spacing?
- Are controls attached to the content they affect?
- Did the change reuse the established surface and component language?
- Are heading count, icon count, cards, borders, and metadata lower than or
  equal to what the workflow requires?
- Do long, short, empty, loading, error, and stale states preserve the
  hierarchy?
- Does the same information order survive at mobile width without overflow or
  accidental whitespace?

For nontrivial visual changes, request browser-inspection permission early if
the user has not granted it. When authorized, inspect the real affected flow at
representative desktop and mobile widths and fix hierarchy, grouping, density,
overflow, and responsive composition. A passing build does not prove the
design is correct. When browser inspection is not authorized, say explicitly
that the change received static and build validation only.

Run the project-prescribed frontend generation, lint, and production build
checks. Do not add UI tests unless the user explicitly requests them.
