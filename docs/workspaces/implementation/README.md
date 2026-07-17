# Noema Work Implementation Coordination Contract

**Status:** execution protocol for the implementation packets in this directory.
**Audience:** packet owners, reviewers, and the final integrator.

This directory turns the Work design into independently assignable changes
without creating parallel architecture. The design documents one directory up
remain authoritative for product and technical decisions; an implementation
packet explains how to land a bounded slice of that design.

## Read this before taking a packet

Every packet owner reads these documents in order before editing code:

1. [../README.md](../README.md) for program status and document map.
2. [../00-product-contract.md](../00-product-contract.md) for the first-release
   user contract and non-goals.
3. [../01-system-architecture.md](../01-system-architecture.md) for state
   ownership, transaction boundaries, recovery, and authority rules.
4. The packet's direct design sources:
   [../02-domain-model.md](../02-domain-model.md),
   [../03-storage-and-events.md](../03-storage-and-events.md),
   [../04-commands-and-reconciliation.md](../04-commands-and-reconciliation.md),
   [../05-runtime-and-chat.md](../05-runtime-and-chat.md),
   [../06-graphql-contract.md](../06-graphql-contract.md), and
   [../07-work-ui.md](../07-work-ui.md), as applicable.
5. [../08-validation-and-rollout.md](../08-validation-and-rollout.md) for
   required unit validation and reset/rollout rules.

The existing background-task plan and runtime are implementation context, not
the final Work contract:
[../../superpowers/plans/2026-07-11-background-task-system.md](../../superpowers/plans/2026-07-11-background-task-system.md),
[../../harness/runtime.md](../../harness/runtime.md), and
[../../harness/events.md](../../harness/events.md).

## The fixed public seams

Before parallel implementation starts, the integrator records each row below as
**frozen** in the program status table. A frozen seam is a decision made in the
authoritative design documents, not a provisional type made to get one crate
to compile.

| Seam | Owner of the definition | Consumers | Freeze condition |
| --- | --- | --- | --- |
| IDs, workflow behavior, task/gate/message/contract/run records | Domain packet | Store, Runtime, API, Web generator | Public Rust names, fields, validation behavior, and serialization names match the domain document. |
| Semantic commands and typed errors | Domain packet | Store, Runtime, API, tools, Web | Every command has preconditions, idempotency semantics, generation/revision behavior, and one result shape. |
| Transactional store service and bounded query signatures | Store packet | Runtime, API, Integration | Command inputs/results, query projections, cursor inputs, and claim/lease methods are named and versioned in the packet. |
| Work event vocabulary and outbox identity | Domain plus Store packets | Runtime, API, Web | Every producer, payload owner, cursor rule, and notification destination is specified. |
| Worker terminal contracts and reconciliation actions | Runtime packet | Store, API, test support | Planner/executor/reviewer inputs and terminal outcomes map to semantic commands, not direct state writes. |
| GraphQL names, fields, mutations, errors, and subscriptions | API packet | Web packet, Integration | Schema names and operation inputs agree with the GraphQL contract and generated types. |
| Routes, shared detail boundary, and operation ownership | Web packet | Integration | Route names, generated artifacts, action UX, and refresh/subscription behavior are specified. |

No packet may invent a temporary duplicate enum, a handwritten GraphQL mirror,
a “compatibility” state field, or an alternate command path while a seam is
frozen. The project is pre-V1: if a frozen seam must change, change it
deliberately across its consumers rather than preserving two meanings.

## Dependency waves

~~~mermaid
flowchart TD
    Freeze["Wave 0: Contract freeze"] --> Domain["Wave 1: Domain contracts"]
    Domain --> Store["Wave 2: Store and events"]
    Domain --> Runtime["Wave 2: Runtime and tools"]
    Domain --> APIShape["Wave 2: API schema/projection shape"]
    Store --> API["Wave 3: API resolver completion"]
    APIShape --> API
    API --> Web["Wave 3: Work UI"]
    Store --> Integration["Wave 4: Integration"]
    Runtime --> Integration
    API --> Integration
    Web --> Integration
