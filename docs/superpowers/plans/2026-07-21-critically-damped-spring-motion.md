# Critically Damped Spring Motion Implementation Plan

**Mode:** Plan only. This document does not authorize implementation by itself.

**Goal:** Replace Noema's finite web animations and transitions with one coherent
family of three critically damped springs while preserving accessibility,
interaction semantics, dense product hierarchy, and continuous status signals.

**Observable outcome:** Navigation, rails, disclosures, transcript arrivals,
rolling labels, layout changes, hover feedback, and Astryx layer entrances share
the same monotonic spring response. Interrupted motion continues from its current
position and velocity instead of restarting a fixed-duration easing.

**Architecture:** Add Motion for React as the finite-motion runtime, expose three
shared physics presets from one Noema module, and provide sampled CSS `linear()`
equivalents for StyleX and Astryx. Migrate one vertical surface at a time, deleting
the current keyframes, duration constants, timeouts, and transition-end state
machines as their final consumers disappear.

---

## Product Motion Contract

Motion explains a state change, preserves spatial continuity, or provides immediate
interaction feedback. It must not compete with chat content, decisions, task state,
or the current focal action.

Every finite state-to-state animation uses exactly one shared spring:

| Preset | Mass | Stiffness | Damping | Approx. 95% response | Intended scale |
| --- | ---: | ---: | ---: | ---: | --- |
| `micro` | 1 | 1568.16 | 79.2 | 120ms | Movement within one compact component |
| `standard` | 1 | 696.96 | 52.8 | 180ms | A component changing presence, size, or state |
| `surface` | 1 | 392.04 | 39.6 | 240ms | A surface changing screen composition |

For a damped spring, `dampingRatio = damping / (2 * sqrt(stiffness * mass))`.
Each pair therefore has a damping ratio of exactly `1`: the fastest return to
equilibrium without overshoot.

- Use `micro` for chevrons, pressed states, hover/focus feedback, compact status
  text, and controls moving within roughly one component dimension.
- Use `standard` for disclosures, message arrival, rolling-label width, menu-level
  replacement, cards entering or leaving, and ordinary component layout changes.
- Use `surface` for the shell deck, mobile navigation settlement, the chat detail
  rail, and changes that recompose a substantial portion of the viewport.
- A parent and its coordinated children use the same preset unless the child is an
  independent state change. Opacity, color, shadow, and geometry describing one
  transition settle together.
- Opening and closing normally use the same preset. A faster exit uses the next
  faster preset instead of custom parameters.

Component-local stiffness, damping, mass, duration, cubic Bézier, or sampled spring
curves are prohibited. A fourth preset requires a demonstrated new physical class
of motion and a product-contract update.

### Non-settling signal exception

Continuous signals have no equilibrium and are not springs. Linear rotation,
glimmers, typing dots, indeterminate progress, and other loops may retain time-based
keyframes. They must respect reduced motion, remain visually subordinate, and share
signal constants when repeated. No route transitions, parallax, ambient motion, or
animation without a state-transition purpose will be added.

---

## Scope and Current Baseline

The July 21 audit found motion across 17 Noema-owned frontend files, with 14 global
CSS keyframes, three StyleX keyframes, and four JavaScript controllers whose cleanup
depends on matching animation durations.

In scope:

- Rolling text width and word replacement.
- Shell deck, sidebar levels, collapse/expand, and mobile swipe settlement.
- Chat detail rail, resize handle, and main-pane space reservation.
- Transcript arrival, disclosures, chevrons, scroll-to-latest, and automatic
  follow-bottom scrolling.
- Finite hover, focus, color, border, shadow, opacity, and filter transitions.
- Astryx finite motion reachable through public theme tokens or `xstyle`.
- Reduced-motion consolidation and removal of stale motion code.

Out of scope:

- New route transitions or decorative animation.
- Changes to information architecture, transcript virtualization authority, shell
  navigation semantics, detail-rail product behavior, or backend state.
- Local forks or patches to installed Astryx files.
- Frontend test expansion, because the repository currently requires static, build,
  and browser validation for UI work rather than new UI tests.

The main cleanup targets are `RollingText.tsx`, `AppShell.tsx`, `ShellSidebar.tsx`,
`ChatSurface.tsx`, `ChatDetailRail.tsx`, `Transcript.tsx`,
`TranscriptBottomFollower.tsx`, and `styles.css`. The task-detail frame selectors
and two task-detail exit keyframes in `styles.css` have no JSX consumer and should
be deleted rather than migrated.

