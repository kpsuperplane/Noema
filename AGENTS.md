# Noema Personal Agent

## Resources 
- Project context: `docs/project.md`
- Current project brief: `docs/context/current.md`
- Engineering simplicity workflow: `docs/development/simplicity.md`
- Plain code terms: `docs/development/terms.md`
- Product UI design guidance: `docs/frontend/product-design.md`

## Communication
- Explain work through what the user can see or do. Start with the problem, the change, or the result.
- Use everyday language in progress updates and final answers. Assume no knowledge of the code or internal design.
- Short sentences alone are not enough. Replace technical shorthand with a concrete explanation of its effect on the user.
- Include implementation details only when requested or needed to explain a decision, limitation, or failure.
- When a technical term is necessary, explain its meaning on first use. Do not stack technical terms in one sentence.
- Describe checks by the behavior they verified. Keep command lists, internal names, and detailed counts in linked evidence when possible.
- For example: "I’ll prevent clicks on changed buttons. I’ll keep passwords out of page summaries. Actions you declined will require your approval."
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

## Noema Development Access

These instructions describe the Linux root development instance in `/root/noema`.
Use `noema-build` for development and inspection. It grants full local filesystem access and direct network access for the privileged development launcher.
Keep credential values in protected stores and secure bindings.

### Start and check the instance

- Use the existing development session when it is running.
- To start the complete session, run `./attach` with `noema-build` in `/root/noema`.
- The launcher requires `bindfs`. It starts the server, private socket relay, and read-only home view.
- Do not substitute `go run ./cmd/noema-dev` for this launcher. That command does not start the inspection relays or home view.
- Check actual socket access with this read-only request:

```sh
curl --silent --show-error --max-time 5 \
  --unix-socket /tmp/noema-codex/graphql.sock http://localhost/auth/status
```

The expected response is `{"state":"authenticated"}`. A socket file alone does not prove that the server is running.

### Inspect files and the database

- Use `/tmp/noema-codex/home` to inspect the complete development `NOEMA_HOME`.
- This live, read-only view includes databases, configuration, memory, artifacts, protected stores, and newly created files.
- The source defaults to `/var/lib/noema-dev`. An explicit launcher `NOEMA_HOME` changes the source, not the inspection path.
- Use ordinary file tools through the view. Open SQLite with `mode=ro` for database inspection.
- Direct source reads can fail because the source belongs to `noema-dev`. That failure does not mean inspection needs Full Access.
- The view changes displayed ownership and permissions. It does not change source files or their permissions.
- Follow Information Handling above. Keep actual credential values out of model context, logs, screenshots, and artifacts.

### Inspect the web app

- Use `/tmp/noema-codex/graphql.sock` for app requests. Do not use `/tmp/noema-codex/home/run/graphql.sock`; the home view does not forward sockets.
- Use `scripts/route-browser-inspection.mjs` with Playwright for authenticated, read-only browser inspection.
- After installing the helper on a fresh browser context, navigate to `http://noema.local/<route>`.
- `noema.local` is the helper's routed origin. It is not a public hostname or a standalone browser login route.
- Follow [browser inspection](docs/frontend/browser-inspection.md) for the complete Playwright example and visual review procedure.
- The helper rejects mutations. Inspection access does not authorize changes to live product data.
- Do not disable public authentication or expose the socket through an unauthenticated TCP listener.

### Resolve access failures

- If commands still have old restrictions, select `noema-build` and start a new Codex session.
- To check the saved profile explicitly, prefix a command with `codex sandbox -C /root/noema -P noema-build --`.
- If the socket refuses connections or the home view is missing, check the host development session and launcher output.
- Start or repair the launcher with `noema-build`. A stale restricted session must reload the updated profile first.
- The default profile needs no HTTP inspection credential. The helper uses the authenticated relay when proxy environment variables are present.
- Do not print or manually copy the relay credential. The helper loads it internally.
- See [development permissions](docs/development/codex-permissions.md) for profile settings, setup, and verified checks.

### Debug user-reported bugs

Treat reported product bugs as noema-dev bugs unless the human names another environment.
Start from the screenshot, approximate time, and expected behavior when available.
Inspect available evidence before asking the human to collect logs or identify the subsystem.

1. Use the existing development instance. Check the authenticated socket and running build before reproducing. Do not start a second server.
2. Identify the affected Chat turn, Task run, or screen. Record its identifiers so later checks follow the same failure.
3. Read saved events, errors, timing spans, and relevant database state through the read-only development view. Follow the information-handling rules above.
4. Reproduce through the affected product path using existing session authorization. For Chat, use the [local CLI](docs/cli.md) with the same account, conversation, and wording. For UI bugs, use the authenticated browser helper. Before repeating an action, check whether it could duplicate an external effect.
5. Compare the expected behavior, saved records, and actual outcome. For model bugs, inspect the exact prepared context, tool definitions, and provider request when available. Distinguish missing evidence from verified facts.
6. Trace the first disagreement across input, context preparation, provider response, tool execution, persistence, and display. Check authoritative connection health, authentication, permissions, and tool availability before accepting a model's access claim.
7. Fix the responsible rule or implementation with the smallest general correction. Follow the validation rules below for regression coverage. Avoid fixes that recognize only the reported wording.
8. Confirm that the development server runs the rebuilt code. Repeat the original reproduction within the authorized scope. Verify resulting state and recorded execution, not only the assistant's reply. For access bugs, use a small read-only operation when authorized.
9. Restore temporary environment changes. Report the cause, verified behavior, checks, commit, and remaining limits. Disclose test messages and other live changes made during reproduction.

