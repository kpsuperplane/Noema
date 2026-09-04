# Mock validation

Date: 2026-09-04. Updated after spacing, model-detail, list-feedback, and surface-placement review.
Scope: the saved design gallery. These results do not certify the production flows.

## Completed checks

- `node --check mocks.js` passed.
- Chromium opened the gallery directly from disk.
- Network requests were blocked during review. The gallery requested no remote resources.
- All 33 states rendered at desktop, 390-pixel, and 320-pixel preview widths.
- The 99 layout checks found no content outside the preview width and no text overflow.
- Provider cards stay within the content width and use the Task list’s six-pixel gap.
- The expanded model form also fit a physical 320-pixel browser viewport.
- All action targets and documented screen links resolved.
- Keyboard activation opened recovery and focused the code input.
- Returning to sign-in focused the passkey action.
- Browser Back restored the previous screen.
- Native connection denial displayed the no-new-access result.
- Model customization exposed all nine assignments.
- Changing an assignment updated the grouped model summary. The confirmation action remained explicit.
- The Local example kept action reviews as human approval.
- Model review showed resolved example model names and their jobs, without repeated “Recommended” values.
- OpenRouter, Codex, and Local model summaries passed nine focused layout checks across the three preview widths.
- Each provider retained all nine assignments under customization. Changed model choices regrouped the summary immediately.
- Provider cards matched ListCardButton borders, padding, radius, shadow, hover fill, and external keyboard focus outline.
- Provider labels and descriptions matched the Task list’s existing text metrics; descriptions wrapped on phones.
- Enter on the Local row opened local setup. Each row remains one action.
- Human intervention examples, layout code, and their screenshot were removed from the gallery.
- Service callback results remained separate pages. Success copy did not imply that connection policy was complete.
- Expanded model details showed the catalog model name, 5.3 GB size, license, format, runtime, and file.
- Reduced motion stopped the waiting spinner, success check, and avatar animations.
- The avatar used the existing beam component, local agent seed, and Noema palette.
- The full avatar remained visible above the card paint, with an 8-pixel white outline joining the card.
- One parent drop shadow followed the combined silhouette. The avatar and card had no separate box shadows.
- The exposed avatar arc used the same one-pixel `border-subtle` border as the card. Its lower arc stayed hidden.
- Titles, introductions, and setup steps were centered beneath the avatar. Form content stayed left aligned.
- Card corners matched the primary Chat surface: `radius-page` and `corner-shape-page`.
- The page background used the browser app’s existing `--pine-50` token. The logo/text header was absent.
- Blocking the avatar bundle preserved the static avatar and the usable gallery.
- The reviewed sessions reported no uncaught JavaScript errors.
- Heading inspection confirmed the existing 24-pixel, 600-weight semantic heading with normal letter spacing.
- The existing bundled heading, body, and code fonts loaded without external font imports.
- `git diff --check` and the zero-change Rust size budget passed before commit.

## Saved visual references

- [Shared avatar/card surface, desktop](screenshots/proposed-login-desktop.png)

- [Provider card hover, desktop](screenshots/proposed-providers-hover-desktop.png)
- [Provider cards, phone](screenshots/proposed-providers-phone.png)
- [Expanded model details, phone](screenshots/proposed-local-details-phone.png)
- [Setup complete, desktop](screenshots/proposed-complete-desktop.png)
- [Native consent, desktop](screenshots/proposed-consent-desktop.png)
- [Codex device sign-in, phone](screenshots/proposed-codex-phone.png)
- [Local download, desktop](screenshots/proposed-download-desktop.png)
- [OpenRouter model review, phone](screenshots/proposed-models-phone.png)
- [Codex model review, phone](screenshots/proposed-models-codex-phone.png)
- [Local model review, phone](screenshots/proposed-models-local-phone.png)
- [Partial connection result, phone](screenshots/proposed-partial-phone.png)
- [Rejected recovery code, phone](screenshots/proposed-recovery-phone.png)

## Limits

The gallery performs no authentication, API calls, account writes, or model downloads.
It simulates the named states; it does not verify server transitions or provider consent pages.
The native browser passkey prompt is not reproduced.
Screen-reader acceptance and a real mobile keyboard review remain implementation checks.
The non-technical usability session remains an implementation acceptance gate.
Application builds and Rust tests were not run because this unit changes plan files only.
No UI test files were added.
