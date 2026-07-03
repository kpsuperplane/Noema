# Noema Design System

> **Noema** is a friendly but powerful, open-source **personal agent platform**. It runs on your own machine, is transparent about everything it does, and is built with evident care. The brand has to feel four things at once: **casual, helpful, secure, and thoughtfully built** — approachable enough for a first-time user, credible enough for the developer who's reading the source.

The name comes from phenomenology — a *noema* is the object of a thought. Fittingly, Noema turns your intentions into agents that act, while always showing their reasoning.

This directory is a prototype/reference design kit for Noema's visual
language: proposed tokens, fonts, reusable demo components, and the standalone
spec page. It is not the production component source.

Production web UI lives in `crates/noema-core/web` and currently uses Astryx as
the component foundation, a Noema-owned Neutral-derived theme, and StyleX for
Noema-specific layout and state styling. Treat this design kit as brand
reference and artifact scaffolding. Do not copy its demo React components into
production without first translating them to the production Astryx/StyleX
patterns and checking the current frontend contract in
`docs/frontend/current-contract.md`.

---

## Sources & provenance

This system was authored **from a written brand brief only** — no existing codebase, Figma file, or asset library was provided. Every visual decision (palette, type pairing, logo mark, motion) is an original proposal and is flagged for review in the **Caveats** section below. If you have brand assets, a product repo, or a Figma library, share them and this system will be re-grounded against them.

Since this kit was created, the production frontend foundation has moved to
Astryx and StyleX. The files here remain useful for brand exploration,
throwaway prototypes, generated mockups, and asset reference. They should not
override production architecture, routing, component ownership, accessibility,
or data-flow contracts.

---

## How this system is organized

| Path | What's there |
|---|---|
| `styles.css` | The single entry point consumers link. `@import` lines only. |
| `tokens/` | CSS custom properties: `colors`, `typography`, `spacing`, `effects`, `fonts`, `base`. |
| `components/<group>/` | Demo React primitives (`core`, `forms`, `display`, `navigation`, `feedback`, `agent`) for prototypes only. |
| `Noema Design System.html` | **Single-page spec** — the shareable overview of the whole system (live components). |
| `guidelines/` | Foundation specimen cards shown in the Design System tab. |
| `assets/` | Logo / mark SVGs. |
| `SKILL.md` | Agent-Skill manifest for using this system in Claude Code. |

Prototype components are consumed as
`const { Button } = window.NoemaDesignSystem_3d237e` after loading
`_ds_bundle.js` (auto-generated — never edit by hand). Production code should
instead use the current web app's Astryx/StyleX component patterns.

---

## CONTENT FUNDAMENTALS — how Noema writes

Noema's voice is **plain-spoken, warm, and transparent**. It sounds like a capable friend who happens to be an expert, not a product brochure and not a terminal.

- **Person:** Address the user as **"you"**; the product is **"Noema"** (or "the agent"). Avoid "we" except in community/open-source contexts ("a platform the community owns").
- **Casing:** Sentence case everywhere — headings, buttons, menus. The wordmark **noema** is set lowercase. Mono micro-labels (eyebrows, statuses) are the only ALL-CAPS use.
- **Tone:** Confident but never hypey. We state what's true and useful.
  - ✅ "Noema runs on your machine. You decide what it can touch."
  - ✅ "Here's exactly what the agent did."
  - ✅ "Want a hand setting this up?"
  - ❌ "Revolutionary AI that supercharges your workflow!"
  - ❌ "Leverage synergistic agentic paradigms."
- **Transparency is a content rule, not just a UI one.** Copy always tells the user what's happening and what an agent *can* and *can't* do. Tool calls are narrated ("called `web_search`"), permissions are explicit.
- **Security framed as user control**, never fear: "Agents ask before anything irreversible," "revoke access anytime."
- **Developer register** is welcome where it belongs: tool names, commands, and versions are mono (`web_search`, `noema new research-agent`, `v0.4.2`).
- **No emoji** in product UI or marketing copy. The brand's warmth comes from tone and the clay accent, not emoji. (Status is shown with dots/badges, not 🟢.)
- **Length:** Short. One clear sentence beats three hedged ones. Buttons are verbs ("Install Noema", "New agent", "Allow").

---

## VISUAL FOUNDATIONS

The system is a warm **"paper & ink"** world with two brand hues. It deliberately avoids the cold blue-purple gradients of generic AI products.

### Color
- **Neutrals** pair a white page and card surface (`#FFFFFF`) with warm greige *paper* support surfaces (`--paper-*`) graduating to a warm near-black *ink* (`--ink-900 #17160F`) for text. Nothing is pure cold grey.
- **Pine** (`--pine-500 #1F7A57`) is the primary brand color — it carries trust, security, "go", and the open-source spirit. It's the default CTA, focus ring, and agent-avatar color.
- **Clay** (`--clay-500 #D9663F`) is the warm accent — friendliness and human touch. Used for marketing CTAs, person avatars, highlights. Used sparingly; pine leads.
- **Semantics** (`success`=pine, `warning`=amber, `danger`=warm red, `info`=muted teal-blue) each ship as a soft + solid pair.
- Always reference the **semantic aliases** (`--text-primary`, `--surface-card`, `--action-primary`), not raw ramp steps, in product code.

### Typography
- **Display — Bricolage Grotesque** (600–800), tight tracking. Characterful and "thoughtfully built." Headlines, hero, card titles.
- **Body/UI — Hanken Grotesk** (400–700). Warm humanist grotesque; highly readable.
- **Mono — JetBrains Mono** (400–600). The agent/developer voice: tool names, commands, timestamps, versions, eyebrow labels.
- Scale runs 11 → 80px on a tuned major-third-ish ramp (`--text-2xs … --text-5xl`).

