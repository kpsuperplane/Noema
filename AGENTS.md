# Noema Personal Agent

## Resources 
- Project context: `docs/project.md`
- Current project brief: `docs/context/current.md`
- Engineering simplicity workflow: `docs/development/simplicity.md`
- Product UI design guidance: `docs/frontend/product-design.md`

## Standards
- Work on main branch unless explicitly instructed
- Optimize total system simplicity and net code, not local completeness. Read `docs/development/simplicity.md` before nontrivial implementation, refactoring, testing-policy, architecture, harness, or workflow work.
- Fix the smallest general failure mode demonstrated by a bug. Do not expand into adjacent failure classes, redesign surrounding architecture, or add speculative extensibility unless the user requests it.
- Prefer changing an existing authority over introducing a parallel abstraction. A new trait, port, DTO, compatibility layer, or generic framework requires at least two concrete production consumers; tests and hypothetical future use do not count.
- Refactors must be net-negative unless they add named user-visible functionality. Consolidate existing logic whenever reasonable and stop a reduction slice that starts growing helper infrastructure.
- The project is under active development; do not build general backwards compatibility unless explicitly instructed.
- Every persisted database schema change must append a forward-only migration and advance the schema version. Keep previously shipped migration SQL immutable, and test both an existing-version upgrade and fresh-schema convergence. Rewrite or reset a database schema only when explicitly instructed.
- Try to keep code source files under 750 lines. It is not a hard rule, however any file exceeding that threshold should be inspected for refactor, split up, and cleanup opportunities
- Do not use direct text, prefix, or English phrase matching as the authority for semantic user intent. It is brittle and fails for multilingual users. Prefer explicit product state, structured model/tool interpretation with policy checks, or language-aware parsers/tests.
- Avoid polling unless it is the only viable solution. Prefer WebSockets, server-sent events, subscriptions, or another realtime event stream whenever the source can push state changes.

## Codex Workflow
- Start by checking `git status --short --branch`.
- Preserve unrelated dirty worktree changes.
- For architecture, memory, harness, frontend IA, or workflow work, read `docs/project.md`, `docs/context/current.md`, and the closest relevant docs first.
- For nontrivial work, make the task mode explicit before proceeding: explore only, plan only, implement, adversarial review, or ship.
- Before nontrivial implementation, state the observable outcome, non-goals, reuse/consolidation target, expected files, production/test code budget, planned tests with the unique risk each covers, and stop conditions. Do not edit until that brief is coherent.
- Measure the patch with `bun run scripts/report-rust-size.ts --base <ref>` at milestone boundaries and before commit. Apply the task budget with `--max-production-net`, `--max-test-net`, and `--max-new-tests`; use `--require-net-negative` for refactors. Stop when the patch exceeds its production or test estimate by 50% or 500 lines, whichever is smaller.
- Split long work at milestone boundaries. After a major commit or completed phase, summarize durable context into `docs/context/current.md` before continuing.
- Keep `docs/context/current.md` below 300 lines. Replace stale material instead of appending milestone history; move durable subsystem decisions to the closest authoritative document and rely on Git history for completed execution detail.
- Make a commit after finishing each unit of work unless explicitly instructed not to.
- Treat raw `~/.codex/sessions` files as private source material. Read them only when asked, summarize durable decisions, and do not quote raw transcript unless explicitly requested.

## Product UI Work
- Use the repo-local `noema-product-ui` skill for any task that designs, builds, reviews, or materially changes frontend layout, hierarchy, spacing, density, responsive behavior, or information disclosure.
- Read `docs/frontend/product-design.md` and the closest surface contract before editing. Establish the human's job, the focal action or content, the information priority, and the intended grouping before choosing components or writing CSS.
- Treat Noema as a dense product interface, not a marketing page. Do not add whitespace, cards, headings, icons, metadata, columns, or motion merely to make a screen feel designed; every structural device must communicate a relationship, priority, state, or action.
- Use Astryx components and spacing tokens before one-off controls or raw spacing values. Reuse the existing shell, detail, transcript, and domain patterns instead of creating a parallel presentation for the same concept.
- For nontrivial visual work, ask for browser-inspection permission early when it has not already been granted. If visual inspection is not authorized, complete static and build validation but state that the layout was not visually verified.