### File-level baseline inventory

The implementation must refresh this inventory before Milestone 1 because active
frontend work may add motion sites after this plan is written.

| Current owner | Current motion | Planned disposition |
| --- | --- | --- |
| `components/RollingText.tsx` | Width transition, two word keyframes, RAF, timeout | Replace with `micro`/`standard` Motion presence and layout |
| `components/shell/AppShell.tsx` | Deck geometry and header transform transitions | Replace with coordinated `surface` Motion values/layout |
| `components/shell/ShellSidebar.tsx` | Menu background transition and 300ms frame retention | CSS `micro` feedback; `standard` presence for frames |
| `components/shell/useShellNavSwipe.ts` | Pointer-following offset and velocity calculation | Preserve recognizer; hand release velocity to `surface` spring |
| `components/ChatSurface.tsx` | Four visual phases and two-RAF entry trigger | Collapse to semantic target state plus presence |
| `components/chatDetail/ChatDetailRail.tsx` | Transition-end focus/unmount coordination | Preserve focus semantics; move visual completion to presence |
| `components/transcript/Transcript.tsx` | RAF/timeout tool-detail presence | Replace with `standard` presence |
| `components/transcript/TranscriptBottomFollower.tsx` | Manual 240ms cubic scroll loop | Replace only the loop with cancellable `standard` spring |
| `components/transcript/TranscriptScroller.tsx` | Scroll-to-latest opacity/transform transition | Replace with `micro` |
| `components/transcript/ToolMarker.tsx` | Opacity, chevron, grid-row, spinner | Springs for finite state; retain linear spinner |
| `components/transcript/ActivityRow.tsx` | Chevron transform | Replace with CSS or Motion `micro` |
| `components/transcript/TranscriptChatBubble.tsx` | Interactive shadow/filter feedback | Replace with CSS `micro` |
| `components/transcript/ArtifactReferenceCard.tsx` | Hover color/border/shadow | Replace with CSS `micro` |
| `components/transcript/TaskReferenceCard.tsx` | Hover color/border/shadow | Replace with CSS `micro` |
| `components/transcript/MultipleChoicePrompt.tsx` | Option color/border feedback | Replace with CSS `micro` |
| `components/chatDetail/task/TaskToolMarker.tsx` | Marker opacity | Replace with CSS `micro` |
| `components/chatDetail/task/TaskStatusBadge.tsx` | Infinite status spinner | Keep periodic; consolidate signal timing |
| `components/settings/ModelPreferenceSelect.tsx` | Selector finite feedback | Replace with CSS `micro` or Astryx token |
| `components/work/WorkViews.tsx` | Card background/border feedback | Replace with CSS `micro` |
| `pages/memoryPageStyles.ts` | Related-card background/border/shadow | Replace with CSS `micro` |
| `components/transcript/TypingMessage.tsx` | Infinite typing dots | Keep periodic and stop under reduced motion |
| `styles.css` | Shell, task, arrival, detail, glimmer, skeleton keyframes | Delete finite rules; retain classified periodic signals |

Astryx also supplies finite entry motion for dialogs, popovers, selectors, tooltips,
and field status, plus periodic spinner/progress motion. Installed package files are
audit inputs, never edit targets.

---

## Technical Design

### Shared runtime authority

Create `apps/web/src/motion/springs.ts` as the only TypeScript authority for the
three readonly physics presets. Keep `mass: 1`; specify physics parameters rather
than `duration` or `bounce`.

Create `apps/web/src/motion/MotionRoot.tsx` to install `LazyMotion`, `domMax`, and
`MotionConfig reducedMotion="user"`. Use slim `m` components consistently. Layout
projection and gesture velocity are central to the accepted shell, rail, and rolling
text outcomes, so the implementation should make one deliberate `domMax` choice
rather than retain parallel custom layout machinery. Record the built bundle delta
before accepting the foundation; stop if the used behavior does not justify it.

### CSS and Astryx representation

Define `micro`, `standard`, and `surface` CSS motion tokens in the Noema theme using
sampled `linear()` curves generated from the approved critical-response equation,
never tuned by eye. CSS owns only elements Motion cannot own cleanly, especially
Astryx internals and simple StyleX feedback. Motion and CSS must not simultaneously
own the same transform or layout property.

