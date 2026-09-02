# Figma Source-of-Truth Validation Checklist

Use this checklist before Noema treats a Figma file as a design source of truth.

The file passes only when every required item is complete. Record each exception with an owner and resolution date.

## Current validation state

Validation date: 2 September 2026

Named checkpoint: `Measured runtime-diff audit · 2 Sep 2026`

Noema Git revision: `0df4e5037cc3`

Figma file: [Noema](https://www.figma.com/design/qjclqND2rnNYLdWV41hcdl/Noema)

| Mechanical gate | Current result | Evidence |
| --- | --- | --- |
| Native product frames | Pass | 36 web desktop, 36 web mobile, 53 iOS iPhone, and 4 iOS system frames. |
| Product frames without instances | Pass | 0 of 129 frames. |
| Screen-sized product raster fills | Pass | 0. Twelve smaller raster nodes are product assets. |
| Runtime comparisons | Open | 128 direct overlays and 1 documented simulator exception exist. Exact-size pixel metrics rank 125 directly comparable product pairs. Visible differences still require resolution or approval. |
| Component library | Pass | 293 local components in 46 component sets. |
| Component descriptions | Pass | 0 missing component or component-set descriptions. |
| Variables and styles | Pass | 202 variables, 180 text styles, 6 paint styles, and 9 effect styles. |
| Supported token bindings | Pass | 0 unbound visible fills, strokes, type roles, spacing values, opacity values, or radii. |
| Interactive target sizes | Pass | 0 prototype targets below 24 px on web or 44 pt on iOS. |
| Text contrast | Open | 61 automated candidates require manual classification. Disabled controls and placeholders account for most candidates. |
| Prototype connections | Pass | 454 links across native screens and current flows. |
| Fixtures | Pass | Synthetic ordinary information only. |

The structural audit passes. Human review and exception approval remain open.

The measured repair pass has corrected three high-drift frames. The blurred mean error changed from 8.36 to 6.72 for iOS Chat. It changed from 8.36 to 4.90 for iOS Agents. The routed web-mobile chat now includes the missing suggestion and focus state.

## Review record

- [x] Record the Figma file URL.
- [x] Record the Figma version or named checkpoint.
- [x] Record the Noema Git revision.
- [ ] Record the reviewer and review date.
- [x] Record the tested web and iOS builds.

## Immediate failure conditions

Fail the review when any condition below is true.

- [x] No product frame uses a screen-sized raster image.
- [x] No screenshot replaces editable interface layers.
- [ ] No outdated reconstruction remains on a product page.
- [x] No repeated control uses detached copies when a component exists.
- [x] No product color, spacing, radius, or type value bypasses an approved token without a note.
- [x] No secret or unauthorized private information appears in fixtures.
- [x] No product page claims coverage for an omitted state.

Raster images are permitted only when the product itself displays raster content.

Reference screenshots must live on a clearly named reference page. Exclude that page from product coverage.

## File structure

- [x] Keep one cover page with ownership, revision, and validation status.
- [x] Keep foundations, components, web, iOS, flows, and coverage on separate pages.
- [x] Name pages with a stable numeric order.
- [x] Group frames by product area and platform.
- [x] Use stable names in `Platform / Area / Surface / State` form.
- [x] Keep deprecated work on a clearly marked archive page.
- [x] Remove empty sections, duplicate frames, and abandoned experiments.
- [x] Keep product frames outside component-library sections.

## Variables and styles

- [x] Define semantic color variables for surfaces, text, borders, actions, status, and focus.
- [x] Define spacing variables that match the Astryx spacing scale.
- [x] Define radius variables for controls, panels, dialogs, pills, and full surfaces.
- [x] Define typography variables or styles for every product text role.
- [x] Define elevation variables or styles for raised surfaces.
- [x] Define icon size and stroke variables where Figma supports them.
- [x] Separate primitive values from semantic product variables.
- [x] Bind every supported component property to semantic variables.
- [x] Match variable names with current Noema and Astryx terms.
- [x] Document each unbound value and its product reason.
- [x] Confirm that variable updates propagate through components and screens.

## Component library

- [x] Create components from current product primitives.
- [x] Create component sets for meaningful visual or interaction variants.
- [x] Use component properties for text, icons, state, visibility, and swaps.
- [x] Use nested instances for icons and repeated subcomponents.
- [x] Use instances in every product frame.
- [x] Remove detached instances unless the exception is documented.
- [x] Include default, hover, focus, pressed, disabled, loading, and error states when supported.
- [x] Include compact and standard density only when both exist in the product.
- [x] Include desktop and mobile variants only when composition changes.
- [x] Keep platform-specific components separate when behavior or typography differs.
- [x] Map each shared component to its current source component.
- [ ] Remove component variants that have no current product path.

## Native editability

- [x] Build every screen from frames, text, vectors, shapes, and component instances.
- [x] Keep text as editable text layers.
- [x] Keep icons as vectors or component instances.
- [x] Keep fills, strokes, effects, and typography token-bound.
- [x] Use Auto Layout for repeated rows, stacks, forms, navigation, and dialogs.
- [x] Use constraints or responsive resizing for bounded regions.
- [x] Name important layers by product meaning.
- [x] Keep source order aligned with visual and reading order.
- [x] Confirm that content changes do not break the composition.
- [x] Confirm that a token change updates the complete surface.

## Product hierarchy

- [ ] State the human's job for each distinct surface.
- [ ] Identify one focal action or content area per frame.
- [ ] Put required context before supporting evidence.
- [ ] Keep controls beside the content they change.
- [ ] Use tighter spacing within groups than between groups.
- [ ] Remove decorative containers that communicate no relationship.
- [ ] Keep management surfaces compact and scannable.
- [ ] Keep reading surfaces focused on prose.
- [ ] Keep implementation details behind named disclosure.
- [ ] Preserve the same information order across responsive versions.

## Runtime fidelity

- [ ] Compare every product frame with the same live runtime state.
- [x] Use identical viewport dimensions during comparison.
- [ ] Use the correct platform font and available font weight.
- [ ] Match shell geometry, region sizes, and responsive boundaries.
- [ ] Match spacing, alignment, radii, borders, shadows, and opacity.
- [ ] Match text content, wrapping, truncation, and line height.
- [ ] Match icon source, size, stroke, and optical alignment.
- [ ] Match control state and enabled behavior.
- [ ] Match scroll position when it changes the visible composition.
- [x] Review an overlay comparison for each distinct surface.
- [ ] Resolve every visible difference or record an approved exception.

Do not accept similarity by inspection alone. Use an overlay or difference image at the original frame size.

## Responsive behavior

- [ ] Validate web desktop at its stored reference viewport.
- [ ] Validate web mobile at its stored reference viewport.
- [ ] Validate iPhone at each supported stored reference viewport.
- [ ] Verify shell navigation changes at the current breakpoint.
- [ ] Verify master-detail composition at wide and narrow widths.
- [ ] Verify drawers, sheets, dialogs, and full-screen mobile flows.
- [ ] Verify long titles, long prose, and wrapped labels.
- [ ] Verify minimum and maximum content lengths.
- [ ] Verify horizontal and vertical overflow behavior.
- [ ] Verify keyboard-safe mobile composition when the keyboard affects the flow.

## Screen and state coverage

- [x] Inventory every current route from the route authority.
- [x] Inventory every current detail rail, drawer, sheet, dialog, and popover.
- [x] Inventory every inline edit and disclosure state.
- [x] Inventory every onboarding and recovery state.
- [x] Inventory each empty, loading, error, stale, offline, and unavailable state.
- [x] Inventory each human decision and approval state.
- [x] Inventory each current iOS system surface.
- [x] Link every inventory row to one native Figma frame.
- [x] Link every inventory row to its source file or component.
- [x] Mark unsupported or removed states as excluded with evidence.
- [x] Keep coverage counts derived from visible native product frames.
- [x] Confirm that no coverage row points to a deleted frame.

## Prototype behavior

- [ ] Connect primary navigation between current destinations.
- [ ] Connect controls that open or close temporary surfaces.
- [ ] Connect tabs, disclosures, menus, and segmented controls.
- [ ] Connect the main path through onboarding.
- [ ] Connect task creation, task detail, and task actions.
- [ ] Connect settings lists to current detail surfaces.
- [ ] Preserve back, close, cancel, and dismissal behavior.
- [ ] Use overlays, drawers, and sheets with correct placement.
- [ ] Document behavior that Figma cannot represent.

## Content and information handling

- [x] Use synthetic ordinary information for examples.
- [x] Keep secrets outside Figma.
- [x] Keep unauthorized private information outside Figma.
- [x] Preserve product terms exactly.
- [x] Use realistic text lengths and states.
- [x] Keep identifiers only when the live surface shows them.
- [x] Keep technical details behind the same disclosure as the product.
- [x] Confirm that fixture content does not imply unsupported behavior.

## Accessibility

- [ ] Check text and essential control contrast.
- [ ] Check focus indicators against each surface.
- [x] Check minimum control target sizes for each platform.
- [ ] Check logical reading order from the layer structure.
- [ ] Check that color is not the only state signal.
- [ ] Check text at supported larger accessibility sizes.
- [ ] Check reduced-motion outcomes for animated prototypes.
- [ ] Record approved exceptions with a product reason.

## Design-system maintenance

- [ ] Give each token and component one clear owner.
- [ ] Record the source file for each shared component.
- [ ] Record the last validated Git revision.
- [ ] Document how to add a screen, state, token, and component.
- [ ] Document how to deprecate a component or state.
- [ ] Keep component descriptions current and searchable.
- [ ] Publish the library only after this checklist passes.
- [ ] Revalidate affected frames after each token or component change.
- [ ] Revalidate coverage after each route or state change.

## Final audit

- [x] Count visible native frames by platform.
- [x] Count full-screen raster image fills. The required result is zero.
- [x] Count detached repeated controls. The required result is zero.
- [x] Count unbound supported style values. The required result is zero.
- [x] Count coverage rows without frames. The required result is zero.
- [x] Count frames without runtime comparisons. The required result is zero.
- [ ] Count unresolved visible differences.
- [ ] Record each approved exception below.

| Exception | Reason | Owner | Resolution date |
| --- | --- | --- | --- |
| iOS 27 simulator does not render the compact Dynamic Island after both Live Activity approvals. | The Lock Screen runtime state is verified separately. The Visual QA page overlays the expected native component on the simulator capture. | Engineering | Open. Confirm on a physical Dynamic Island device. |
| The Figma bridge renders refreshed SF Pro glyphs at zero width in some editable iOS text layers. | Existing SF Pro layers remain editable. Affected refreshed layers use Inter and document the difference. | Design Systems | Open. Remove the fallback after the bridge renders SF Pro correctly. |

## Sign-off

- [ ] Product design confirms hierarchy and completeness.
- [ ] Engineering confirms current runtime fidelity.
- [ ] Design systems confirms variables, styles, components, and instances.
- [ ] The reviewer confirms zero screen-sized screenshot layers.
- [ ] The owner approves the Figma file as a source of truth.
