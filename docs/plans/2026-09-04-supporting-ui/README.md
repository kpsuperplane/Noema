# Supporting UI polish plan

Status: proposed implementation. The mocks are complete design references, not production behavior.
Date: 2026-09-04. Source baseline: `main` at `53b15734`.

## Review the mocks

Open [the gallery](index.html) in a browser. An editor file link can show source instead of the rendered page.
For browser review, serve this directory with `python3 -m http.server 8765 --bind 127.0.0.1`.
Then open [the rendered gallery](http://127.0.0.1:8765/). No build or remote resources are required.
Use the Flow and State controls to inspect each example.
Use Width to compare desktop, phone, and narrow layouts.
The address fragment identifies a screen. Browser Back restores the previous screen.
Buttons change example states. Credential fields are read-only. `DEMO-CODE` is inert sample data.
Model names, transfer values, accounts, and service names are examples unless identified as live evidence.
The gallery consolidates the earlier login, recovery, model review, and callback concepts.
The [adversarial audit](adversarial-audit.md) records seven open UX, copy, and flow findings. Resolve its implementation findings before applying the plan.

| Flow | Entry | Included states |
| --- | --- | --- |
| Passkeys | [Sign in](index.html#access-login) | Sign in, initial claim, browser prompt, cancellation, unsupported browser |
| Recovery | [Enter code](index.html#recovery-code) | Code entry, rejection, enrollment retry, expired setup session, restored access |
| Model setup | [Choose provider](index.html#provider-choice) | Provider choice, OpenRouter, Codex device code, waiting, expiration, model review, completion |
| Local models | [Before download](index.html#local-ready) | Download choice, progress, verification, installed, failed, unavailable |
| Native app connection | [Approve connection](index.html#oauth-consent) | Consent, connected, denied, expired, invalid request |
| Service callback results | [Sign-in complete](index.html#service-connected) | Browser callback: sign-in complete, tools unavailable, failed |
| Unavailable states | [Server unavailable](index.html#system-unavailable) | Server authentication unavailable, fatal app load failure |

## User direction

- Keep the existing design system, including fonts, semantic type sizes, weights, and line heights.
- Improve the supporting flows without redesigning Chat, Tasks, Memory, or Settings.
- Make the interface polished, restrained, and understandable for a non-technical person.
- Remove the logo/text header. Center the existing animated Noema avatar above the card, as part of the same raised surface.
- Use the existing list components for provider choices. Human intervention cards and their flows are outside this plan.
- Show concrete model details. Do not substitute a general memory-fit statement for the model identity.
- Save these designs for later application. Do not implement product changes in this unit.

## Design model

The person wants to gain access, connect a service, or finish setup.
The focal point is the next required action or the current waiting state.
Show the consequence needed for a safe decision before that action.
Keep technical evidence reachable through disclosure.
Use one column for sequential work. Group labels, fields, feedback, and their actions together.
Use the existing setup frame, connection surfaces, and Astryx controls during implementation.

## Visual treatment

- Use a compact white surface on `--pine-50`, the existing browser app background wash.
- Use a 456-pixel surface.
- Omit the server address below the card.
- Keep the card and wash on phones. Use 12-pixel outer insets and 16-pixel horizontal card padding.
- Center the avatar and heading group, including the title, introduction, and setup step. Keep form content left aligned.
- Keep subheadings to one short sentence that fits one line at the normal card width.
- Put necessary instructions and consequences beside their controls. Keep consent warnings explicit.
- Center standalone supporting notes, including the text below action buttons. Keep field labels and detailed form content left aligned.
- Use `text-wrap: pretty` for centered text. Allow wrapping on narrow screens or with larger text; do not truncate it.
- Show the full 80-pixel avatar with an 8-pixel white outline that joins the card. Do not hide it behind the card.
- Give the exposed avatar outline the card’s same thin `border-subtle` border. Hide the lower arc inside the card.
- Give the combined avatar/card outline one shadow. Neither element casts a separate shadow onto the other.
- Reuse `IdentityAvatar` with the local agent seed, existing palette, and idle animation. Do not redraw the character.
- Keep errors calm. Reserve the success check for confirmed completion.
- Use existing `text-heading-1`, `text-body`, `text-label`, and `text-supporting` roles.
- Use the existing font assets. Do not add font imports, custom tracking, or another type scale.
- Match `AppShell.contentDeckPrimary`: use `radius-page` and `corner-shape-page` for the card.
- Match `shadow-shell-frame` for the combined silhouette: pine at 16% opacity, with zero offset and the same blur width.
- The preview converts the 24-pixel box-shadow blur to a 12-pixel drop-shadow deviation. This preserves the compound outline.
- Keep `radius-element` for controls.
- Assign one spacing owner per boundary: 6 pixels inside the header, 12 between body groups, and 16 before primary actions.
- Keep label/control gaps at 6 pixels. Do not add child margins to a parent stack gap.
- Use 16 pixels between the header and body. Use 12-pixel status and detail padding, and 8-pixel fact gaps.
- Reserve 44 pixels above card content: 32 for the avatar overlap and 12 for clearance.
- Use 20-pixel desktop side and bottom insets. Use 16-pixel phone side and bottom insets.
- Use `ListCardButton` from `components/ListCardLink.tsx` for provider choices. It shares the Task card frame.
- Match Task card borders, white fill, small shadow, 8/12-pixel padding, 6-pixel list gaps, and 13/12-pixel text metrics.
- Reuse its hover border/background and external focus outline. Do not add the earlier inset ring or moving chevron.
- Keep descriptions wrapped and each card a single action.
- Keep model name, download size, and computation location visible. Disclose license, format, runtime, and exact file.
- Give one action primary emphasis. Keep alternatives quiet and access consequences visible.
- Use the actual Astryx `Button` with `size="md"` and its standard 32-pixel height. Do not override button padding or visuals.
- Put paired desktop actions side by side, with equal widths.
- Put the secondary action first and the primary action second in both visual and keyboard order.
- Keep related provider actions in one group. Stack paired actions on phones, using the same standard Astryx size.
- Native consent keeps its existing two-column phone decision layout.
- Use the shared motion presets. Keep loading geometry stable and preserve focus through transitions.
- Honor reduced motion in the existing avatar and other feedback. Do not add a separate bobbing or bouncing animation.
- Keep the avatar outside keyboard navigation. It does not communicate authentication or download state.

## Audit evidence

The live audit covered login, recovery, native authorization errors, and malformed integration callbacks.
It used desktop and phone viewports, plus a 320-pixel reflow check.
Setup and successful authentication paths received source review, not authenticated live acceptance.
No credentials were submitted. No account, grant, or recovery code was changed.

| Finding | Evidence | Proposed correction |
| --- | --- | --- |
| Login actions had equal emphasis and 32-pixel height. | [Live login](screenshots/live-login.png) | One primary action and quieter recovery; retain the standard Astryx button size after user review |
| Login used an undefined heading font alias. | `apps/web/src/auth/AuthGate.tsx` | Use the current semantic heading component and tokens |
| Recovery repeated configuration and rotation instructions. | [Live recovery](screenshots/live-recovery.png) | One consequence notice, field help, clear operator guidance |
| Native sign-in recovery did not open. | `/?native_authorization=resume`; `AuthGate` forces the login state | Permit the recovery subflow while preserving the native return destination |
| Returning from recovery lost keyboard focus. | Browser keyboard inspection | Restore focus to the passkey action |
| Callback results used default browser styling. | [Live callback](screenshots/live-callback.png) | Consistent branded result pages for every terminal result |
| Model review exposed nine assignments immediately. | `components/onboarding/ModelSetup.tsx` | Summarize defaults; retain all assignments under customization |
| Provider copy assumed prior knowledge and described the host as a Mac. | `components/onboarding/Onboarding.tsx` | Explain account requirements and identify the actual computation host |
| Device authentication lacked a code-copy control. | `components/onboarding/AuthAttempt.tsx` | Name the destination; group code, copy, waiting, and cancellation |

## Contract constraints

- Preserve passkeys as browser authentication. Do not add passwords, email links, or another recovery credential.
- Keep initial passkey claim separate from recovery. Initial claim needs no recovery code.
- Keep recovery codes out of URLs, model context, logs, browser storage, exports, and API read models.
- Production code entry remains a password field with paste support.
- Every parsed recovery attempt replaces the code, including an incorrect attempt.
- On rejection, clear the field and direct the person to the current server configuration code.
- Code acceptance starts enrollment immediately. Keep an enrollment retry action if the browser prompt is cancelled.
- A recovery setup session lasts five minutes. Its expiry requires a new recovery attempt.
- Recovery does not remove old passkeys or revoke earlier native grants.
- Native consent must disclose complete Noema access before approval.
- Preserve recent passkey verification, CSRF, PKCE, origin checks, and validated callback destinations.
- Use saved attempt state to distinguish denial, expiration, failed activation, and successful connection.
- Service setup and human intervention flows are outside scope. Only browser callback results remain in this plan.
- Callback return actions preserve their existing destinations; this plan does not redesign those destinations.
- Keep successful sign-in separate from successful tool discovery. A partial result is not fully connected.
- Use exact attempt events and the existing foreground query. Do not add polling.
- Keep API-key, local, browser OAuth, device-code, credential, and no-auth paths where currently supported.
- Group model review by resolved model name and show its jobs. Update the summary from the current draft.
- Resolve automatic recommendations through provider profiles; keep automatic selection mode unless the person changes it.
- Never use repeated “Recommended” labels as the model summary.
- Keep all model assignments and explicit confirmation. Local action reviews remain human approval where required.
- Use host-selected model fit, transfer size, progress, and diagnostics. Do not promise unsupported resume or timing.
- Keep authenticated PWA offline behavior and existing local error boundaries.
- Recovery still needs server-file access. Give non-technical users an explicit operator-help path.

## Implementation order

Apply one unit at a time. Each unit must leave its complete user path usable.
Estimates below are maximum net additions, not targets. Reassess against current code before implementation.

| Unit | Existing authority to reuse | Estimate | Acceptance risk |
| --- | --- | --- | --- |
| 1. Access and recovery | `auth/AuthGate.tsx`, `components/shell/SetupFrame.tsx`, current theme, `components/shell/AppBootBoundary.tsx` | +160 production lines; no new UI tests | Recovery must work during native sign-in and PWA reauthentication; preserve session rules and focus |
| 2. Connection pages | `components/onboarding/AuthAttempt.tsx`, server callback and consent renderers | +220 production lines; up to one server unit regression test | Every result must state the true access outcome and return safely |
| 3. Setup | `components/onboarding/Onboarding.tsx`, `components/onboarding/ModelSetup.tsx`, `components/ListCardLink.tsx` (`ListCardButton`), existing model preference selectors | +120 production lines; no new UI tests | Defaults remain drafts; all assignments remain editable; local and cloud paths remain complete |

Paths above are relative to `apps/web/src` unless identified as server files.
At this baseline, server renderers live in `crates/noema-server/src/web/router.rs` and `native_oauth.rs`.
A separate Go migration is active. Recheck the production renderer before applying unit 2; do not duplicate renderer work across retired implementations.
Extend the existing setup frame and reuse `components/IdentityAvatar.tsx` in the application. Keep public result renderers independent.
Do not require the authenticated SPA to render a public callback or fatal startup error.
Keep server result pages usable independently of the app bundle. Render a static avatar when animation code is unavailable.

## Validation and stop conditions

For this plan, inspect all example states at desktop, 390-pixel, and 320-pixel widths.
Check keyboard navigation, direct links, browser Back, disclosure, progress, and reduced motion.
Check the saved files without network access. Record the result in [validation.md](validation.md).
No production build or Rust tests are needed for these isolated plan files.
For implementation, run `bun run check:generated`, `bun run lint`, and `bun run build` from `apps/web`.
For Rust changes, run the repository Rust gates and focused unit checks through `cargo validate`.
Recheck the active backend’s validation rules if the Go migration has completed.
Use manual browser acceptance for UI risks. Do not add UI tests without a separate request.
Require a non-technical person to complete setup and recover from a cancelled or expired sign-in without coaching.
Stop if the work needs new credential mechanisms, new model routing rules, or broader core-UI changes.
Stop if a unit exceeds its estimate by 50% or 500 lines, whichever is smaller.
Commit each completed unit separately. Revert that unit’s code commit to restore its previous presentation.
This plan requires no database migration or persistent schema change.

## Saved assets

`index.html`, `mocks.css`, and `mocks.js` are the editable standalone gallery.
`assets/noema-theme.css` copies the existing theme without its original font-face paths.
`assets/astryx.css`, `astryx-reset.css`, and `astryx-neutral.css` copy the installed Astryx 0.1.9 styles.
`render-buttons.jsx` renders the real Astryx `Button` to static templates in `button-templates.js`.
The gallery uses those templates with Noema’s theme, including its clay primary accent. No parallel button CSS remains.
The three font files, their licenses, and `assets/noema.png` copy the existing Noema assets without modification.
The local-model example comes from `crates/noema-providers/resources/local-models/catalog.toml` at the source baseline.
It shows Gemma 4 E4B IT, its 5.3 GB Q4_K_M build, Apache-2.0 license, and Metal runtime.
The model review examples also use `crates/noema-providers/src/recommendations.rs` from the same baseline.
These are source examples, not claims about the live server’s selected models or current provider availability.
Other mock form controls use native HTML. The avatar uses the existing `@kpsuperplane/boring-avatars` 0.1.6 component.
`assets/avatar-preview.jsx` copies the local agent identity settings from `IdentityAvatar`; it does not define new animation behavior.
`avatar-preview.js` bundles that entry with installed React dependencies using Bun’s production browser build.
`avatar-still.svg` is a static render of the same component. Dependency licenses are included beside these files.
These assets support offline review. Reuse production components instead of shipping this preview bundle.
Use production components and current tokens when applying the plan. Do not copy snapshot CSS into the application.
Only light appearance is specified. A new dark theme is outside this plan.
