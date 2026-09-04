# Mock validation

Date: 2026-09-04. Updated after spacing, model-detail, list-feedback, and surface-placement review.
Scope: the saved design gallery. These results do not certify the production flows.

## Completed checks

- `node --check mocks.js` passed.
- Chromium opened the gallery directly from disk.
- Network requests were blocked during review. The gallery requested no remote resources.
- All 35 states rendered at desktop, 390-pixel, and 320-pixel preview widths.
- The 105 layout checks found no content outside the preview width and no text overflow.
- The brand band and row hit areas extend into their reserved gutters; these intended insets remain inside the viewport.
- The expanded model form also fit a physical 320-pixel browser viewport.
- All action targets and documented screen links resolved.
- Keyboard activation opened recovery and focused the code input.
- Returning to sign-in focused the passkey action.
- Browser Back restored the previous screen.
- Native connection denial displayed the no-new-access result.
- Model customization exposed all nine assignments.
- Changing an assignment changed the confirmation label to match the custom selection.
- The Local example kept action reviews as human approval.
- Provider rows showed distinct hover and pressed feedback. Keyboard focus showed a solid outline.
- Enter on the Local row opened local setup. Each row remains one action.
- The service reconnect and waiting examples rendered inside the Chat intervention context.
- Service callback results remained separate pages. Success copy did not imply that connection policy was complete.
- Expanded model details showed the catalog model name, 5.3 GB size, license, format, runtime, and file.
- Reduced motion stopped the waiting spinner and success-check animation.
- The reviewed sessions reported no uncaught JavaScript errors.
- Heading inspection confirmed the existing 24-pixel, 600-weight semantic heading with normal letter spacing.
- The existing bundled heading, body, and code fonts loaded without external font imports.
- `git diff --check` and the zero-change Rust size budget passed before commit.

## Saved visual references

- [Provider row hover, desktop](screenshots/proposed-providers-hover-desktop.png)
- [Inline connection request, phone](screenshots/proposed-connection-inline-phone.png)
- [Expanded model details, phone](screenshots/proposed-local-details-phone.png)
- [Setup complete, desktop](screenshots/proposed-complete-desktop.png)
- [Native consent, desktop](screenshots/proposed-consent-desktop.png)
- [Codex device sign-in, phone](screenshots/proposed-codex-phone.png)
- [Local download, desktop](screenshots/proposed-download-desktop.png)
- [Model review, phone](screenshots/proposed-models-phone.png)
- [Partial connection result, phone](screenshots/proposed-partial-phone.png)
- [Rejected recovery code, phone](screenshots/proposed-recovery-phone.png)

## Limits

The gallery performs no authentication, API calls, account writes, or model downloads.
It simulates the named states; it does not verify server transitions or provider consent pages.
The native browser passkey prompt is not reproduced.
The Chat context illustrates placement only. Production must reuse HumanInterventionCard and existing Chat composition.
Screen-reader acceptance and a real mobile keyboard review remain implementation checks.
The non-technical usability session remains an implementation acceptance gate.
Application builds and Rust tests were not run because this unit changes plan files only.
No UI test files were added.
