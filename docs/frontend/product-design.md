# Noema Product UI Design

Noema is a dense product interface for conversation, decisions, work, memory,
and inspection. Design each surface around the human's current job and reveal
system depth progressively; do not borrow marketing-page defaults such as large
decorative whitespace, repeated cards, bento composition, or motion without a
state-transition purpose.

## Start With The Job

Before choosing components, write a compact design model:

| Question | Required answer |
| --- | --- |
| Who is here? | The specific human and their immediate context. |
| What are they doing? | One verb-led job, such as answer, inspect, compare, recover, or configure. |
| What must win? | One focal action or content block. |
| What is needed now? | State and context required to act correctly. |
| What can wait? | Evidence, history, metadata, and internals available through disclosure. |
| What should be omitted? | Duplicate state, implementation detail, and information that cannot change the next action. |

If the surface has several equally prominent jobs, its hierarchy is unresolved.
Split the flow, choose a default, or demote secondary work before implementing.

## Information Order

Rank content before laying it out:

1. **Act now:** a human decision, primary action, blocking error, or the object
   the user opened.
2. **Understand:** the minimum context needed to take that action safely.
3. **Verify:** evidence, history, provenance, and secondary state.
4. **Inspect:** raw metadata, model/runtime detail, identifiers, and audit data.

This order changes by surface. A task detail with an open human gate should lead
with the question and its response controls; the same task without a gate should
lead with status and the original request. Do not preserve a static visual order
when the user's job has changed.

Progressive disclosure is not concealment. Keep exact evidence and internals
reachable through a clear expansion or drill-in, while the default view answers
the current user question without making them parse the entire system record.

## Grouping And Hierarchy

Spacing and containers assert relationships, so every group needs a semantic
reason.

- Keep controls with the content they affect. Response controls belong with the
  prompt; version controls belong with the artifact version; filters belong
  above the results they change.
- Use a card for an independent object, state, or action boundary. Do not wrap
  every section in a card or nest cards solely to create visual texture.
- Add a heading only when it helps navigation or distinguishes a non-obvious
  region. Omit labels such as “Status” or “Details” when the content already
  communicates that role.
- Use one column in narrow rails and sequential detail flows. Use multiple
  columns for true comparison or parallel scanning, not to consume available
  width.
- Keep one focal point per view. Demote supporting content with order, weight,
  color, and proximity before reaching for borders, icons, or larger type.
- Prefer the existing Noema representation of a concept. Chat markers, task
  transcripts, detail rails, semantic actions, and Astryx controls should not
  acquire parallel visual languages on different routes.

The proximity test is simple: items within a group must sit closer together
than the group sits to its neighbors. Equal vertical gaps between every element
flatten hierarchy and create accidental groupings.

## Density And Spacing

Noema defaults to productive density. Reading surfaces may breathe more;
management, settings, dialogs, rails, tables, and task surfaces should remain
compact enough to scan and act without excessive scrolling.

Astryx already provides a tokenized spacing scale through `spacingVars` and
`var(--spacing-*)`:

| Tokens | Space | Typical role |
| --- | --- | --- |
| `--spacing-0-5` | 2px | Optical correction only. |
| `--spacing-1`, `--spacing-1-5` | 4–6px | Icon/text pairs and tightly coupled metadata. |
| `--spacing-2` | 8px | Related elements inside a compact component. |
| `--spacing-3` | 12px | Distinct subgroups inside one component or dense region. |
| `--spacing-4` | 16px | Component padding or section separation in a constrained surface. |
| `--spacing-6` and above | 24px+ | Major page regions, used deliberately rather than as a default. |

Use tokens instead of raw values. A July 2026 snapshot found 17 distinct raw
numeric `gap` values and 13 distinct raw numeric `padding` values in the web
TypeScript while only three spacing declarations referenced Astryx spacing
variables. That freedom made every component locally plausible but the product
globally inconsistent.

Audit accumulated whitespace, not isolated declarations. A visually excessive
gap often comes from parent padding, child padding, component defaults, and a
scroll inset adding together. Animated or sliding surfaces should own their
internal padding so the moving plane can reach its visible edges without an
invisible border.

## Component And Layout Decisions

Use this order:

1. Existing Noema domain component or shared surface.
2. Astryx component and its supported props or variants.
3. Token-backed StyleX composition.
4. New custom behavior only when the first three cannot express the interaction.

Backend availability is not a presentation requirement. Do not show every
returned field, action, or history record by default. Likewise, component
availability is not a design reason: an icon, badge, accordion, dialog, grid,
or card belongs only when it improves the current task model.

Keep mobile semantically equivalent to desktop. Composition may change, but
the focal action, essential context, and information order must survive. Test
long and short content, not only the convenient fixture that inspired the
change.

## Motion

Motion communicates a finite state change; it does not decorate stable content.
Use the shared critically damped presets from `apps/web/src/motion/springs.ts`
and their sampled CSS tokens instead of local durations or easing curves:

| Preset | Physics | Use |
| --- | --- | --- |
| `micro` | mass 1, stiffness 1568.16, damping 79.2 | hover feedback, chevrons, and compact visibility changes |
| `standard` | mass 1, stiffness 696.96, damping 52.8 | replacement, arrival, disclosure, and automatic scrolling |
| `surface` | mass 1, stiffness 392.04, damping 39.6 | shell composition, swipe settlement, rails, and meaningful reflow |

A finite response gets exactly one preset. Preserve pointer position and
velocity when a gesture hands control to a spring, and let a new state interrupt
the current response from its current value. Do not add timeouts, retained
render states, or animation-end state machines when Motion presence can own the
visual lifecycle. Exiting interactive content becomes inert and leaves the
accessibility tree when semantic state ends.

Periodic indicators remain time-based because their cadence communicates
ongoing work: spinners, typing dots, progress signals, and glimmers are the
approved exceptions. They stop under reduced motion. Reduced motion also sets
sampled spring durations to zero and applies final geometry immediately without
changing focus, scroll destination, mounted product state, or information order.

Keep virtualizer positioning transforms outside animated descendants. Automatic
scrolling uses the shared monotonic `standard` spring against the live clamped
bottom and yields immediately to wheel, pointer, touch, or keyboard ownership.

## Review Gate

Before calling a visual change complete, answer these questions:

- Can a person identify the focal action or content within three seconds?
- Does every visible item support the current job, a safe decision, or a clear
  drill-in?
- Are controls attached to the object they affect?
- Are related elements visibly closer than unrelated groups?
- Did the design reuse the established Noema surface and component language?
- Can any heading, icon, card, border, column, or metadata row be removed
  without losing meaning? If so, remove it.
- Do empty, loading, error, stale, long-content, and narrow-screen states keep
  the same hierarchy?
- Is transparency preserved without making raw internals the default view?

Run generated-type, lint, and production-build validation. For nontrivial
visual work, request browser-inspection permission when it has not already been
granted and compare the real affected flow at representative desktop and mobile
widths. If browser inspection is not authorized, report that the result was not
visually verified rather than treating a passing build as design validation.
