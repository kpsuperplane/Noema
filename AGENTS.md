# Noema Personal Agent

## Resources 
- Project context: `docs/project.md`
- Current project brief: `docs/context/current.md`
- Engineering simplicity workflow: `docs/development/simplicity.md`
- Plain code terms: `docs/development/terms.md`
- Product UI design guidance: `docs/frontend/product-design.md`

## Communication
- Use `docs/development/terms.md` for Noema terms in new prose and changed identifiers.
- Use the current issue of ASD-STE100 Simplified Technical English for all user communication and all prose that you write.
- Use short sentences, active voice, and one topic in each sentence. Use no more than 20 words in an instruction and 25 words in a descriptive sentence.
- Use one word for one meaning. Use approved words when possible. Use Noema and software terms as technical nouns or technical verbs when necessary.
- Put a condition before the action that depends on it. Give one instruction in each sentence unless the actions occur at the same time.
- Do not change quoted text, user text, code, commands, identifiers, protocol fields, or required external terms to make them comply with ASD-STE100.

## Standards
- Work on main branch unless explicitly instructed
- Optimize total system simplicity and net code, not local completeness. Read `docs/development/simplicity.md` before nontrivial implementation, refactoring, testing-policy, architecture, harness, or workflow work.
- Fix the smallest general failure mode demonstrated by a bug. Do not expand into adjacent failure classes, redesign surrounding architecture, or add speculative extensibility unless the user requests it.
- Prefer changing an existing authority over introducing a parallel abstraction. Unless the human explicitly directs future-facing work, a new trait, port, DTO, compatibility layer, manager, or generic framework requires at least two concrete production consumers with materially different needs; multiple callers of one implementation, tests, and agent-inferred hypothetical future use do not count. Explicit human direction may authorize a named future contract, but implement only that bounded contract without extrapolating adjacent extensibility.
- Refactors must be net-negative unless they add named user-visible functionality. Consolidate existing logic whenever reasonable, do not add production APIs or polymorphism solely for test convenience, and stop a reduction slice that starts growing helper infrastructure.
- Treat every abstraction, schema field, state variant, configuration option, fallback, and background process as a liability until a current end-to-end production path enforces it or the human explicitly requests it for a named future contract. Without that direction, a value that is only parsed, stored, copied, prompted for, or tested is not implemented product behavior; remove or reject it instead of advertising it.
- Before deleting apparently dead code, check non-default Cargo features, binaries, generated clients, native clients, and evaluation/support targets. A concrete support consumer may justify retaining code, but does not justify generalizing the production design.
- The project is under active development; do not build general backwards compatibility unless explicitly instructed. Every retained compatibility path needs a named supported source/version, a bounded support window or removal condition, and a test of the exact retained behavior. Do not run speculative cleanup scans on every startup.
- A cleanup or refactor request does not authorize retiring a live product capability. When the largest reduction requires choosing which client, integration substrate, workflow, or vertical system remains supported, stop and request that product decision.
- Every persisted database schema change must append a forward-only migration and advance the schema version. The development server watches and rebuilds the application, so it may live-apply a new migration to `.noema-dev` before the code is committed or the task is finished. Treat a migration as immutable as soon as it is written: never revise, expand, reorder, or reuse its version after it may have run; append another migration for every subsequent schema correction. Test both an existing-version upgrade and fresh-schema convergence. Rewrite or reset a database schema only when explicitly instructed.
- Treat a source file over 750 lines as an inspection signal, not a reason to split it. Split only when the result creates clearer ownership and reduces or preserves total complexity; never introduce pass-through modules, mirrored types, or helper layers to satisfy a line target.
- Do not use direct text, prefix, or English phrase matching as the authority for semantic user intent. It is brittle and fails for multilingual users. Prefer explicit product state, structured model/tool interpretation with policy checks, or language-aware parsers/tests.
- Avoid polling unless it is the only viable solution. Prefer WebSockets, server-sent events, subscriptions, or another realtime event stream whenever the source can push state changes.

## Information Handling