~~~

### Wave 0 — Contract freeze

**Owner:** integrator, with acknowledgement from each packet owner.

Freeze the public seams table, review the authoritative documents for
unresolved decisions, and make a short dependency map before code changes.
The integrator also reserves generated files, root manifests, and durable
project context. If a package needs a new workspace member such as
`noema-workspaces`, the Domain owner provides the crate-local manifest
fragment during this wave and the integrator owns the corresponding root
workspace change.

Wave 0 ends only when packet owners can implement without asking product or
cross-crate architecture questions. A missing detail is a contract-change
request, not an invitation to choose locally.

### Wave 1 — Domain contracts

**Packet:** [01-domain-contracts.md](01-domain-contracts.md)
**Owner:** Domain agent

Create `noema-workspaces` and evolve `noema-tasks` into the shared Work
contract. This wave replaces the old task-level status abstraction with the
workflow/stage model and establishes the typed vocabulary every other packet
consumes. No Store, Runtime, API, or Web packet begins implementation against
guessed Rust types.

### Wave 2 — Store, Runtime, and API shape

**Packets:** [02-store-and-events.md](02-store-and-events.md),
[03-runtime-and-tools.md](03-runtime-and-tools.md), and
[04-graphql-api.md](04-graphql-api.md).

Store and Runtime may proceed in parallel once Domain is handed off because
they own disjoint crates. The API owner may freeze schema object names,
operation contracts, and projection needs after Domain, but resolver
implementation that requires a Store method waits for the Store handoff.
Runtime must request missing Store methods through the contract-change process
instead of adding a private persistence path.

### Wave 3 — API completion and Work UI

**Packets:** [04-graphql-api.md](04-graphql-api.md) and
[05-work-ui.md](05-work-ui.md).

The API owner completes bounded queries, semantic mutations, errors, and the
event subscription after Store is available. The Web owner starts once the
GraphQL names are frozen and the integrator has generated a baseline schema
artifact. UI work consumes generated types; it does not re-declare task stage,
attention, or valid-action logic in TypeScript.

### Wave 4 — Integration

**Packet:** [06-integration.md](06-integration.md)
**Owner:** integrator

Integrate root manifests and generated artifacts, remove obsolete task-status
routes and APIs, resolve only cross-packet failures, execute the cross-system
scenarios and full validation suite, update `docs/context/current.md`, and
serialize focused commits. Wave 4 is not a second implementation pass: a
defect inside a packet's owned files returns to that packet owner unless the
integrator is resolving a genuinely cross-crate seam.

## Exclusive file ownership

Ownership is exclusive during a packet. “Work-related portions” means new
files and clearly isolated Work code; an owner must coordinate before touching
a shared existing module whose change would alter another packet's API.

| Packet | Owned files/directories | Explicitly forbidden |
| --- | --- | --- |
| Domain contracts | `crates/noema-workspaces/**`, `crates/noema-tasks/**`, including their crate-local manifests and tests. | `Cargo.toml` at the repository root, `Cargo.lock`, Store, Runtime, API, Web, generated files, and `docs/context/current.md`. |
| Store and events | Work-specific files in `crates/noema-store/**`, including the crate-local manifest and store tests. | Domain crate internals, Runtime, API, Web, root manifests/lockfile, generated files, and durable context. |
| Runtime and tools | Work-specific files in `crates/noema-runtime/**`, including the crate-local manifest and runtime tests. | Store implementation files, Domain internals, API, Web, root manifests/lockfile, generated files, and durable context. |
| GraphQL API | Work-specific files in `crates/noema-api/**`, including API tests and crate-local manifest. | Store/Runtime/Domain internals, Web source, root manifests/lockfile, generated files, and durable context. |
| Work UI | Work/task/chat-detail/shell portions of `apps/web/**`, excluding generated output. | `apps/web/src/generated/**`, `apps/web/src/routeTree.gen.ts`, backend crates, root manifests/lockfile, and durable context. |
| Integrator | Root `Cargo.toml`, `Cargo.lock`, workspace wiring, generated GraphQL/route files, cross-crate fixes, `docs/context/current.md`, and final validation artifacts. | Product redesign or broad packet-internal rewrites without returning the work to its owner. |