### Shape, depth & texture
- **Radii** are generous but not bubbly: cards `--radius-lg (16px)`, controls `--radius-md (11px)`, pills for badges/switches. Friendly, still serious.
- **Cards**: white surface, **1px hairline border** (`--border-subtle`), **soft warm-tinted shadow** (`--shadow-sm`). Never neutral-grey shadows — they're tinted with warm ink. Interactive cards lift 2px on hover.
- **Shadows** are layered and low-contrast (`--shadow-xs … xl`), plus a pine `--shadow-brand` glow used only for active-agent emphasis.
- **Texture**: a barely-there paper **grain** (`--grain`) is available for opt-in large surfaces like the conversation thread — felt, not seen.
- **Borders** do most of the separation work; we lean on hairlines over heavy shadows.

### Backgrounds & layout
- Mostly flat white pages with warm paper support surfaces. **No decorative gradients.** Rhythm comes from one **dark ink band** (`--ink-900`) per long page (e.g. the install/terminal section), not from color washes.
- Generous whitespace; content max-widths (`--container-*`) keep prose and threads readable (threads cap ~780px).
- Fixed chrome: app sidebar (268px) and tools rail (268px) are persistent; the marketing nav is sticky with a blur backdrop (`backdrop-filter: blur`) over a translucent paper fill.

### Motion
- Calm and confident. UI transitions use `--ease-out` at 120–200ms. Toggles, pop-ins, and presses get a slight spring (`--ease-spring`).
- **Press** = subtle scale-down (0.92–0.985) ± 0.5px nudge. **Hover** = a step-darker brand color or a faint warm `--surface-hover` wash (never a glow except active agents).
- Dialogs fade the scrim + spring-pop the panel. The agent "typing" indicator is the one looping animation — everything else is one-shot.
- Respect `prefers-reduced-motion` in production.

### Transparency & blur
- Blur is used only for the sticky nav backdrop and the dialog scrim. Transparency is functional (legibility over scrolling content), never decorative.

---

## ICONOGRAPHY

- **Icon set: [Lucide](https://lucide.dev)** — loaded from CDN (`lucide@0.460.0`). Clean, consistent 24×24 grid, ~2px round-cap strokes. It matches Noema's friendly-but-precise feel and is itself open source, which is on-brand.
  - **This is a substitution to flag:** no proprietary Noema icon set was provided, so Lucide is the proposed standard. If Noema has (or wants) a bespoke set, swap it here.
- **Usage in code:** `<Icon name="Send" size={18} />` (PascalCase names) — a tiny wrapper that renders real Lucide geometry from the `lucide` CDN global (see the spec page's gallery for the pattern). **Never hand-draw icons** — a few small inline SVGs exist only inside components (checkmarks, chevrons, the spinner) where a dependency-free glyph is warranted.
- **Logo / mark:** `assets/noema-mark.svg` (pine on light) and `assets/noema-mark-light.svg` (for dark surfaces) — an "aperture / thought-node" motif (a perceiving eye + a clay arc). The wordmark is **noema**, lowercase, in Bricolage Grotesque. *This mark is an original proposal — see Caveats.*
- **Status & affordances** use dots, badges, and Lucide glyphs — **never emoji or unicode symbols** as iconography. (The one decorative exception: terminal specimens use `✓`/`$` as literal shell characters.)
- **Agents vs. people:** agents are drawn as **rounded-square** pine-gradient avatars; people as **circular** clay avatars. This shape distinction is a core brand signal — keep it consistent.

---

## Component index

**core** — `Button`, `IconButton`
**forms** — `Input`, `Textarea`, `Select`, `Checkbox`, `Switch`
**display** — `Card`, `Badge`, `Avatar`, `Tag`
**navigation** — `Tabs`
**feedback** — `Dialog`, `Toast`, `Tooltip`
**agent** — `ChatMessage`, `ToolCall`  ← Noema's signature primitives (conversation turns + transparent tool calls)

Each component directory holds `<Name>.jsx`, `<Name>.d.ts` (props + starting-point tags), `<Name>.prompt.md` (usage), and one `@dsCard` HTML demo.

---

## Foundation cards (Design System tab)

`guidelines/` holds specimen cards grouped as **Colors** (pine, clay, neutrals, semantic), **Type** (display, body, mono, scale), **Spacing** (scale, radii, shadows), and **Brand** (logo, voice & tone, motion).

---

## CAVEATS — please review

This system was built from a brand brief with **no supplied assets**, so several foundational choices are my proposals and need your sign-off:

1. **Palette** — white page, warm paper support neutrals, **pine** primary, and **clay** accent. Does this match how you want Noema to feel, or should the brand be cooler / more technical / more playful?
2. **Typefaces** — Bricolage Grotesque / Hanken Grotesk / JetBrains Mono. These load from **Google Fonts CDN** (`tokens/fonts.css`); local binaries are **not** bundled and the compiler reports 0 packaged fonts. If you want offline/self-hosted fonts, send the files and I'll add real `@font-face` rules.
3. **Logo & mark** — the "aperture/thought-node" SVG is a quick original concept, not a finished identity. Replace with your real logo when ready.
4. **Iconography** — Lucide is a substitution; confirm or provide your set.
> Note: this turn removed the earlier mock product surfaces (an app workspace and a marketing site) in favor of a single **spec page** (`Noema Design System.html`). If you later want full product-screen recreations, share real Noema screens or a repo and I'll build them precisely.

### The ask
**Tell me which of the five above are right and which are wrong.** The fastest way to make this perfect: send any real Noema assets (logo, brand colors, product screenshots, a repo, or a Figma link). Absent those, point me at adjectives — e.g. "warmer," "more technical," "less earthy" — and I'll iterate the palette and type in one pass.