- `docs/harness/security.md` is the detailed authority. Use exactly three information classes: **secrets**, **private information**, and **ordinary information**. Trust, provenance, retention, action risk, and memory lifecycle are orthogonal attributes, not additional secrecy levels.
- Secrets are credential material whose possession grants authority, including passwords, API keys, access/refresh tokens, private keys, session cookies, authorization codes, PKCE verifiers, and recovery codes. Keep them only in explicit credential or protected transient-auth stores, pass them through secure bindings, and never place them in model context, logs, conversation history, ordinary events, artifacts, or exports.
- Private information may persist in its governed source and may reach an LLM intact when the run, scope, purpose, and grants authorize it. When access is not authorized, omit or deny it at the context boundary; do not destructively redact the source value. External disclosure is decided by the egress model.
- Ordinary information should remain intact. Do not redact or conceal a value merely because it is an identifier, opaque string, path, URL, hostname, port, model name, provider/account ID, schema field, technical diagnostic, or because its field name contains words such as `authorization`, `secret`, or `cookie`.
- Redaction is not a substitute for authorization or egress policy. Use typed secret wrappers, exact schema annotations, or credential-store provenance rather than substring, entropy, or English-name matching. Redact only the actual secret-bearing value/component at a forbidden sink, or create an explicitly policy-approved redacted derivative for egress while preserving the governed source.
- Treat unnecessary concealment of authorized private information or ordinary information as a correctness and observability regression. A secret-handling change needs a paired preservation assertion for representative non-secret values at the same boundary.
- When existing code, tests, archived plans, or historical specs conflict with this contract, treat them as cleanup targets rather than precedent. Current subsystem authorities and this section win.

## Codex Workflow
- Start by checking `git status --short --branch`.
- Preserve unrelated dirty worktree changes.
- For architecture, memory, harness, frontend IA, or workflow work, read `docs/project.md`, `docs/context/current.md`, and the closest relevant docs first.
- For nontrivial work, make the task mode explicit before proceeding: explore only, plan only, implement, adversarial review, or ship.
- Before nontrivial implementation, state the observable outcome, non-goals, reuse/consolidation target, expected files, production/test code budget, planned tests with the unique risk each covers, and stop conditions. Do not edit until that brief is coherent.
- Measure Go server patches as production, tests, generated GraphQL, and inclusive tracked lines. Keep both migration ratios below 80%.
- For retained Rust patches, use `bun run scripts/report-rust-size.ts --base <ref>`. Apply its budget options and use `--require-net-negative` for refactors.
- Stop when a patch exceeds its production or test estimate by 50% or 500 lines, whichever is smaller.
- Split long work at milestone boundaries. After a major commit or completed phase, summarize durable context into `docs/context/current.md` before continuing.
- Keep `docs/context/current.md` below 300 lines. Replace stale material instead of appending milestone history; move durable subsystem decisions to the closest authoritative document and rely on Git history for completed execution detail.
- Make a commit after finishing each unit of work unless explicitly instructed not to.
- Treat raw `~/.codex/sessions` files as private source material. Read them only when asked, summarize durable decisions, and do not quote raw transcript unless explicitly requested.

## Product UI Work
- Use the repo-local `noema-product-ui` skill for any task that designs, builds, reviews, or materially changes frontend layout, hierarchy, spacing, density, responsive behavior, or information disclosure.
- Read `docs/frontend/product-design.md` and the closest surface contract before editing. Establish the human's job, the focal action or content, the information priority, and the intended grouping before choosing components or writing CSS.
- Treat Noema as a dense product interface, not a marketing page. Do not add whitespace, cards, headings, icons, metadata, columns, or motion merely to make a screen feel designed; every structural device must communicate a relationship, priority, state, or action.
- Use Astryx components and spacing tokens before one-off controls or raw spacing values. Reuse the existing shell, detail, transcript, and domain patterns instead of creating a parallel presentation for the same concept.
- When editing a field, use either inline editing that saves without a separate Save button, or an actual dialog with an explicit Save button and the first input automatically focused. Do not place an explicit Save flow inline on the page.
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
- Default Go server validation:
  - `CGO_ENABLED=0 go test ./cmd/... ./internal/...`
  - `CGO_ENABLED=0 go vet ./cmd/... ./internal/...`
- Default retained Rust validation:
  - `cargo fmt --all --check`
  - `cargo check-workspace`
  - `cargo gate-lint`
  - `cargo gate-test`
- Run focused Cargo commands through `scripts/validate-rust <cargo-command> [arguments]`, for example `scripts/validate-rust test -p noema-capability-adapters --lib`.
- Run unit tests only. Do not run smoke tests or fixture tests unless explicitly requested.
- Add tests for unique risks at the authoritative layer. A bug normally gets one regression test; an ordinary feature normally gets three to eight focused tests. More than ten new tests requires a written risk and redundancy justification before implementation continues.
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
CLI: from `apps/web`, run `bun run astryx -- <cmd>` (shown below as `astryx ...`).

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