## Product UI Work
- Use the repo-local `noema-product-ui` skill for any task that designs, builds, reviews, or materially changes frontend layout, hierarchy, spacing, density, responsive behavior, or information disclosure.
- Read `docs/frontend/product-design.md` and the closest surface contract before editing. Establish the human's job, the focal action or content, the information priority, and the intended grouping before choosing components or writing CSS.
- Treat Noema as a dense product interface, not a marketing page. Do not add whitespace, cards, headings, icons, metadata, columns, or motion merely to make a screen feel designed; every structural device must communicate a relationship, priority, state, or action.
- Use Astryx components and spacing tokens before one-off controls or raw spacing values. Reuse the existing shell, detail, transcript, and domain patterns instead of creating a parallel presentation for the same concept.
- When editing a field, use either inline editing that saves without a separate Save button, or an actual dialog with an explicit Save button and the first input automatically focused. Do not place an explicit Save flow inline on the page.
- UI requests grant implicit permission for visual browser inspection unless the user explicitly restricts it. Do not request inspection permission again.
- Follow Noema Development Access above for connection details and `docs/frontend/browser-inspection.md` for visual review.
- State any limits when rendered states remain unavailable.

## Review And Subagents
- Keep changes expected to touch fewer than roughly 1,000 lines or two architectural areas inline when delegation overhead would exceed the work.
- Parallel implementers own independently shippable vertical slices, not domain/store/runtime/API layers of one slice. The parent owns the total code and test budget and must stop local completeness from expanding global scope.
- For adversarial review, reviewers should inspect and report findings without editing files.
- Default to one review pass and one correction pass. Fix correctness, security, privacy, data-loss, and demonstrated-regression findings automatically; require explicit scope approval for speculative hardening, extensibility, or additional abstraction. Run another review only when the first found a serious unresolved defect.
- The main agent owns final integration, validation, and the user-facing summary.

## Validation
- During implementation, run focused checks for the affected packages and behavior.
- Use `scripts/with-build-limits` for Go builds, tests, and vet. Web build commands apply the same limits automatically.
- After focused checks pass, run broad validation once for each completed server or native code unit, using affected languages.
- The integrating agent owns broad validation of the combined changes. Other agents run focused checks for their changes.
- Before repeating a check, state the changed inputs or unresolved failure. Relevant inputs include code, dependencies, configuration, test inputs, and environment.
- When those inputs remain unchanged, reuse successful results. A commit, push, report edit, or agent handoff does not invalidate them.
- Record each reused check's command, result, and tested revision or worktree changes in the task summary.
- After a failure, correct its cause. Retry the failed check first. If the correction affects broader results, repeat those checks.
- If a run stops early, complete the unexecuted checks. Do not report them as passed.
- Before using `-count=1`, repeated test runs, or race tests, state the specific execution or concurrency risk.
- For documentation-only changes, check the changed text, links, and Git whitespace. Do not run application builds or test suites.
- For frontend-only changes, use frontend checks and the visual review rules below. Backend checks require a stated dependency or behavior risk.
- For changed scripts or fixtures, check their affected behavior. Do not run unrelated application suites.
- Use additional analysis tools when a named risk or explicit task requirement needs them. Existing CI requirements still apply.
- Never circumvent, disable, bypass, unset, or otherwise interfere with the `sccache` build cache.
- Never modify `CARGO_BUILD_RUSTC_WRAPPER` or attempt to work around the configured Rust compiler wrapper. Doing so invalidates shared cache state, causes 15min+ builds, and can break other agents building in parallel.
- Broad Go server validation:
  - `CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`
  - `CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...`
- Broad retained Rust validation:
  - `cargo fmt --all --check`
  - `cargo check-workspace`
  - `cargo gate-lint`
  - `cargo gate-test`
- Frontend code validation: run `bun run lint` and `bun run build` from `apps/web`.
- Run focused Cargo commands through `scripts/validate-rust <cargo-command> [arguments]`, for example `scripts/validate-rust test -p noema-capability-adapters --lib`.
- Run unit tests only. Do not run smoke tests or fixture tests unless explicitly requested.
- Add tests for unique risks at the authoritative layer. A bug normally gets one regression test; an ordinary feature normally gets three to eight focused tests. More than ten new tests requires a written risk and redundancy justification before implementation continues.
- Do not test derives, getters, constructors, enum mirrors, pass-through mappings/resolvers, or mock interactions unless they enforce an external compatibility or security contract. Do not repeat the same behavior through domain, store, API, and runtime layers unless each boundary owns materially different logic.
- Do not write tests for UI/frontend work unless explicitly requested.
- For frontend or UI work, inspect relevant rendered states at desktop and phone widths. Apply the browser permission rules above.

## Ship Checklist
- Before committing or pushing, run:
  - `git status --short --branch`
  - `git diff --check`
- Confirm that validation covers the current changes under the rules above. Reuse valid results instead of repeating checks.
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
