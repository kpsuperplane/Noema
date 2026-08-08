# Overengineering reduction roadmap

> Implementation results and final dispositions are recorded in the
> [remediation report](2026-08-08-overengineering-remediation.md). This roadmap
> remains the historical execution plan and is not the current status authority.

This roadmap converts the [audit findings](2026-08-08-overengineering-audit.md)
into independently shippable units. It is intentionally deletion-first and does
not authorize any product-scope decision gate.

## Operating rules

Every slice should:

1. start from a clean, recorded baseline and preserve unrelated work;
2. name one authority being removed or strengthened;
3. remain independently reviewable and committable;
4. use `--require-net-negative` for refactors;
5. count generated output, production code, tests, and documentation separately;
6. add only tests that protect a unique risk;
7. stop if a general compatibility layer, framework, DTO hierarchy, or test harness
   is needed to complete the reduction;
8. preserve current information handling, migrations, approvals, fencing,
   ambiguous-outcome behavior, and exact external protocol contracts;
9. update the closest durable authority, not this audit, when a design decision is
   accepted;
10. make one commit per completed slice and never combine a product decision with a
    mechanical reduction.

For estimates below, “production” means authored Rust/TypeScript/Swift/GraphQL,
not generated client output. Deletion may substantially exceed the negative budget;
the budget limits additions and net growth.

## Milestone 0 — Repair current contract failures

### S0.1 — Synchronize native GraphQL generation

**Outcome:** Every committed iOS operation is generated from an authored operation
that validates against the current schema, and production code uses
`credentialSetup`/`SetupAdapterConnectionInput`.

**Non-goals:** changing Apollo, removing committed generated output, sharing web and
iOS documents, redesigning credential setup UI.

**Reuse target:** existing `graphql/schema.graphql`, current iOS operation documents,
Apollo CLI/configuration, and one clean-diff pattern analogous to the web check.

**Expected files:**

- `apps/ios/Noema/Operations/Chat.graphql`
- `apps/ios/Noema/Operations/Settings.graphql`
- Settings/Chat credential model and views
- generated Apollo Swift
- one existing validation entrypoint or small command/documentation update

**Budget:** up to +150 authored production lines because this repairs current user
functionality; generated churn excluded. At most 0 new Swift unit tests unless a
test target already exists; rely on schema validation and compile.

**Validation and unique risks:**

- validate all authored and generated operations against the same schema;
- regeneration is clean on a second run;
- build the iOS target;
- exercise adapter definition review, every supported credential input kind,
  connection setup, and pending chat interventions;
- confirm no retired field/mutation name remains in authored or generated operation
  strings.

**Stop conditions:** stop if repair requires hand-editing generated output, keeping
both old and new credential APIs, or adding a custom generator around Apollo.

### S0.2 — Remove Work-event heuristic secrecy

**Outcome:** closed Work-event payload schemas remain intact while ordinary/private
values are no longer rejected by English key-name guesses.

**Non-goals:** changing event vocabulary, payload size, schema migrations, or secret
handling at actual capability/provider boundaries.

**Budget:** production net at most -20; test net at most +20; one focused regression
test.

**Validation and unique risks:**

- ordinary values survive event construction/persistence;
- unknown fields, wrong types, oversized payloads, and invalid kind invariants still
  fail;
- no actual secret-bearing structure becomes permitted because arbitrary fields
  remain closed.

**Stop conditions:** stop if a general secret scanner or replacement heuristic is
proposed. The correct authority is the typed event schema.

### S0.3 — Correct ordinary-value Debug preservation

**Outcome:** routing IDs, normalized non-credential endpoints, public OAuth client
ID/attempt IDs, and local paths remain visible in diagnostics; actual credentials
and transient auth secrets remain excluded.

**Non-goals:** changing persistence, model context, authorization, egress policy, or
secret wrapper ownership.

**Budget:** production net at most -15; test net at most +40; three to six paired
assertions across the existing tests, not a new test module.

**Validation and unique risks:**

- preserve representative ID/path/safe endpoint at each touched boundary;
- exclude API key, access/refresh token, auth code, state, PKCE verifier, user code,
  and credential-bearing URL component;
- show only an exact sanitized URL derivative where ordinary and secret components
  coexist.

**Stop conditions:** stop a slice that starts changing unrelated redaction sites or
cannot classify a value from type/provenance.

## Milestone 1 — Delete code with no executable consumer

### S1.1 — Remove dormant adapter events

**Outcome:** strict adapter manifests reject event configuration because no event
ingress exists; event HMAC/challenge/deduplication code is absent.

