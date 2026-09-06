# Supporting UI implementation

Implemented on `codex/go-server-migration` on 2026-09-05.
The approved gallery remains the design reference. Human intervention cards remain outside this change.

## Ownership

- `SetupFrame` and `SetupCard` serve access, recovery, setup, and startup errors.
- `theme/supporting.css` owns their shared geometry and public page styling.
- `IdentityAvatar` supplies the existing animated Noema avatar.
- `ListCardButton` supplies provider choices. Astryx supplies buttons, inputs, grids, and progress.
- `ModelSetup` groups resolved model names by their jobs. Customize retains all nine assignments.
- `internal/publicpage` renders native consent and provider, adapter, and MCP callback pages.
- Public pages load the current Vite stylesheet through `/assets/supporting.css`.
- Desktop loopback results use that stylesheet and the static avatar fallback.

The public Button markup is generated from Astryx. After an Astryx upgrade, run:

```sh
cd apps/web
bun scripts/render-public-buttons.tsx
```

## Flow corrections

Recovery remains available during native sign-in and PWA reauthentication.
It clears rejected codes, preserves passkey retries, and explains expired recovery access.
Recovery completion retains the native authorization destination.
Browser Back restores passkey focus. Forward restores recovery field focus.
Existing passkeys and connected apps remain authorized after recovery.

Provider sign-in opens the provider page. Reopening uses the existing attempt.
Codex keeps its device code and copy action visible while waiting.
Model loading retries keep the account. Model saving keeps the full draft until setup finishes.
Local setup shows the actual model, file, download size, license, runtime, and download progress.
Error disclosures retain exact diagnostics.

Native callbacks validate state before showing denial or handoff.
A handoff does not claim that token exchange has finished.
Adapter callbacks distinguish saved account access from failed activation.
Public callback return actions preserve the existing safe destination.

## Review and validation

One independent adversarial review found six bounded issues.
The correction pass addressed return actions, focus, denial, partial setup, model details, and browser history.
No UI test files were added.

Manual browser inspection used production components at 1100, 390, and 320 pixels.
It covered access, recovery, provider choice, Codex waiting, model review, customization, and local downloads.
The built application also passed recovery Back/Forward focus checks at desktop and mobile widths.
Public consent and callback pages worked with JavaScript disabled at all three widths.

The avatar centre matched the card edge exactly. Inspected pages had no horizontal overflow.
Desktop paired buttons measured 203 pixels each and 32 pixels high in the 456-pixel card.
Native consent kept equal columns on mobile. Other mobile action pairs stacked in source order.
Representative screenshots are in `screenshots/implementation/`.

Automated validation and patch measurements are recorded in `validation.md`.
