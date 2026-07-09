# Artifact Card Action Icon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Update Noema's subagent model guidance for GPT-5.6 and replace artifact-card action text with a bottom-right icon that appears on hover or keyboard focus while remaining visible on touch devices.

**Architecture:** Keep artifact navigation owned by the existing Astryx `Item`; the action icon is a non-interactive affordance so it does not create a duplicate tab stop. Position the icon inside the card with StyleX, and use stable `data-slot` selectors in the global stylesheet for parent hover/focus and pointer-capability behavior.

**Tech Stack:** Markdown repository guidance, React, TypeScript, Astryx `Item` and `Tooltip`, StyleX, CSS media queries, Lucide icons.

## Global Constraints

- Work on `main`, as required by the repository.
- Do not add UI tests or inspect with browser tools unless explicitly requested.
- Keep local detail artifacts on the existing `onOpenDetail` path and external/download artifacts on their existing links.
- Use `PanelRightOpen`, `Download`, and `ExternalLink` for the three available action types.
- Hide the icon only on hover-capable devices; reveal it on card hover or `:focus-within`; keep it visible on touch devices.

---

### Task 1: GPT-5.6 Subagent Policy

**Files:**
- Modify: `AGENTS.md`

**Interfaces:**
- Consumes: the existing `Review And Subagents` policy.
- Produces: durable model-selection guidance for future Noema tasks.

- [ ] **Step 1: Replace the GPT-5.5 default with workload-based GPT-5.6 guidance**

Set the section to use `gpt-5.6-terra` at medium reasoning for normal coding, `gpt-5.6-sol` at high reasoning for difficult architecture and adversarial review, and `gpt-5.6-luna` at low or medium reasoning only for mechanical high-volume work. State that narrow changes should stay inline when delegation overhead exceeds the work.

- [ ] **Step 2: Validate the repository guidance diff**

Run:

```bash
git diff --check
git diff -- AGENTS.md
```

Expected: no whitespace errors, and only the `Review And Subagents` model guidance changes.

- [ ] **Step 3: Commit the policy update**

```bash
git add AGENTS.md
git diff --cached --stat
git diff --cached --name-status
git commit -m "Update subagent guidance for GPT-5.6"
```

Expected: one committed file and no unrelated staged changes.

---

### Task 2: Artifact Card Hover Action

**Files:**
- Modify: `crates/noema-core/web/src/components/transcript/ArtifactReferenceCard.tsx`
- Modify: `crates/noema-core/web/src/styles.css`

**Interfaces:**
- Consumes: `opensDetail`, `ArtifactLink`, `artifactActionIcon`, Astryx `Item`, and Astryx `Tooltip`.
- Produces: an icon-only `endContent` affordance with `data-slot="artifact-reference-action"` inside a card marked `data-slot="artifact-reference-item"`.

- [ ] **Step 1: Replace action text with the typed icon affordance**

In `ArtifactReferenceCard.tsx`:

- Import `Tooltip` from `@astryxdesign/core/Tooltip` and `PanelRightOpen` from `lucide-react`.
- Return `PanelRightOpen` when `opensDetail`, otherwise reuse `artifactActionIcon(link)` for direct download and external links.
- Omit `endContent` when the artifact has no available action.
- Render the icon in a tooltip-labeled, non-interactive span with `data-slot="artifact-reference-action"`.
- Add `data-slot="artifact-reference-item"` to the Astryx `Item`.
- Position the icon at the bottom-right and reserve inline space through StyleX so it cannot cover card text.

- [ ] **Step 2: Add hover, focus, touch, and reduced-motion CSS**

In `styles.css`, add:

```css
[data-slot="artifact-reference-action"] {
  opacity: 1;
  transition: opacity 140ms ease;
}

@media (hover: hover) {
  [data-slot="artifact-reference-item"] [data-slot="artifact-reference-action"] {
    opacity: 0;
    pointer-events: none;
  }

  [data-slot="artifact-reference-item"]:hover [data-slot="artifact-reference-action"],
  [data-slot="artifact-reference-item"]:focus-within [data-slot="artifact-reference-action"] {
    opacity: 1;
    pointer-events: auto;
  }
}

@media (prefers-reduced-motion: reduce) {
  [data-slot="artifact-reference-action"] {
    transition-duration: 1ms;
  }
}
```

Expected: mouse users see the icon only while hovering or focusing the card; touch users always see it.

- [ ] **Step 3: Run frontend validation**

Run from `crates/noema-core/web`:

```bash
bun run lint
```

Then run from the repository root:

```bash
git diff --check
git status --short --branch
```

Expected: type generation, TypeScript, ESLint, and whitespace validation succeed; only the two intended frontend files are modified.

- [ ] **Step 4: Commit the artifact-card interaction**

```bash
git add crates/noema-core/web/src/components/transcript/ArtifactReferenceCard.tsx crates/noema-core/web/src/styles.css
git diff --cached --stat
git diff --cached --name-status
git commit -m "Refine artifact card action affordance"
```

Expected: the two frontend files are committed with no unrelated staged changes.
