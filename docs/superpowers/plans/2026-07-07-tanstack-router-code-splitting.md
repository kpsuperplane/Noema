# TanStack Router Code Splitting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate the Noema web UI to TanStack Router file-based routes and enable route-level JavaScript chunks.

**Architecture:** Keep the app client-first on Vite. The TanStack root route owns the existing boot boundary and shell frame, while leaf route files render chat, memory, graph, and settings surfaces from a small Noema React context. The Rust daemon serves every emitted asset chunk under `/assets/` instead of only `app.js`.

**Tech Stack:** React 19, Vite 7, TanStack Router, StyleX, Apollo Client, Bun, Rust daemon asset serving.

## Global Constraints

- Work on `main`.
- Preserve unrelated dirty worktree changes.
- Keep Noema's Rust GraphQL server and Tauri asset model; do not add TanStack Start or SSR.
- Do not redesign shell IA, Settings IA, chat, onboarding, transcript, or memory graph UX.
- Frontend validation uses `bun run gen:types`, `bun run lint`, and `bun run build` from `crates/noema-core/web`.
- Rust validation for this slice uses `cargo fmt --all --check` and targeted daemon web tests.

---

### Task 1: Router Dependencies And Vite Chunks

**Files:**
- Modify: `crates/noema-core/web/package.json`
- Modify: `crates/noema-core/web/bun.lock`
- Modify: `crates/noema-core/web/vite.config.ts`
- Modify: `crates/noema-core/web/vite.desktop.config.ts`

**Interfaces:**
- Produces: TanStack Router plugin configured before `@vitejs/plugin-react`.
- Produces: Vite output with dynamic JS chunks enabled and CSS kept in `styles.css`.

- [ ] Add `@tanstack/react-router` as a runtime dependency and `@tanstack/router-plugin` as a dev dependency with Bun.
- [ ] Import `tanstackRouter` from `@tanstack/router-plugin/vite` in both Vite configs.
- [ ] Add `tanstackRouter({ target: "react", autoCodeSplitting: true })` before `react(...)`.
- [ ] Remove `inlineDynamicImports: true` from the daemon Vite config.
- [ ] Set `cssCodeSplit: false` in both Vite configs so existing single stylesheet serving remains valid.

### Task 2: App Root And Route Context

**Files:**
- Modify: `crates/noema-core/web/src/app/App.tsx`
- Modify: `crates/noema-core/web/src/app/routes.ts`
- Create: `crates/noema-core/web/src/app/AppRuntimeContext.tsx`
- Create: `crates/noema-core/web/src/router.tsx`
- Modify: `crates/noema-core/web/src/main.tsx`

**Interfaces:**
- Produces: `useAppRuntime()` with `chatView`, `openMemoryGraph()`, and `settingsSection`.
- Produces: `router` exported from `src/router.tsx`.

- [ ] Replace the hand-rolled `useBrowserRoute` hook usage with TanStack `useLocation`, `useNavigate`, and `useRouter`.
- [ ] Keep pure `routeFromPathname`, `pathForRoute`, and settings back helpers for shell state/tests.
- [ ] Move route-visible app values into `AppRuntimeContext`.
- [ ] Make `main.tsx` render `<RouterProvider router={router} />` inside the existing Apollo provider.

### Task 3: File-Based Routes

**Files:**
- Create: `crates/noema-core/web/src/routes/__root.tsx`
- Create: `crates/noema-core/web/src/routes/index.tsx`
- Create: `crates/noema-core/web/src/routes/memory.tsx`
- Create: `crates/noema-core/web/src/routes/memory/graph.tsx`
- Create: `crates/noema-core/web/src/routes/settings.tsx`
- Create: `crates/noema-core/web/src/routes/settings/agents.tsx`
- Create: `crates/noema-core/web/src/routes/settings/tools/web.tsx`
- Create: `crates/noema-core/web/src/routes/settings/tools/mcps.tsx`
- Create: `crates/noema-core/web/src/routes/settings/safety/usage.tsx`
- Create: `crates/noema-core/web/src/routes/settings/safety/approvals.tsx`
- Create: `crates/noema-core/web/src/routes/settings/safety/identities.tsx`
- Create: `crates/noema-core/web/src/routes/settings/system/providers.tsx`
- Create: `crates/noema-core/web/src/routeTree.gen.ts`

**Interfaces:**
- Consumes: `AppRoot` and `useAppRuntime()`.
- Produces: Route files for all current canonical Noema routes.

- [ ] Root route renders `<AppRoot><Outlet /></AppRoot>`.
- [ ] Index route renders the existing chat view from runtime context.
- [ ] Memory home route renders `MemoryHomePage`.
- [ ] Memory graph route renders `MemoryGraphPage`.
- [ ] Settings routes render `SettingsSurface` with the canonical section.
- [ ] Generate and commit `routeTree.gen.ts`.

### Task 4: Daemon Asset Chunk Serving

**Files:**
- Create: `crates/noema-core/build.rs`
- Modify: `crates/noema-core/src/daemon/web/assets.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`

**Interfaces:**
- Produces: release embedded asset lookup for all files emitted to `target/web-assets`.
- Produces: debug asset lookup for exact static paths under `/assets/`.

- [ ] Add a build script that scans `target/web-assets` and writes an `OUT_DIR` Rust asset table with `include_bytes!` entries.
- [ ] Route `/assets/<filename>` requests through the emitted asset table or debug filesystem lookup.
- [ ] Preserve SPA entry fallback for non-asset product routes.
- [ ] Keep missing `/assets/*` paths returning `None`.
- [ ] Add tests for present chunk-like assets and missing static assets.

### Task 5: Validation And Commit

**Files:**
- Modify as needed from prior tasks only.

**Interfaces:**
- Produces: passing route/build validation and a scoped commit.

- [ ] Run `bun run gen:types`.
- [ ] Run `bun run lint`.
- [ ] Run `bun run build`.
- [ ] Run `cargo fmt --all --check`.
- [ ] Run targeted daemon web tests.
- [ ] Run `git status --short --branch` and `git diff --check`.
- [ ] Inspect staged changes with `git diff --cached --stat` and `git diff --cached --name-status`.
- [ ] Commit the TanStack Router migration.

## Self-Review

- Spec coverage: the plan covers TanStack Router adoption, route code splitting, daemon chunk serving, and validation.
- Placeholder scan: no deferred implementation placeholders remain.
- Scope check: this is one cohesive routing/bundling migration and does not include UI redesign or server-framework changes.