**Budget:** production ≤ -330, tests ≤ -120, no more than two replacement tests.

**Validation and unique risks:**

- a currently supported adapter still compiles and runs;
- an `event` field fails closed with a clear unsupported/unknown-field diagnostic;
- strict unknown-field behavior is unchanged elsewhere.

**Stop conditions:** stop if deletion triggers creation of a compatibility parser or
stub event interface. Reintroduce only with an ingress and consumer.

### S1.2 — Narrow adapter continuation and policy schema

**Outcome:** the executable adapter definition advertises only continuation and
policy behavior that runtime actually enforces.

**Decision required inside the slice:** choose the next strict adapter schema/digest
version and whether development definitions are regenerated/reset. This is a format
decision, not a general backwards-compatibility project.

**Budget:** production ≤ -250, tests ≤ -80, three to six focused tests.

**Validation and unique risks:**

- `None` and `ResponseToken` compile and execute unchanged;
- removed continuation kinds fail at compile/setup, never silently at invocation;
- quota/gate fields are rejected rather than ignored;
- definition digest/replacement behavior converges after reset or explicit version
  transition;
- no second policy layer is introduced.

### S1.3 — Delete the test-only cursor authority

**Outcome:** durable cursor persistence is the only cursor state authority.

**Budget:** production/tests combined ≤ -160; no production additions beyond test
fixture plumbing.

**Validation and unique risks:** use temporary durable storage to cover open/reopen,
expiry, stale handle, successful rotation, failed commit atomicity, and original
argument/revision binding. Do not preserve `FullResyncRequired` if production cannot
enter it.

### S1.4 — Remove unreachable client code

**Outcome:** unimported web components, dead task helpers/exports, and unused
`TasksSidebar` view are gone.

**Budget:** authored production ≤ -700; no new UI tests.

**Validation:** web typecheck/lint/build/test and iOS compile. Search for deleted
symbols and dynamic import strings. No visual redesign is part of this slice.

### S1.5 — Remove inactive repository gates

**Outcome:** the repository contains only validation machinery that is wired and
passes at the current baseline.

**Precondition:** check external CI/release configuration for direct invocation.

**Choice:**

- delete all three systems if no active policy depends on them; or
- retain one compact Cargo-metadata-derived boundary gate and delete the focused
  tree and test-name registry.

**Budget:** tooling/docs ≤ -1,600; any retained checker ≤ 250 production lines and
≤ 100 table-driven test lines.

**Validation:** current Cargo validation aliases, size reporting, and web generation
checks still work. No exact test-name manifest remains.

### S1.6 — Dead dependency, index, and public-surface sweep

Keep this as several tiny commits if ownership differs.

**Candidates:**

- remove `noema-store -> noema-memory`;
- remove unread `memory_pages` creation/clearing/writes;
- delete verified unused store methods/constants or reduce visibility;
- remove test-only task-model-pool paths;
- remove `noema-home` force option and public test-only diagnostics surface;
- remove test-only artifact operation-ID polymorphism;
- replace one `chrono` parse with `jiff` and remove the direct dependency.

**Budget per commit:** production net negative; test net no greater than +30; zero
new test declarations unless a unique external behavior needs one.

**Validation and unique risks:** focused crate tests plus workspace check; memory
initialize/publish/search/rebuild; task pool readiness/stale selection through the
real transactional path; artifact collision/traversal behavior unchanged.

## Milestone 2 — Consolidate duplicate authorities

### S2.1 — One Work-event decoder and one criteria loader

**Outcome:** typed records are immutable, event data is validated at construction
and persisted-row ingress, all reads use one decoder/column authority, and criteria
are queried by one batch helper.

**Budget:** production ≤ -90, tests ≤ +60, three to six focused tests.

**Validation and unique risks:** forward/backward pagination, recovery, notification
source loading, malformed stored payload, missing/duplicate criteria, exact
submission/review coverage.

**Stop conditions:** do not create a generic SQL row-decoder framework or move
domain validation into the store.

### S2.2 — One artifact metadata writer

**Outcome:** every artifact metadata create/append goes through
`ArtifactMetadataStore`; domain errors cross the boundary once.

**Budget:** production ≤ -60, tests ≤ +100, four to eight focused tests.

**Validation and unique risks:** external-URL create authorization, local create,
append conflict, busy retry, storage-kind mismatch, transaction rollback, and
filesystem publication cancellation.

**Stop conditions:** keep `ArtifactMetadataStore`, `ArtifactOperations`, and their
crate boundary. Do not merge filesystem and SQLite responsibilities.