The integrator owns `apps/web/src/generated/graphql.ts`,
`apps/web/src/generated/schema.graphql`, and
`apps/web/src/routeTree.gen.ts` because generation changes can span API and Web
work. Packet owners may request a generation run and inspect its output but
must not stage or edit those files directly.

If two packets need to touch the same existing file, stop before editing it.
The requester sends a contract-change request; the integrator either moves the
file into one packet's ownership or performs the minimal shared edit during an
integration boundary.

## Contract-change protocol

A contract change is required for any change to a frozen public type, command,
store method, event kind/payload, GraphQL name, generated operation shape, or
cross-crate behavior. It is required even when the proposed change looks
source-compatible, because Work's semantics are more important than a local
compile result.

Send the integrator this exact information:

~~~text
Contract change request
-----------------------
Packet and owner:
Frozen seam affected:
Problem observed:
Proposed exact contract change:
Why the current contract cannot safely express it:
Affected packet owners:
Stage/generation/event implications:
Validation to add or update:
~~~

The integrator evaluates the request with every affected owner, updates the
authoritative design document before code when the decision is material, then
announces the revised frozen seam. No owner works around a pending request with
an adapter, duplicate type, undocumented JSON field, or temporary direct SQL
call.

## Packet contract

Every implementation packet in this directory must contain all of the
following sections. The packet template is a checklist for work, not a
replacement design spec.

| Required section | What it must make unambiguous |
| --- | --- |
| Mode | Usually `implementation`; state whether it is a prerequisite, integration, or review packet. |
| Goal | The observable subsystem outcome and the design documents it implements. |
| Prerequisites | Frozen seams, preceding packet handoffs, generated artifacts, and configuration needed before editing. |
| Owned files | Exact directories/files the agent may change. |
| Forbidden files | Neighboring ownership boundaries and generated/root exceptions. |
| Consumes | Concrete types, commands, store services, events, GraphQL objects, or route contracts from prior packets. |
| Produces | Concrete public interfaces and behavior that downstream owners may rely on. |
| Checklist | Ordered implementation steps with clear completion conditions. |
| Focused tests | Unit tests that prove this packet's semantics without using smoke or fixture tests. |
| Full validation | The package-level commands and the conditions under which workspace/frontend checks must run. |
| Expected commit | A narrow commit subject and the exact boundary it represents. |
| Handoff checklist | The report format below, including open assumptions and contract requests. |

Packet owners should keep source files below the project's normal 750-line
threshold. If a packet touches a larger file, it either extracts a coherent
Work module or explains why the boundary cannot yet be split.

## Working protocol

### Before editing

1. Read the assigned packet and its linked design documents.
2. Run `git status --short --branch` and record unrelated modified/untracked
   files. Preserve all of them.
3. Confirm ownership boundaries with the integrator and obtain the current
   frozen-seam revision.
4. Identify the smallest test seam that can validate the packet's behavior.

### During implementation

1. Use the domain command vocabulary. Do not expose a generic stage mutation
   to simplify a local test or resolver.
2. Keep task stage, run status, gate, review, and contract data separate in
   code and GraphQL projections.
3. Use the command service for every persistent transition. Store repositories
   may expose narrow primitives, but Runtime and API do not assemble
   cross-table state changes themselves.
4. Keep every external effect behind the existing capability and approval
   boundary. Workspace/project context is descriptive until a later phase
   explicitly adds governed scope behavior.
5. Report a contract problem immediately rather than modifying another
   packet's files.

### At packet handoff

