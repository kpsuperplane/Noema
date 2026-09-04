# Adversarial gallery audit

Date: 2026-09-04. Reviewed revision: `6a5dcaa6`.
Mode: adversarial review. No mock or production fixes were applied.
Reviewer: dedicated adversarial agent. The parent checked key source claims and reproduced the history focus issue.

## Assessment

The visual system is coherent. The main remaining issues concern flow accuracy, decision copy, and failure recovery.
The current typography, compact spacing, avatar treatment, and action hierarchy need no general redesign.
Resolve findings 1–5 before treating the gallery as a complete implementation reference.

Priority meanings: P1 blocks an ordinary path; P2 needs correction before implementation; P3 improves clarity or gallery fidelity.

## Findings

### 1. P1: Keep the Codex code visible during sign-in

Screens: `provider-codex` → `provider-waiting`.
References: `mocks.js:108`, `mocks.js:115`.

Selecting **Open Codex sign-in** removes the code and copy control from Noema.
A person who opens the sign-in page before copying must use browser Back to retrieve the code.
The existing `apps/web/src/components/onboarding/AuthAttempt.tsx:48` keeps `attempt.userCode` visible during waiting.
This is a proposed flow defect, separate from the gallery’s intentionally inert external navigation.

Correction: keep the active code and copy control on the Codex waiting screen.
When the person reopens sign-in, retain the same attempt.

### 2. P2: Confirm only the native connection state that is known

Screen: `oauth-connected`.
Reference: `mocks.js:189`.

“Noema Desktop is connected” and “Connection complete” imply usable desktop access.
The server first issues a code and redirects. The desktop callback responds before state verification and token exchange finish.
Sources: `crates/noema-server/src/web/native_oauth.rs:483` and `crates/noema-desktop/src/remote_oauth.rs:95` and `:141`.

Correction: preserve the immediate callback handoff.
Use “Return to Noema Desktop to finish connecting.”
Show confirmed connection inside the app after token exchange succeeds.
Do not insert a required browser button before the existing callback.

### 3. P2: Design the existing model loading and saving failures

Screens: provider sign-in → model review → confirmation.
References: `mocks.js:127`, `mocks.js:135`, `mocks.js:278`.

The gallery covers sign-in expiry but moves directly to model review and successful saving.
Production already supports model loading failure and confirmation failure.
Sources: `apps/web/src/components/onboarding/Onboarding.tsx:358` and `apps/web/src/components/onboarding/ModelSetup.tsx:142`.

Correction: add a loading failure with **Try loading models again**.
Add saving and failed-save variants to model review.
Retain the connected account and model draft after failure. Show success only after saving succeeds.
These are existing product states, not a request for a larger error catalog.

### 4. P2: Explain the provider choice

Screen: `provider-choice`.
Reference: `mocks.js:94`.

The OpenRouter and Codex descriptions explain how to select each provider, but give little reason to choose one.
A newcomer cannot tell what either service provides. Local at least identifies where its model runs.

Correction: use each existing card description for the provider’s main distinction and account requirement.
For OpenRouter, explain the choice of hosted models.
Verify current eligibility before naming the account required for Codex.
Retain the three peer choices and current card layout. Do not invent a universal recommendation or payment claim.

### 5. P2: Explain retained access after recovery

Screen: `recovery-complete`.
References: `mocks.js:87`, `mocks.js:89`.

The note suggests reviewing other passkeys and connected apps, but does not explain that their access remains active.
A person who lost a device may assume that the new passkey replaces earlier access.
The implementation constraint states the correct behavior, but the proposed user screen does not.

Correction: replace the current note with this copy:

> Existing passkeys and connected apps still have access. Review them in Settings.

Use the existing note. No additional panel is needed.

### 6. P3: Remove repeated terminal-state copy

Screens: `oauth-denied`, `recovery-complete`, `provider-complete`, `oauth-connected`.
References: `mocks.js:196`, `mocks.js:85`, `mocks.js:133`, `mocks.js:189`.

Denial repeats the outcome in “No new access was granted” and “This request did not connect the app.”
Recovery repeats success in its title, introduction, and status block.
These repetitions add reading without explaining another consequence or action.

Correction: give each region one purpose: outcome, consequence, then next action.
For denial, consider “Connection declined” with “No new access was granted.”
Remove redundant status copy where the title already confirms the same event.
Preserve the distinction between the current request and earlier grants.

### 7. P3: Restore focus after browser Back

Scope: gallery only.
Reference: `mocks.js:333`.

Reproduction: open login, activate recovery, then use browser Back.
The login screen returns, but `document.activeElement` becomes `BODY`.
The explicit **Back to sign in** action correctly focuses the passkey action.
The parent independently reproduced the lost focus. The next Tab target differed between inspections, so this finding does not depend on it.

Correction: apply the existing destination focus rule when history navigation replaces the screen.
This finding does not establish a production routing defect.

## Coverage

| Area | States | Review focus |
| --- | ---: | --- |
| Passkeys | 5 | Initial claim, cancellation, recovery entry, browser support |
| Recovery | 5 | Code rotation, enrollment, expiry, retained access |
| Model setup | 7 | Provider choice, device code, waiting, drafts, confirmation |
| Local models | 6 | Download information, progress, verification, cancellation, failure |
| Native connection | 5 | Consent consequences, denial, handoff, terminal claims |
| Service callback results | 3 | Authorization versus tool readiness, return actions |
| Unavailable states | 2 | Retry and operator help |

The agent rendered all 33 states at desktop, 390-pixel, and 320-pixel preview widths.
All 99 default-size layout checks passed. The agent visually inspected 14 representative captures across the three widths.
The review included source transitions, keyboard activation, browser Back, and relevant production contracts.
The parent independently checked native exchange ordering, existing model failure handling, and browser Back focus.

## Limits

No credentials were submitted. No providers were called. No models were downloaded.
Actual native handoff was not exercised. Read-only fields and inert terminal buttons are intentional gallery behavior.
The documented OpenRouter-only expiry example is a gallery limitation, not a production recommendation.
Screen-reader acceptance, enlarged text, real phone keyboards, and unassisted user testing remain unverified.
Backend source references use the plan checkout. Recheck their active equivalents after the separate Go migration.
This review adds no tests and changes no product behavior.