### S2.3 — One provider account authority

**Outcome:** account metadata has one representation and selectable system backends
do not require a synthetic-account merge plus persistence bypass.

**Preferred direction:** persist selectable built-ins through a forward-only
migration and derive capabilities from one record. If built-ins are not actually
accounts, model them explicitly outside accounts instead; do not retain both.

**Budget:** production ≤ -150, tests ≤ +120, five to eight focused tests. Any schema
change must append a migration and advance the version.

**Validation and unique risks:** existing-version upgrade, fresh convergence,
default/built-in account visibility, capability assignment integrity, deletion
fencing, runtime selection, and web backend resolution.

**Stop conditions:** stop if a compatibility DTO or third account type appears.

### S2.4 — One provider telemetry authority

**Outcome:** runtime debug spans/profile are the sole newly-written provider usage
telemetry; web and iOS do not parse legacy transcript metadata.

**Precondition:** explicit decision on retention of pre-V1 development transcripts.

**Budget:** authored production ≤ -180, tests ≤ -60; generated churn excluded.

**Validation and unique risks:** provider/model/phase/round indices, input/output/
cached tokens, zero versus missing cache usage, debug dialog/native presentation,
and absence of duplicate transcript metadata writes.

**Stop conditions:** do not add a versioned telemetry adapter framework. If old data
must remain temporarily visible, give the compatibility path a dated deletion
condition.

### S2.5 — Remove single-implementation routing traits

**Outcome:** `CapabilityRegistryRouter` and `RegistryProviderRouteResolver` expose
inherent operations; real lower-level seams remain.

**Budget:** production/tests combined ≤ -40; no new tests.

**Validation:** focused capability routing and provider selection/lease race tests,
then workspace check.

**Stop conditions:** retain `ProviderSelectionLoader`, capability invokers, payload
sanitizers, providers, and backends. Do not replace trait objects with generics
through runtime.

### S2.6 — Consolidate native web binding/execution

**Outcome:** foreground and approved web paths share one concrete binding builder and
executor; approval admission/state transitions remain separate.

**Budget:** production ≤ -60, tests ≤ +120, four to eight focused tests.

**Validation and unique risks:** identical destination/behavior/revision, fallback,
auth updates, observed URL recording, persisted sanitization, reviewed argument
digest, and outcome-uncertain behavior through both paths.

**Stop conditions:** never bypass immutable catalog snapshots or approval. Defer the
`RuntimeExecutionInvoker` removal until this slice is stable and a second measured
net-negative sketch exists.

### S2.7 — One adapter snapshot authority

**Outcome:** API and host consume the service-owned adapter snapshot/projections
instead of reconstructing filesystem stores independently.

**Budget:** production ≤ -15, tests ≤ +60.

**Validation:** startup filesystem/SQLite convergence, superseded-definition
reporting, connection projections, and API reads from a coherent snapshot.

**Stop conditions:** expose only missing concrete projections; do not turn the
service into a generic repository facade.

## Milestone 3 — Simplify client contracts

### S3.1 — Add iOS GraphQL fragments and delete generic mappers

**Outcome:** repeated Settings and Memory shapes are authored as fragments and
mapped once in Swift.

**Budget:** authored GraphQL/Swift ≤ -450 combined; generated output measured
separately; no new Swift protocol/DTO hierarchy.

**Validation:** regenerate cleanly, compile, and exercise Settings model preferences,
provider/local model/account/capability flows plus Memory list/page/subscription/
update flows.

**Stop conditions:** do not combine with shared web/iOS operation documents. First
prove local fragment reduction.

### S3.2 — Optionally share exact GraphQL documents

**Precondition:** S3.1 complete and exact-match inventory refreshed.

**Outcome:** only structurally identical, platform-neutral documents/fragments live
under one root GraphQL source directory; platform-specific operations remain local.

**Budget:** authored GraphQL ≤ -300 after configuration changes. Stop if codegen
configuration/runtime document plumbing costs more than removed duplication.

**Validation:** both generators, web build/tests, iOS compile, and exact operation
name/selection comparison.

### S3.3 — Collapse one-consumer web layers

Split this by domain, not into one global Settings rewrite.

**Candidates:**

- colocate each Settings pane with its Apollo state and delete the one-use content
  prop mirror;
- make one task-status projection authority;
- route PWA transcript reconciliation through the canonical transcript loader;
- align Astryx/StyleX versions through the required upgrade workflow, then remove
  per-file `xstyle` cast adapters.