For critical damping with `mass = 1`, sample the normalized step response
`1 - (1 + omega * t) * exp(-omega * t)`, where `omega = sqrt(stiffness)`. Use a
common response threshold and sample density for all three curves. The approximate
99% settlement windows are 168ms (`micro`), 251ms (`standard`), and 335ms
(`surface`); committed CSS durations may round those values consistently but may
not be visually retuned per component. Keep the physics tuple and sampled CSS
representation adjacent and document the generation command in comments so a
future preset change updates both representations together.

Override Astryx's public standard easing token only after confirming it governs
finite transitions. Use public `xstyle` for supported exceptions and retain Astryx
periodic timing. Never target generated Astryx classes.

### Presence, interruption, and semantics

Use `AnimatePresence` when exiting content must remain mounted. Replace matching
timeouts and animation-end bookkeeping with presence lifecycle callbacks. Focus,
`aria-hidden`, inertness, pointer events, scroll ownership, and product state remain
semantic component state; animation completion may permit unmounting but does not
become product authority.

Rapid open-close-open sequences, changing labels, navigation during transition,
and swipe reversal must continue from the rendered position without flashing,
duplicating content, waiting for stale timeouts, or invoking stale callbacks.

Use the same shared Motion rest thresholds for every physics preset. Start with
Motion's defaults during the proving slice; if an invisible tail or premature stop
is demonstrated, change thresholds once in `springs.ts` and revalidate all three
presets rather than overriding one consumer.

### Reduced motion

Reduced motion disables transform and layout travel and applies final state
immediately. Opacity may remain only where needed for comprehension; looping
glimmers, dots, and rotations stop. Preference changes must be observed reactively,
and focus, scroll destination, mounted content, and product state must remain equal
with motion enabled or reduced.

---

## Surface Mapping

| Surface or interaction | Preset | Required outcome |
| --- | --- | --- |
| Shell deck and header offset | `surface` | One coordinated compositional response |
| Mobile shell swipe settle/cancel | `surface` | Continue from pointer position and velocity |
| Sidebar root/settings replacement | `standard` | Directional presence without retention timeout |
| Detail rail, handle, main-pane reserve | `surface` | Coordinated presence, focus, and reflow |
| Rolling words / container width | `micro` / `standard` | Preserve stagger; remove cleanup timeout |
| Message arrival | `standard` | Preserve anchoring and one-time arrival identity |
| Tool and activity disclosures | `standard` | Presence/layout without grid-row clocks |
| Chevrons and scroll-to-latest button | `micro` | Shared rotation/visibility response |
| Automatic scroll-to-bottom | `standard` | Cancellable live target; never overshoot bounds |
| Hover/focus finite feedback | `micro` | Sampled CSS spring across visual properties |
| Astryx popovers, selectors, fields, dialogs | `standard` | Public tokens or `xstyle`; no package fork |
| Spinners, progress, glimmers, typing dots | Exception | Retain periodic signal timing; reduce to static |

---

## Milestone 1: Foundation and Rolling-Text Proving Slice

**Expected files:** `package.json`, `bun.lock`, `main.tsx`,
`theme/noema-neutral.css`, `components/RollingText.tsx`, `styles.css`, plus new
`motion/springs.ts` and `motion/MotionRoot.tsx`.

- [ ] Add Motion and the root reduced-motion policy.
- [ ] Define the three exact physics presets and sampled CSS counterparts.
- [ ] Convert rolling words and width to `micro` and `standard` springs.
- [ ] Preserve the screen-reader-only current value and bounded word stagger.
- [ ] Prove rapid replacement does not show stale words or clip width.
- [ ] Delete rolling keyframes, duration constants, generation cleanup, and timeout.
- [ ] Delete orphaned task-detail motion from `styles.css`.
- [ ] Compare production bundle output before and after the dependency.
- [ ] Commit the independently usable foundation and proving slice.

**Gate:** The slice must delete more local motion lifecycle code than it adds outside
the two foundation files. If rolling layout requires more measurement and state than
today, stop and evaluate CSS-token motion before migrating another surface.

## Milestone 2: Shell and Detail Surfaces

**Expected files:** `shell/AppShell.tsx`, `shell/ShellSidebar.tsx`,
`shell/useShellNavSwipe.ts`, optionally `shell/deckNavigation.ts`, `ChatSurface.tsx`,
`chatDetail/ChatDetailRail.tsx`, and corresponding `styles.css` rules.

- [ ] Convert shell deck and header movement to `surface`.
- [ ] Carry swipe release velocity into settlement while preserving horizontal
  intent detection, vertical scrolling, and click suppression.
