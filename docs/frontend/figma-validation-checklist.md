# Figma Source-of-Truth Validation Checklist

Use this checklist before Noema treats a Figma file as a design source of truth.

The file passes only when every required item is complete. Record each exception with an owner and resolution date.

## Current validation state

Validation date: 2 September 2026

Named checkpoint: `Structure-weighted native-surface repair audit · 2 Sep 2026`

Noema Git revision: `8b7473c881b8ba410538d1b650f3a85c6e423301`

Figma file: [Noema](https://www.figma.com/design/qjclqND2rnNYLdWV41hcdl/Noema)

| Mechanical gate | Current result | Evidence |
| --- | --- | --- |
| Native product frames | Pass | 36 web desktop, 36 web mobile, 53 iOS iPhone, and 4 iOS system frames. |
| Product frames without instances | Pass | 0 of 129 frames. |
| Screen-sized product raster fills | Pass | 0. Fourteen smaller raster nodes are product assets. |
| Runtime comparisons | Open | 128 direct overlays and 1 documented simulator exception exist. Exact-size metrics cover 125 product pairs. The blurred mean error is 2.69. The median is 2.71. The highest remaining value is 4.67. |
| Component library | Pass | 320 local components in 49 component sets. |
| Component descriptions | Pass | 0 missing component or component-set descriptions. |
| Component section overlaps | Pass | 0 overlaps across the 10 product-library sections. |
| Variables and styles | Pass | 208 variables, 185 text styles, 6 paint styles, and 10 effect styles. |
| Supported token bindings | Pass | 0 undocumented bypasses for supported fills, strokes, type roles, spacing, opacity, or radii. |
| Interactive target sizes | Pass | 0 prototype targets below 24 px on web or 44 pt on iOS. |
| Text contrast | Open | 61 automated candidates require manual classification. Disabled controls and placeholders account for most candidates. |
| Prototype connections | Pass | 454 links across native screens and current flows. |
| Fixtures | Pass | Synthetic ordinary information only. |

The structural audit passes. Human review and exception approval remain open.

The measured repair pass corrected fifty-six individually rebuilt product states. The web scrim token improved 19 comparisons. The shared iOS status component improved 48 comparisons and regressed none.

The shared navigation repair replaced three obsolete iOS glyphs with the repository Lucide assets. It improved 34 of 53 iOS comparisons and regressed none.

The shared identity repair rebuilt the Beam avatar from the current Swift source. It also replaced both web onboarding marks with the current PWA asset. It improved 38 of 41 affected comparisons and regressed none.

The later geometry pass corrected chat text insets, task-reference symbols, and three sheet heights. It reduced seven high-error iOS comparisons by 7.71 points in total.

The shared onboarding repair corrected the iOS heading font, disabled-state opacity, card borders, and model selectors. It improved all four onboarding comparisons.

The current web-mobile repair corrected the task dock, GGUF dialog, Memory article, and Providers catalog.

The Providers catalog now uses one responsive component for all five rows.

The recovery repair restored current text, field help, focus treatment, and disabled actions on both web viewports.

The later iOS repair corrected the shared keyboard shift symbol, device corners, the Chat empty state, and the Agents selector.

The current chat repair corrected the shared web composer across four chat states. It also corrected the mobile MCP setup dialog and empty state.

The shared mobile-web repair applied current navigation color and device corners. It improved 34 comparisons and preserved 2 comparisons.

The current settings repair corrected web Agents typography and three iOS Web cards. It also corrected the iOS Execution selector and disabled model field.

The latest iOS repair corrected the adapter credential roles, password keyboard shift icon, chat microphone states, and client-detail typography.

The latest task repair replaced the obsolete beige task type with the current neutral control token across seven mobile-web states.

The latest setup repair corrected iOS MCP labels, placeholders, disabled actions, and the stored system time.

The latest Agents repair aligned iOS type roles, disabled selectors, disclosure icons, and ACP action placement.

The latest mobile-chat repair matched the three assistant bubble heights and their exact vertical rhythm.

The latest iOS surface repair matched the chat-intervention mask and sheet curvature. It also rebuilt Providers with native text roles and selector geometry.

The latest provider-setup repair reused the current mobile detail hierarchy. It also matched modal labels, fields, borders, and actions.

The latest Task-sheet repair applied the shared web scrim token. It also corrected the mobile header, close action, edit action, and transcript bubble.

The latest iOS task repair corrected the Cancel Task sheet curvature, native type roles, copy width, and action alignment.

The latest iOS chat repair matched the current device curvature. It also applied the native pine status token to the shared composer action.

The latest iOS client repair matched the current device curvature in both client-detail states.

The current device repair matched all 53 iOS product frames to the 48-point runtime curvature. The 45 changed comparisons improved, and none regressed.

The same pass corrected the native provider-choice title face and eyebrow position.

The latest state repair added the captured project-detail tooltip as a reusable component. It also corrected the iOS Agents system time.

The current mobile-web repair matched all 36 product frames to the 24-point runtime curvature. Every comparison improved, and none regressed.

The current desktop repair matched all 36 product frames to the 16-point runtime curvature. It also replaced the obsolete onboarding cards with the live three-row chooser.

The structure-weighted audit rebuilt both desktop Chat states with the current inset shell, message sizing, focused composer, and task suggestion.

The two Chat comparisons reached 87% structural edge overlap. The routed state rose from 39%.

The latest iOS settings repair replaced the obsolete Local Models row with the current device summary, import action, and hardware-fit card.

The Local Models comparison reached 91% structural edge overlap. It began at 54%.

The latest iOS Memory repair matched the live content spacing, system time, status type roles, and two-line article notice.

The iOS Memory root reached 85% structural edge overlap. It began at 43%.

The latest iOS onboarding repair restored the pine system ground, live time, inset surface, and token-bound indeterminate spinner.

The same pass separated all 10 product-library sections. No component section now overlaps another.

The pass also added twenty-five reusable components and three component sets. It added dedicated scrim, avatar, iOS border, disabled-surface, composer-focus, and shell-elevation tokens.

| Repaired surface | Blurred mean error |
| --- | --- |
| iOS Chat conversation | 8.36 to 3.78 |
| iOS Settings agents | 8.36 to 4.08 |
| iOS Settings providers | 8.00 to 2.71 |
| iOS Adapter credential sheet | 7.89 to 4.10 |
| iOS Chat inline intervention | 7.84 to 4.67 |
| iOS MCP setup sheet | 7.83 to 3.45 |
| Web-mobile onboarding welcome | 7.77 to 2.15 |
| Web-mobile advanced GGUF import | 7.88 to 4.59 |
| Web-mobile routed chat | 8.09 to 4.17 |
| iOS Chat intervention sheet | 7.73 to 4.07 |
| iOS Task detail workspace | 6.54 to 2.66 |
| iOS Task detail transcript | 6.04 to 2.16 |
| iOS Task recurrence workspace | 7.69 to 3.82 |
| Web-desktop project editing | 2.29 to 1.92 |
| Web-mobile project editing | 7.15 to 4.15 |
| iOS MCP connection sheet | 7.13 to 2.13 |
| iOS Memory root | 6.91 to 1.32 |
| Web-mobile Clients | Current 2.85 |
| Web-mobile Pair client sheet | 7.09 to 3.76 |
| Web-mobile Notifications | Current 2.63 |
| Web-mobile Configure APNs sheet | 6.85 to 3.87 |
| iOS Settings Web | 6.85 to 3.39 |
| Web-mobile provider setup sheet | 6.64 to 3.08 |
| Web-mobile Settings Agents | 6.55 to 4.20 |
| iOS API connection sheet | 6.49 to 3.75 |
| iOS Settings Provider detail | 6.30 to 3.12 |
| iOS MCP reauthentication sheet | 6.26 to 2.96 |
| iOS Settings Execution | 6.16 to 3.90 |
| Web-mobile cancel-task sheet | 6.02 to 3.67 |
| Web-mobile Provider detail | 6.00 to 1.85 |
| Web-mobile Settings Execution | 5.94 to 3.05 |
| Web-mobile Chat conversation | 5.77 to 4.01 |
| Web-mobile Settings Web tools | 5.68 to 2.95 |
| Web-mobile Task detail documents | 5.51 to 2.80 |
| Web-mobile Task detail transcript | 5.06 to 1.96 |
| Web-mobile Settings Providers | 5.04 to 3.60 |
| Web-mobile Memory article | 5.00 to 2.52 |
| Web-desktop onboarding recovery | 1.41 to 0.29 |
| Web-desktop onboarding welcome | 1.90 to 0.33 |
| Web-mobile onboarding recovery | 4.87 to 1.44 |
| iOS Chat empty | 4.73 to 2.75 |
| iOS onboarding provider choice | 5.30 to 4.64 |
| iOS onboarding provider authentication | 5.25 to 3.73 |
| iOS onboarding local model download | 5.10 to 3.72 |
| iOS onboarding model confirmation | 5.08 to 3.85 |
| iOS Revoke Client sheet | 5.20 to 4.64 |
| Web-mobile recurrence detail | 5.18 to 3.63 |
| Web-desktop Memory article | 5.68 to 1.83 |
| Web-desktop Memory index through the shared status component | 2.56 to 2.32 |
| iOS Memory article | 5.68 to 3.35 |
| Shared web scrim token across 19 overlay states | Mean 4.98 to 3.15 |
| Shared iOS status component across 53 comparisons | 48 improved; mean change -0.14 |
| Shared iOS detail shell across 5 comparisons | 5 improved; mean change -0.21 |
| Shared provider-choice cards across 2 comparisons | 2 improved; no regression |
| Shared iOS Lucide navigation icons across 53 comparisons | 34 improved; mean change -0.015; no regression |
| Shared Beam avatar and web onboarding app icon across 41 comparisons | 38 improved; mean change -0.024; no regression |
| iOS Runtime debug sheet | 5.36 to 3.73 |
| iOS Discard task sheet | 5.61 to 3.89 |
| iOS Cancel Task sheet | 4.33 to 4.00 |
| Shared iOS keyboard and device corners across 11 comparisons | 10 improved; 1 inactive state preserved by an instance override |
| Shared iOS device curvature across 45 comparisons | 45 improved; mean change -0.34; no regression |
| Shared mobile-web navigation and device corners across 36 comparisons | 34 improved; 2 unchanged |
| Shared mobile-web device curvature across 36 comparisons | 36 improved; mean change -0.023; no regression |
| Shared web-desktop device curvature across 36 comparisons | 36 improved; mean change -0.059; no regression |
| Web-mobile MCP setup sheet | 4.64 to 3.56 |
| Shared iOS keyboard microphone across 11 comparisons | 11 improved |
| Shared web-mobile task type across 7 comparisons | 7 improved |
| iOS Tool Activity row | 5.36 to 4.49 |
| iOS Client detail | Current 2.51 |
| Web-mobile Task detail overview | 4.46 to 2.42 |
| Web-mobile Project detail overview | 4.21 to 3.29 |
| Web-desktop Chat conversation | 2.69 to 1.21 |
| Web-desktop Connect API routed chat | 3.77 to 1.24 |
| iOS Settings Local Models | 3.34 to 1.36 |
| iOS onboarding loading | 1.94 to 0.74 |

The repaired frames use editable layers, token bindings, and reusable components. Runtime fidelity remains open for unresolved visible differences.

The inline-intervention human bubble records one Figma limitation. Figma cannot bind a variable to each individual gradient stop.

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
