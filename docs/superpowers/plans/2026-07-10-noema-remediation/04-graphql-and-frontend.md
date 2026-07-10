# Phase 4: GraphQL and Frontend Simplification

**Target time:** 1 hour 30 minutes
**Hard cap:** 2 aggregate agent-hours
**LOC target:** at least 1,000 net lines deleted
**Prerequisite:** backend contracts from Phases 1–3 are stable

## Goal

Remove GraphQL test/setup duplication, redundant frontend transport and
controller code, dead dependencies, and stale product states. Fix a small set of
high-impact UX semantics while ending with less code.

## Files in Scope

- `crates/noema-core/src/graphql/schema.rs`
- GraphQL domain resolver modules and test helpers
- `crates/noema-core/web/src/app/App.tsx`
- `crates/noema-core/web/src/graphql/operations.ts`
- browser and desktop GraphQL transport modules
- transcript/composer execution-state modules
- provider, memory, and MCP status surfaces touched by contract consolidation
- `crates/noema-core/web/package.json` and lockfile
- obsolete wrappers, props, state, styles, and generated artifacts made unused

## Sequence

### Checkpoint 4A — GraphQL consolidation (45 minutes)

- [x] Keep one cached schema per runtime host and delete per-request schema
  construction left after Phase 1.
- [ ] Move repeated GraphQL setup, fixtures, and response assertions into small
  shared test support. Preserve distinct behavioral tests; delete copy/pasted
  setup and tests of removed transport internals.
- [ ] Split `schema.rs` tests beside domain resolvers only when the move also
  removes duplicated setup or broad imports. Pure movement is optional.
- [ ] Replace raw internal error strings at touched boundaries with one compact
  typed public error shape; do not build a new global error taxonomy.
- [ ] Split handwritten GraphQL operations by domain or move them to colocated
  `.graphql` documents only when generation removes handwritten TypeScript.
- [x] Regenerate schema and operation types once at the end of the checkpoint.

Checkpoint target: at least 500 net lines deleted.

### Checkpoint 4B — Product state and dead UI (45 minutes)

- [ ] Model composer state with one discriminated execution state so drafting,
  pending submission, streaming, cancellation, and recovery cannot form invalid
  boolean combinations.
- [ ] Prevent duplicate submit while preserving drafting during a running turn.
- [ ] Add Stop and stop-and-replace behavior using Phase 3 operations; delete
  old ad hoc pending/stream flags and handlers.
- [ ] Render a stable empty-conversation state instead of a perpetual skeleton.
- [ ] Reuse one provider/memory/MCP service-status representation and remove
  components that infer state from English strings.
- [x] Remove obsolete barrel wrappers, dead state/prop chains, unused animation
  rules, and dependencies confirmed unused by `rg` and the package graph.
- [x] Remove `@xyflow/react`, `d3-force`, and `@types/d3-force` if still unused.
- [ ] Keep one Noema-owned React component per file, but do not split components
  solely for style when doing so adds LOC.

## Accessibility Scope

Within touched components:

- use native buttons/forms and existing Astryx primitives;
- preserve keyboard focus after submit/cancel/recovery;
- expose concise status announcements through one existing live region;
- use Unicode-safe IDs rather than English/punctuation-derived anchors;
- preserve `dir` inheritance and reduced-motion behavior;
- remove direct `I am`/`I'm` or English phrase matching used as semantic
  authority.

Broader visual redesign, browser automation, screenshot matrices, and a new UI
framework are out of scope.

## Validation

Backend:

```bash
cargo fmt --all --check
cargo check -p noema-core -p noema-desktop
cargo test -p noema-core graphql:: -- --test-threads=1
```

Frontend:

```bash
cd crates/noema-core/web
bun run gen:types
bun run gen:routes
bun run lint
bun run build
```

Do not add or run frontend unit tests or browser tooling.

## Exit Evidence

- [x] At least 1,000 net source lines deleted.
- [x] Generated GraphQL and route artifacts are current.
- [x] The initial bundle is no larger than the Phase 0 baseline; record all
  emitted chunk sizes.
- [ ] Duplicate submission and empty-state behavior are represented by backend
  tests and type-checked frontend state.
- [x] No English phrase matching remains in touched authority paths.
- [x] No newly unused frontend dependency remains.

## Completion Record

Completed at `90,541` maintained source lines: `-1,846` in this phase and
`-3,383` from the program baseline. Commits: `93829ed7`, `d73b9e2b`, and
`5e7510b6`.

The phase removed the entirely non-enforced MCP owner-extractor/trusted-identity
backend, GraphQL, generated, routing, and Settings surfaces, including 200+
lines of English field-name ownership inference. Reviewed calibration,
fingerprint, disabled, health, and authentication gates remain. Mixed ownership
is always blocked: new Ready+Mixed saves fail, legacy rows project blocked,
runtime eligibility rejects them, and server enabled state requires both the
stored enable bit and a Ready non-mixed calibration. Independent Luna-max review
found no remaining Critical or Important issue.

Frontend generation, route generation, lint, and production build passed. The
largest emitted chunks were `app.js` at 924.75 kB (265.47 kB gzip) and
`SettingsPage.js` at 102.62 kB (22.69 kB gzip), down from 927.32 kB and 111.41
kB at the preceding recorded build. The full workspace gate passed with 661
core library, 10 development-binary, and 7 desktop tests.

Composer cancellation/state-machine work was not attempted because Phase 3 did
not add public cancel/interrupt operations; inventing a frontend-only state path
would have violated the phase boundary. Remaining GraphQL test consolidation
and controller decomposition stay available for measured crate/closure work.

## Stop Conditions

- Skip a state-machine library if a TypeScript discriminated union is smaller.
- Do not trade test coverage for the LOC target.
- Do not perform visual redesign or broad component movement.
- At the hard cap, preserve the last generated/lint/build-green checkpoint and
  defer remaining UX items.