- [ ] Replace the sidebar frame timeout with directional `standard` presence.
- [ ] Replace detail opening frames and four-state visual controller with `surface`
  presence while preserving target selection and resizing.
- [ ] Preserve focus capture/trap/restoration, Escape, inertness, and desktop reserve.
- [ ] Remove superseded shell/detail CSS and commit the coordinated surfaces.

**Gate:** Open-close-open, interrupted swipe reversal, and detail open-close-open
must never leave stale focus, invisible interactive content, or endpoint snapping.

## Milestone 3: Transcript and Finite Feedback

**Expected files:** `transcript/Transcript.tsx`,
`transcript/RenderedTranscriptEntryFrame.tsx`,
`transcript/TranscriptBottomFollower.tsx`, `transcript/TranscriptScroller.tsx`,
`transcript/ToolMarker.tsx`, `transcript/ActivityRow.tsx`, finite feedback consumers
found by the baseline inventory, and `styles.css`.

- [ ] Convert message arrival to `standard` without changing virtualizer identity,
  measurement, bottom anchoring, or one-time arrival semantics.
- [ ] Replace tool-detail and group disclosure clocks with `standard` presence.
- [ ] Convert chevrons and scroll-to-latest to `micro`.
- [ ] Replace cubic automatic scrolling with a cancellable `standard` spring whose
  target follows the live bottom and remains clamped.
- [ ] Cancel automatic motion immediately on user scroll, wheel, pointer, or touch.
- [ ] Normalize finite hover/focus transitions to the CSS `micro` token.
- [ ] Retain and centralize only approved periodic signal animation.
- [ ] Delete superseded durations, keyframes, reduced-motion patches, and obsolete
  `will-change`; commit the transcript and feedback migration.

**Gate:** Streaming, rapid tool changes, expanded groups, older-history loading, and
user interruption must not cause scroll jumps, repeated arrival, virtual-row
overlap, or inaccessible content.

## Milestone 4: Contract and Final Audit

**Expected files:** `docs/frontend/product-design.md`,
`docs/frontend/current-contract.md`, and `docs/context/current.md` only if the
decision remains active context.

- [ ] Repeat the motion inventory and classify every remaining result.
- [ ] Confirm every finite result references a preset or CSS spring token.
- [ ] Confirm every time-based result is an approved signal, non-motion scheduler,
  or unrelated React transition.
- [ ] Update durable frontend contracts and remove stale compatibility code.
- [ ] Run one read-only adversarial review focused on interruption, accessibility,
  virtualization, performance, and duplicate authority.
- [ ] Fix demonstrated in-scope P0-P2 defects and commit final cleanup.

---

## Acceptance, Performance, and Accessibility

- [ ] Reduced motion before load and when changed at runtime produces immediate final
  geometry, static signals, correct focus, and no delayed unmount.
- [ ] Keyboard navigation, focus rings, traps, Escape, and return focus remain unchanged.
- [ ] Exiting content becomes non-interactive and leaves accessibility traversal when
  semantic state ends, even if visual presence is completing.
- [ ] Desktop, narrow browser, and Tauri preserve information order and focal action.
- [ ] Empty, loading, error, stale, long, and reconnecting states gain no decorative
  entrance sequence.
- [ ] Prefer transform and opacity. Animate layout only for genuine reflow, and do not
  leave permanent layer promotion on transcript rows or large surfaces.
- [ ] Rest thresholds stop invisible spring tails from extending layout or paint.
- [ ] The virtualizer does not remeasure many rows per frame; swipe remains
  pointer-synchronous until release.
- [ ] Record initial JavaScript and route-chunk sizes before Milestone 1 and after
  Milestones 1 and 4; reject a feature bundle larger than the behavior used.

### Browser scenario matrix

