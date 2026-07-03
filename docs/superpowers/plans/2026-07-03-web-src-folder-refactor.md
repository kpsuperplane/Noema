# Web Src Folder Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move root-level Noema web source helpers into domain folders without changing frontend behavior.

**Architecture:** Keep `src/main.tsx` as the Vite entrypoint and keep platform folders such as `generated/`, `graphql/`, `theme/`, `pages/`, and `components/` intact. Move app routing into `src/app/`, memory helpers into `src/memory/`, transcript event assembly into `src/transcript/`, and shared labels/types into `src/shared/`.

**Tech Stack:** React, TypeScript, Vite, Apollo GraphQL, StyleX, Bun.

---

### Task 1: Move Files Into Domain Folders

**Files:**
- Move: `crates/noema-core/web/src/App.tsx` to `crates/noema-core/web/src/app/App.tsx`
- Move: `crates/noema-core/web/src/routes.ts` to `crates/noema-core/web/src/app/routes.ts`
- Move: `crates/noema-core/web/src/memoryCards.ts` to `crates/noema-core/web/src/memory/cards.ts`
- Move: `crates/noema-core/web/src/memoryGraph.ts` to `crates/noema-core/web/src/memory/graph.ts`
- Move: `crates/noema-core/web/src/memoryGraphLayout.ts` to `crates/noema-core/web/src/memory/graphLayout.ts`
- Move: `crates/noema-core/web/src/transcript.ts` to `crates/noema-core/web/src/transcript/events.ts`
- Move: `crates/noema-core/web/src/format.ts` to `crates/noema-core/web/src/shared/format.ts`
- Move: `crates/noema-core/web/src/types.ts` to `crates/noema-core/web/src/shared/types.ts`

- [ ] **Step 1: Create folders and move files**

Run:

```bash
mkdir -p crates/noema-core/web/src/app crates/noema-core/web/src/memory crates/noema-core/web/src/transcript crates/noema-core/web/src/shared
mv crates/noema-core/web/src/App.tsx crates/noema-core/web/src/app/App.tsx
mv crates/noema-core/web/src/routes.ts crates/noema-core/web/src/app/routes.ts
mv crates/noema-core/web/src/memoryCards.ts crates/noema-core/web/src/memory/cards.ts
mv crates/noema-core/web/src/memoryGraph.ts crates/noema-core/web/src/memory/graph.ts
mv crates/noema-core/web/src/memoryGraphLayout.ts crates/noema-core/web/src/memory/graphLayout.ts
mv crates/noema-core/web/src/transcript.ts crates/noema-core/web/src/transcript/events.ts
mv crates/noema-core/web/src/format.ts crates/noema-core/web/src/shared/format.ts
mv crates/noema-core/web/src/types.ts crates/noema-core/web/src/shared/types.ts
```

Expected: root `src` contains only entrypoint/style/platform folders plus `.DS_Store` if present.

### Task 2: Update Imports

**Files:**
- Modify: moved files above
- Modify: existing `crates/noema-core/web/src/components/**`
- Modify: existing `crates/noema-core/web/src/pages/**`
- Modify: `crates/noema-core/web/src/main.tsx`

- [ ] **Step 1: Update aliases and relatives**

Use `@/app/routes`, `@/memory/cards`, `@/memory/graph`, `@/memory/graphLayout`, `@/transcript/events`, `@/shared/format`, and `@/shared/types` for cross-folder imports. Keep same-folder relative imports inside component folders.

- [ ] **Step 2: Search for stale imports**

Run:

```bash
rg 'from "(\./|\.\./)*(types|format|memoryCards|memoryGraph|memoryGraphLayout|routes|transcript)"|@/(types|format|memoryCards|memoryGraph|memoryGraphLayout|routes|transcript)' crates/noema-core/web/src
```

Expected: no stale import paths.

### Task 3: Validate And Commit

**Files:**
- Modify: `docs/context/current.md` only if durable frontend organization context changes.

- [ ] **Step 1: Run validation**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: both commands pass.

- [ ] **Step 2: Run ship checks**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected: no whitespace errors; staged files are limited to the web source organization refactor and this plan/context update if included.

- [ ] **Step 3: Commit**

Run:

```bash
git add docs/superpowers/plans/2026-07-03-web-src-folder-refactor.md crates/noema-core/web/src
git commit -m "refactor(web): organize source files by domain"
```