**Budget per domain:** production net negative; no new UI tests. The status/loader
logic slices may add one small table-driven non-visual test each.

**Validation:** web generated-clean check, typecheck, lint, build, and existing tests.
Use the repository UI workflow for any material visual/layout change; these proposed
changes should not alter layout.

## Milestone 4 — Compatibility sunset

Do not start this milestone until supported development-data and upgrade windows
are explicit.

### S4.1 — Remove provider-placeholder startup reconciliation

**Outcome:** no historical account reference scan or directory deletion runs on
every boot.

**Choice:** remove after the support window, or use one explicit version marker/
forward-only migration if transition is still required.

**Validation:** referenced accounts are never deleted, an existing supported
version upgrades, and a fresh home converges. Never rewrite an applied migration.

### S4.2 — Remove retired filesystem and transcript compatibility

**Candidates:** `mnemosyne` startup deletion, English-prefixed display parsing,
legacy provider-usage metadata, retired task-model aliases, and orphaned generated
native operations.

**Exclusions:** SQLite v9 adoption and documented iconless memory-page reads.

**Budget:** net negative in every independently committed slice. No generic
compatibility framework.

## Product decision gates

No implementation begins without explicit direction.

| Gate | Decision | If retained | If removed/deferred |
| --- | --- | --- | --- |
| Adapter scheduler | Is scheduled adapter execution a near-term product capability? | Add a real producer and event-driven worker; stop calling dormant polling machinery “complete” | Delete ~750–850 LOC and use one small legacy-directory transition |
| Native adapters and MCP | Are both integration substrates first-class? | Keep distinct auth/transport/security ownership and remove only dormant overlap | Remove one complete vertical stack; do not build a universal replacement |
| Web and iOS | Are both full-parity clients strategic now? | Share source contracts/fragments only; keep platform code separate | Defer one client for the largest client-side reduction |
| Work roles | Are planner, executor, reviewer, ACP, recurrence, gates, and review all first-class? | Preserve the state model; consolidate readers/errors only | Remove complete roles/workflows with explicit migration/product design |
| A2UI, PWA, evals, debug | Does each have current adoption and a roadmap? | Keep vertical slices bounded and measured | Retire a whole slice rather than adding general frameworks |

## Documentation reduction sequence

Documentation cleanup is large enough to deserve its own three-step unit:

1. **Authority map:** classify each current document as active authority, material to
   extract, historical, or superseded. Identify unique security/protocol rules before
   deletion.
2. **Extraction and rewrite:** move unique durable rules into the closest active
   subsystem authority; rewrite frontend current contract from route/schema source;
   reduce `context/current.md` to active direction and open loops.
3. **Removal:** delete completed/superseded plans/specs, obsolete frontend/harness
   architecture, `docs/postgres.md`, and the old audit tracker. Git history is the
   archive.

Expected doc reduction is 42,000–49,000 lines. This unit has zero production/test
budget. Validate internal links, retired term searches, route/schema claims, Markdown
formatting, and the context size/content contract.

## Validation matrix

Use the narrowest validation that proves each slice, then the standard repository
gate before commit.

| Change type | Focused validation | Pre-commit validation |
| --- | --- | --- |
| Rust deletion/consolidation | `cargo validate test -p <owner> --lib` with named unique-risk tests | `cargo fmt --all --check`, `cargo check-workspace`, `cargo gate-lint`, `cargo gate-test` |
| Rust refactor budget | size report against slice base with production/test/new-test caps and `--require-net-negative` | repeat size report before commit |
| Web logic/deletion | generated-clean, typecheck/lint, build, existing tests | diff check and staged scope inspection |
| iOS operation/model | schema validation, Apollo generation twice, iOS build | verify generated diff and no retired operation strings |
| Docs/tooling | run retained tools, link/term/source claim checks | diff check and zero Rust growth report |
| Schema/account change | focused existing-version upgrade and fresh convergence | full Rust gates; never edit an applied migration |

## Completion criteria

The reduction program is complete when:

- every accepted slice has one clear authority and no parallel fallback left behind;
- every refactor is net-negative within its declared production/test budget;
- removed feature-shaped code has no advertised schema or documentation residue;
- compatibility retained temporarily has a named supported window and deletion
  condition;
- ordinary diagnostics remain observable while true secrets remain excluded;
- current docs describe current routes, storage, state, and validation;
- decision-gate systems are either real vertical product slices or absent;
- no protected migration, approval, protocol, security, or data-loss invariant has
  been traded for a superficial LOC reduction.