| Scenario | Width/runtime | Required observation |
| --- | --- | --- |
| Shell collapse/expand | 1440px browser and Tauri | Deck, header, radius, and shadow read as one `surface` response |
| Shell menu open/close | 1024px and 760px boundary | No content flash or stale surface visibility |
| Touch swipe commit/cancel/reverse | 390px coarse-pointer emulation | Pointer tracking is direct; release preserves velocity and direction |
| Sidebar root/settings/root | Desktop and mobile | Direction is correct; outgoing frame cannot receive focus or clicks |
| Detail rail open/close/reverse | 1440px, 1024px, and 390px | Rail, reserve, and handle stay coordinated; focus returns correctly |
| Detail rail resize | Desktop while transcript streams | Resize remains direct and does not fight a spring |
| Rolling text rapid replacement | Narrow and wide labels | No stale layer, width clipping, or screen-reader duplication |
| New human/assistant message | Bottom-following and scrolled away | Arrival runs once and never steals scroll ownership |
| Tool group expand/collapse | One and many calls | Height remains readable; repeated reversal is interruptible |
| Live scroll-to-bottom | Growing response and composer resize | Target follows the live bottom without overshoot or oscillation |
| Reduced motion | All representative scenarios | Immediate geometry, static signals, identical semantic result |

Test representative long text, empty content, loading, failure, reconnecting, and
stale data rather than relying only on convenient fixtures. Record short before/after
captures for the three preset scales so reviewers judge response consistency rather
than isolated component polish.

Browser inspection is required before implementation can be called complete.
Request permission and inspect desktop/mobile widths, reduced motion, rapid
reversals, long/streaming transcript content, disclosures, shell swipe, and rail
resizing.

### Static motion review

- [ ] Each animation explains a product state change or interaction response.
- [ ] Preset selection follows visual scale, not the component author's preference.
- [ ] Coordinated properties settle with one preset and no independent delay.
- [ ] Finite Motion and CSS declarations never compete on the same property.
- [ ] No layout spring distorts text, icons, radii, shadows, or virtualized rows.
- [ ] Periodic signals remain subordinate and become static under reduced motion.
- [ ] No permanent `will-change`, stale exit layer, hidden focus target, or pointer
  blocker survives completion.

---

## Budget and Stop Conditions

This is a frontend refactor and must be net-negative overall in Noema-owned motion
and lifecycle code, excluding lockfile and generated output.

- Milestone 1 production budget: at most `+180` net lines while establishing the
  two shared authorities. Milestones 2-4 must each be net-negative.
- Test budget: zero new frontend tests. Update existing pure behavior tests only if
  semantic contracts change.
- Allowed new abstractions: `springs.ts` and `MotionRoot.tsx`. Do not add
  component-specific motion wrappers or a generic animation framework.
- Stop if a milestone exceeds its estimate by 50%, patches Astryx internals,
  changes virtualization authority, weakens focus/reduced-motion behavior, or needs
  a fourth preset. Split any milestone that cannot ship independently.

Measure Milestone 1 with `--max-production-net 180 --max-test-net 0
--max-new-tests 0`; use `--require-net-negative` for Milestones 2-4.

## Commit and Rollback Strategy

Ship the four milestones as separate commits on `main`, with generated files and
unrelated work excluded from each commit. Before starting a milestone, record its
base commit for size measurement and rollback.

- Milestone 1 is the architectural rollback point. If the proving slice fails its
  code-deletion, bundle, accessibility, or visual gate, revert it and retain the
  existing motion system; do not leave Motion installed without a proven consumer.
- Milestone 2 may be reverted without restoring Milestone 1 because rolling text is
  independently complete. Do not maintain old and new shell/detail paths behind a
  feature flag.
- Milestone 3 may be reverted independently if transcript virtualization or scroll
  ownership regresses. Keep the accepted shell/detail migration intact.
- Milestone 4 changes contracts and cleanup only after the runtime migrations pass.
  Do not document aspirational completion while old finite timing remains.

After each major commit, replace stale active notes in `docs/context/current.md`
rather than appending milestone history. Git history owns completed execution detail.

---

## Validation and Completion

At each milestone, run from `apps/web`:

```bash
bun run gen:types
bun run lint
bun run build
```

Before each commit, run `git status --short --branch`, `git diff --check`, and
inspect `git diff --cached --stat` plus `git diff --cached --name-status`.

Final inventory:

```bash
rg -n "cubic-bezier|transitionDuration|transitionTimingFunction|animationDuration" apps/web/src
rg -n "setTimeout|requestAnimationFrame|onAnimationEnd|onTransitionEnd" apps/web/src
rg -n "prefers-reduced-motion" apps/web/src
```

Every remaining match must be an approved periodic signal, CSS spring token,
non-motion scheduling requirement, or necessary lifecycle boundary. Completion
requires all finite motion to use `micro`, `standard`, or `surface`; all three to
remain critically damped; accessibility and interaction semantics to be preserved;
continuous signals to be classified; the patch to be net-negative; and no component
to own an arbitrary duration or easing curve.
