# Figma Source-of-Truth Validation Checklist

Use this checklist before Noema treats a Figma file as a design source of truth.

The file passes only when every required item is complete. Record each exception with an owner and resolution date.

## Review record

- [ ] Record the Figma file URL.
- [ ] Record the Figma version or named checkpoint.
- [ ] Record the Noema Git revision.
- [ ] Record the reviewer and review date.
- [ ] Record the tested web and iOS builds.

## Immediate failure conditions

Fail the review when any condition below is true.

- [ ] No product frame uses a screen-sized raster image.
- [ ] No screenshot replaces editable interface layers.
- [ ] No outdated reconstruction remains on a product page.
- [ ] No repeated control uses detached copies when a component exists.
- [ ] No product color, spacing, radius, or type value bypasses an approved token without a note.
- [ ] No secret or unauthorized private information appears in fixtures.
- [ ] No product page claims coverage for an omitted state.

Raster images are permitted only when the product itself displays raster content.

Reference screenshots must live on a clearly named reference page. Exclude that page from product coverage.

## File structure

- [ ] Keep one cover page with ownership, revision, and validation status.
- [ ] Keep foundations, components, web, iOS, flows, and coverage on separate pages.
- [ ] Name pages with a stable numeric order.
- [ ] Group frames by product area and platform.
- [ ] Use stable names in `Platform / Area / Surface / State` form.
- [ ] Keep deprecated work on a clearly marked archive page.
- [ ] Remove empty sections, duplicate frames, and abandoned experiments.
- [ ] Keep product frames outside component-library sections.

## Variables and styles

- [ ] Define semantic color variables for surfaces, text, borders, actions, status, and focus.
- [ ] Define spacing variables that match the Astryx spacing scale.
- [ ] Define radius variables for controls, panels, dialogs, pills, and full surfaces.
- [ ] Define typography variables or styles for every product text role.
- [ ] Define elevation variables or styles for raised surfaces.
- [ ] Define icon size and stroke variables where Figma supports them.
- [ ] Separate primitive values from semantic product variables.
- [ ] Bind every supported component property to semantic variables.
- [ ] Match variable names with current Noema and Astryx terms.
- [ ] Document each unbound value and its product reason.
- [ ] Confirm that variable updates propagate through components and screens.

## Component library

- [ ] Create components from current product primitives.
- [ ] Create component sets for meaningful visual or interaction variants.
- [ ] Use component properties for text, icons, state, visibility, and swaps.
- [ ] Use nested instances for icons and repeated subcomponents.
- [ ] Use instances in every product frame.
- [ ] Remove detached instances unless the exception is documented.
- [ ] Include default, hover, focus, pressed, disabled, loading, and error states when supported.
- [ ] Include compact and standard density only when both exist in the product.
- [ ] Include desktop and mobile variants only when composition changes.
- [ ] Keep platform-specific components separate when behavior or typography differs.
- [ ] Map each shared component to its current source component.
- [ ] Remove component variants that have no current product path.

## Native editability

- [ ] Build every screen from frames, text, vectors, shapes, and component instances.
- [ ] Keep text as editable text layers.
- [ ] Keep icons as vectors or component instances.
- [ ] Keep fills, strokes, effects, and typography token-bound.
- [ ] Use Auto Layout for repeated rows, stacks, forms, navigation, and dialogs.
- [ ] Use constraints or responsive resizing for bounded regions.
- [ ] Name important layers by product meaning.
- [ ] Keep source order aligned with visual and reading order.
- [ ] Confirm that content changes do not break the composition.
- [ ] Confirm that a token change updates the complete surface.

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
- [ ] Use identical viewport dimensions during comparison.
- [ ] Use the correct platform font and available font weight.
- [ ] Match shell geometry, region sizes, and responsive boundaries.
- [ ] Match spacing, alignment, radii, borders, shadows, and opacity.
- [ ] Match text content, wrapping, truncation, and line height.
- [ ] Match icon source, size, stroke, and optical alignment.
- [ ] Match control state and enabled behavior.
- [ ] Match scroll position when it changes the visible composition.
- [ ] Review an overlay comparison for each distinct surface.
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

- [ ] Inventory every current route from the route authority.
- [ ] Inventory every current detail rail, drawer, sheet, dialog, and popover.
- [ ] Inventory every inline edit and disclosure state.
- [ ] Inventory every onboarding and recovery state.
- [ ] Inventory each empty, loading, error, stale, offline, and unavailable state.
- [ ] Inventory each human decision and approval state.
- [ ] Inventory each current iOS system surface.
- [ ] Link every inventory row to one native Figma frame.
- [ ] Link every inventory row to its source file or component.
- [ ] Mark unsupported or removed states as excluded with evidence.
- [ ] Keep coverage counts derived from visible native product frames.
- [ ] Confirm that no coverage row points to a deleted frame.

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

- [ ] Use synthetic ordinary information for examples.
- [ ] Keep secrets outside Figma.
- [ ] Keep unauthorized private information outside Figma.
- [ ] Preserve product terms exactly.
- [ ] Use realistic text lengths and states.
- [ ] Keep identifiers only when the live surface shows them.
- [ ] Keep technical details behind the same disclosure as the product.
- [ ] Confirm that fixture content does not imply unsupported behavior.

## Accessibility

- [ ] Check text and essential control contrast.
- [ ] Check focus indicators against each surface.
- [ ] Check minimum control target sizes for each platform.
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

- [ ] Count visible native frames by platform.
- [ ] Count full-screen raster image fills. The required result is zero.
- [ ] Count detached repeated controls. The required result is zero.
- [ ] Count unbound supported style values. The required result is zero.
- [ ] Count coverage rows without frames. The required result is zero.
- [ ] Count frames without runtime comparisons. The required result is zero.
- [ ] Count unresolved visible differences.
- [ ] Record each approved exception below.

| Exception | Reason | Owner | Resolution date |
| --- | --- | --- | --- |
| None |  |  |  |

## Sign-off

- [ ] Product design confirms hierarchy and completeness.
- [ ] Engineering confirms current runtime fidelity.
- [ ] Design systems confirms variables, styles, components, and instances.
- [ ] The reviewer confirms zero screen-sized screenshot layers.
- [ ] The owner approves the Figma file as a source of truth.