## Review And Subagents
- Keep changes expected to touch fewer than roughly 1,000 lines or two architectural areas inline when delegation overhead would exceed the work.
- Parallel implementers own independently shippable vertical slices, not domain/store/runtime/API layers of one slice. The parent owns the total code and test budget and must stop local completeness from expanding global scope.
- For adversarial review, reviewers should inspect and report findings without editing files.
- Default to one review pass and one correction pass. Fix correctness, security, privacy, data-loss, and demonstrated-regression findings automatically; require explicit scope approval for speculative hardening, extensibility, or additional abstraction. Run another review only when the first found a serious unresolved defect.
- The main agent owns final integration, validation, and the user-facing summary.

## Validation
- Never circumvent, disable, bypass, unset, or otherwise interfere with the `sccache` build cache.
- Never modify `CARGO_BUILD_RUSTC_WRAPPER` or attempt to work around the configured Rust compiler wrapper. Doing so invalidates shared cache state, causes 15min+ builds, and can break other agents building in parallel.
- Default Rust validation:
  - `cargo fmt --all --check`
  - `cargo check-workspace`
  - `cargo gate-lint`
  - `cargo gate-test`
- Run focused Cargo commands through `cargo validate <cargo-command> [arguments]`, for example `cargo validate test -p noema-capability-adapters --lib`.
- Run unit tests only. Do not run smoke tests or fixture tests unless explicitly requested.
- Add tests for unique risks at the authoritative layer. A bug normally gets one regression test; an ordinary feature normally gets three to eight focused tests. More than ten new Rust tests requires a written risk and redundancy justification before implementation continues.
- Do not test derives, getters, constructors, enum mirrors, pass-through mappings/resolvers, or mock interactions unless they enforce an external compatibility or security contract. Do not repeat the same behavior through domain, store, API, and runtime layers unless each boundary owns materially different logic.
- Do not write tests for UI/frontend work unless explicitly requested.
- For frontend or UI work, do not inspect with browser tools unless explicitly requested.

## Ship Checklist
- Before committing or pushing, run:
  - `git status --short --branch`
  - `git diff --check`
  - the relevant validation commands above
- Inspect staged changes with `git diff --cached --stat` and `git diff --cached --name-status`.
- Report remaining untracked or unstaged files.
- Commit after each finished unit of work. Push only when explicitly requested.

## Astryx Framework Frontend Guidelines
Astryx v0.1.9 · 90+ components
CLI: run every command as `npx @astryxdesign/cli <cmd>` (shown below as `astryx ...`).

SETUP (once, in your app entry e.g. main.tsx) — without these, components render unstyled:
  import "@astryxdesign/core/reset.css";
  import "@astryxdesign/core/astryx.css";

WORKFLOW — discover, don't guess. Before writing UI:
1. `astryx build "<idea>"` — START HERE: returns a kit (closest [page] + [block]s + [component]s). No args = full playbook.
2. `astryx template <name> [--skeleton]` — scaffold the [page]/[block]s it named, or study their layout. Templates are reference code.
3. `astryx component <Name>` — props + examples for every component you use.

RULES:
- No <div> except for custom, documented, non-standard layouts — astryx components do all layout/spacing. Full page → AppShell; sidebar nav → SideNav.
- Frame first: pick the shell (AppShell / Layout+LayoutPanel) and budget regions in px BEFORE writing content (`astryx docs layout`).
- Dense data = rows (Table, List/Item) edge-to-edge — never Card-wrapped list items. Card = dashboard widgets, galleries, settings groups only.
- Status → StatusDot/Token; Badge only for counts and enumerated states, never decoration.
- Custom styling: component props first; else prefer style/className with tokens — var(--color-*|--spacing-*|--radius-*). Only use raw hex/px for one-time overrides with documented reasons.
- Tokens for every value (`astryx docs tokens`). Brand/accent via `astryx theme` — never override --color-* in :root.
- SELF-CHECK before you finish: re-read the file and replace any raw <div>/<span> layout, imported .css/@apply, or hardcoded value (#hex, 16px) with the component or a token (var(--color-*|--spacing-*|…)). If unsure a component/prop exists, run `astryx component <Name>` / `astryx search "<thing>"`; don't hand-roll CSS.

MORE CLI:
  search "<query>"   find any component / hook / doc / template / block
  component --list   90+ components by category
  template --list    page + block recipes
  docs <topic>       color, elevation, icons, illustrations, internationalization, layout, migration, motion, principles, shape, spacing, styling, theme, tokens, typography
  swizzle <Name>     eject component source for deep customization
  upgrade --apply    run after any @astryxdesign/core bump