The owner stops after its owned slice is coherent, runs focused validation, and
hands the patch to the integrator. In a shared working tree, packet owners do
not stage unrelated files or race each other on the Git index. The integrator
serializes staging and creates the focused packet commit after accepting the
handoff.

Use this handoff format:

~~~text
Work packet handoff
-------------------
Packet:
Status: complete | blocked | needs-contract-change
Commit subject expected:
Changed owned files:
Public interfaces produced:
Interfaces consumed:
Behavior covered:
Focused validation run and result:
Validation intentionally deferred and why:
Known limitations / follow-up:
Contract changes requested or accepted:
Assumptions made (write "none" when fully specified):
Unrelated worktree changes observed:
~~~

“Complete” means the packet has met its own checklist and has no unreported
cross-packet dependency. It does not mean the whole product compiles before
the integrator wires all crates and generated artifacts together.

## Validation and commit protocol

Only unit tests are in scope. Do not add smoke tests, fixtures, browser
inspection, or test-only compatibility paths for this program.

Packet owners run the smallest meaningful checks as their dependencies allow.
The integrator runs the full gate once packet changes are assembled:

~~~bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
git diff --check
~~~

For frontend integration, the integrator owns generation and runs:

~~~bash
cd apps/web
bun run check:generated
bun run lint
bun run build
bun run build:tauri
~~~

`check:generated` and `lint` currently regenerate schema/routes, so they must
run under the integrator's generated-file ownership. The Web owner can request
that generation before validating source changes, but must not commit generated
output itself.

Before every serialized commit, the integrator inspects:

~~~bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
~~~

Expected commits are one per accepted packet plus one integration commit. A
typical sequence is:

~~~text
work(domain): add workflow task contracts
work(store): persist work commands and event ledger
work(runtime): reconcile and execute work tasks
work(api): expose semantic Work GraphQL contract
work(web): add Work board and task controls
work(integration): wire Work across the workspace
~~~

The exact subject may change, but a commit must not mix unrelated dirty
worktree changes. The integrator reports any untracked or unstaged files after
each boundary and updates `docs/context/current.md` with durable decisions and
remaining work.

## Integration protocol

1. **Accept handoffs in dependency order.** The integrator checks the packet
   report, reviews the owned diff, and verifies that no forbidden file changed.
2. **Wire shared surfaces once.** Root workspace members, crate dependency
   edges, generated GraphQL schema/operations, and generated routes are added
   only by the integrator after the producing packet is accepted.
3. **Resolve genuine seams, not local rewrites.** If a compile failure exposes a
   packet-internal defect, return it to that owner. If it exposes an agreed
   interface mismatch, use the contract-change protocol.
4. **Exercise cross-system scenarios.** At minimum, verify capture, queue,
   planner clarification, execution/review, reviewer approval, acceptance,
   request changes, recovery, cancellation, reopen, event replay, notification
   deduplication, GraphQL cursor reconnect, and Work UI action refresh.
5. **Run the full validation gate.** Fix failures in their owning packet where
   practical, then rerun affected checks and the complete final gate.
6. **Close the program boundary.** Update durable context, record the schema
   reset/bootstrap steps, commit integration, and state any remaining deferred
   work explicitly.

## Coordination rules that cannot be waived locally

- Do not add `TaskStatus`, an execution-phase field, or a frontend status
  reducer as a convenience layer. Task stage, current run, active gate, and
  latest review are deliberately separate projections.
- Do not infer user intent from English phrase matching. Use the primary
  model's structured tool choice and server policy.
- Do not infer project assignment from conversational recency, and do not let
  a project choose authority, capability grants, or memory retrieval.
- Do not write root manifests, lockfiles, generated artifacts, or
  `docs/context/current.md` from a non-integrator packet.
- Do not reset, revert, clean, stage, or commit unrelated worktree changes.
- Do not bypass the configured Rust compiler cache or modify
  `CARGO_BUILD_RUSTC_WRAPPER`.

Following this protocol keeps the implementation parallel where the code is
truly independent, while preserving one coherent Work model at the seams that
matter.
