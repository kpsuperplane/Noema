---
name: noema-design
description: Use this skill to generate well-branded interfaces and assets for Noema, either for production or throwaway prototypes/mocks/etc. Contains essential design guidelines, colors, type, fonts, assets, and UI kit components for prototyping.
user-invocable: true
---

Read the README.md file within this skill, and explore the other available files.

If creating visual artifacts (slides, mocks, throwaway prototypes, etc), copy assets out and create static HTML files for the user to view. If working on production code, you can copy assets and read the rules here to become an expert in designing with this brand.

If the user invokes this skill without any other guidance, ask them what they want to build or design, ask some questions, and act as an expert designer who outputs HTML artifacts _or_ production code, depending on the need.

## Quick orientation
- **Voice:** plain-spoken, warm, transparent. Address the user as "you"; the product is "Noema". Sentence case; wordmark lowercase. No emoji. Tool names / commands / versions in mono.
- **Color:** white page background (`#FFFFFF`), warm paper support neutrals, white cards, `#17160F` ink text, **pine** `#1F7A57` primary, **clay** `#D9663F` accent. Avoid cold blue-purple gradients.
- **Type:** Bricolage Grotesque (display), Hanken Grotesk (body/UI), JetBrains Mono (code/agent). Loaded via Google Fonts CDN in `tokens/fonts.css`.
- **Shape:** generous-not-bubbly radii, hairline borders, soft warm-tinted shadows, faint paper grain. Agents = rounded-square pine avatars; people = circular clay avatars.
- **Icons:** Lucide (CDN). Never hand-draw icons.

## Files
- `styles.css` — link this once; it `@import`s all tokens.
- `tokens/` — color, type, spacing, effects, fonts, base.
- `components/<group>/` — React primitives; load `_ds_bundle.js` and read `window.NoemaDesignSystem_3d237e`.
- `Noema Design System.html` — single-page spec / overview of the whole system (live components).
- `guidelines/` — foundation specimen cards.
- `assets/` — logo marks.

## Building an artifact
1. Link `styles.css`.
2. For React components, load React 18 + Babel + `_ds_bundle.js`, then `const { Button, Card, ChatMessage, ToolCall, ... } = window.NoemaDesignSystem_3d237e`.
3. For icons, render Lucide glyphs from the `lucide` CDN global with a small `<Icon name="Send" />` wrapper (PascalCase names) — see `Noema Design System.html`'s gallery for the pattern.
4. Copy `assets/` files you reference; don't hot-link across projects.
