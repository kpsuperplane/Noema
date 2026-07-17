# Noema Work

## Status

This package is the authoritative design and implementation program for Noema's
first workspace, project, and task-management release. It expands the existing
background-task system into a chat-first Work system without introducing a
second task object or importing coding-project assumptions into Noema's core.

The documents describe the target contract. Until the implementation packets
land, the current `TaskStatus`-based background-task implementation remains the
running code.

| Area | Design | Implementation |
| --- | --- | --- |
| Product contract | Settled | Not started |
| Workspace/project/task domain | Settled | Not started |
| SQLite and event ledger | Settled | Not started |
| Commands and reconciliation | Settled | Not started |
| Runtime and chat integration | Settled | Not started |
| GraphQL contract | Settled | Not started |
| Work UI | Settled | Not started |
| Validation and rollout | Settled | Not started |

Update this table only when an implementation packet has passed its acceptance
gate. "In progress" means an agent owns the packet; it does not mean partially
implemented behavior is a supported product contract.

## Product Direction

Noema remains centered on one simple, primary conversation. Work adds durable
organization and asynchronous execution underneath that conversation, plus a
more powerful `/work` surface for users who want to inspect and manage it
directly.

The first release has one Personal workspace, optional projects, and one
canonical task object. Every task is intended for agent execution. Inbox holds
captured work that has not been authorized; Queue authorizes Noema to plan and
execute it; Doing covers automated work; Waiting and Review are the human gates;
Completed and Cancelled live in history.

## Source Of Truth

Noema intentionally does not persist separate workflow, lifecycle, and
task-execution status axes.

| Question | Authority |
| --- | --- |
| Where is the task in the user workflow? | `tasks.stage_id` |
| What is an agent doing right now? | Current `agent_runs.run_kind` and `agent_runs.status` |
| What requirements and context govern this attempt? | Immutable `task_execution_contracts` version |
| Why does the task need the human? | Unresolved `task_gates`, or an approved review while the task is in Review |
| What happened and in what order? | Monotonic `work_events` ledger |
| What result can be accepted? | Latest submission and independent review records |

UI labels such as "reviewer queued" or "needs clarification" are projections
of those authorities. They must never become another mutable task-status field.

## Reading Order

1. [Product contract](00-product-contract.md) defines the experience, user
   journeys, release boundary, and non-goals.
2. [System architecture](01-system-architecture.md) assigns subsystem ownership
   and describes the end-to-end data flows.
3. [Domain model](02-domain-model.md) defines the public types, stage semantics,
   commands, invariants, and transition matrix.
4. [Storage and events](03-storage-and-events.md) defines the pre-V1 schema,
   transactional recipes, indexes, cursors, and idempotency rules.
5. [Commands and reconciliation](04-commands-and-reconciliation.md) defines
   every state-changing operation and crash-recovery decision.
6. [Runtime and chat](05-runtime-and-chat.md) defines planning, execution,
   review, tool visibility, context, gates, and chat delivery.
7. [GraphQL contract](06-graphql-contract.md) defines all first-party client
   queries, mutations, subscriptions, errors, and bounded projections.
8. [Work UI](07-work-ui.md) defines routes, views, interactions, responsive
   behavior, accessibility, and reuse of the current task detail rail.
9. [Validation and rollout](08-validation-and-rollout.md) maps requirements to
   tests and defines the schema-reset release gate.
10. [Future roadmap](09-future-roadmap.md) sequences deferred orchestration,
    planning, collaboration, context, and integration work.

Implementation begins with the [multi-agent program](implementation/README.md).
Each packet is self-contained, owns disjoint files, and ends with a concrete
handoff and validation gate.

## Relationship To Existing Documentation

This package specializes and extends, rather than replaces, the broader Noema
contracts:

- [Project direction](../project.md) defines Noema's overall product and object
  model.
- [Current durable context](../context/current.md) records the implementation
  that exists today.
- [Background task system](../superpowers/plans/2026-07-11-background-task-system.md)
  defines the executor/reviewer foundation being generalized.
- [Frontend object model](../frontend/object-model.md) defines common governed
  objects and presentation principles.
- [Frontend navigation workflows](../frontend/navigation-workflows.md) defines
  chat-led progressive disclosure.
- [Harness runtime](../harness/runtime.md), [capabilities](../harness/capabilities.md),
  and [security](../harness/security.md) remain authoritative for run envelopes,
  capabilities, approvals, and trust boundaries.
- [SQLite](../sqlite.md) remains authoritative for local database ownership and
  location.

If this package conflicts with a general security, capability, or trust rule,
the stricter general rule wins. If it conflicts with the current task schema or
API, this package describes the deliberate pre-V1 replacement.

## Claude-Kanban Mapping

The local [Claude-Kanban](https://github.com/PeiAllen/claude-kanban) repository
is useful architectural prior art, but Noema has a different product center.

| Treatment | Concepts |
| --- | --- |
| Borrow | One backend source of truth, semantic commands, idempotent reconciliation, leases and generation fencing, durable inboxes, monotonic event feeds, attention queues, and archive/reopen |
| Adapt | A card becomes a general Noema task; a live coding session becomes a bounded planner/executor/reviewer run; Needs You covers clarification, approval, recovery, and human acceptance |
| Reject from core | Repository scanning, worktrees, branches, Git status, terminals, tmux, diffs, commits, pull requests, SSH topology, and the assumption that one task equals one process/session |

Coding workflows may later implement optional execution adapters. They must not
change the Work-domain task, stage, command, gate, or event contracts.

## Settled First-Release Defaults

- The only exposed workspace is seeded `workspace:personal`.
- Projects are optional task containers with a name, description, and archive
  state.
- The global conversation has no sticky project scope. A task receives a
  project only from an explicit reference.
- All task execution uses the existing global executor and reviewer pools.
- Human acceptance is required after automated reviewer approval.
- Board transitions are semantic actions, not freeform drag-and-drop.
- Task metadata is intentionally minimal: title, description, optional project,
  stage, provenance, and derived run/review information.
- Schema changes rewrite the pre-V1 bootstrap schema directly; there is no
  compatibility layer or mixed-schema operation.

## Implementation Program

| Packet | Owner area | Depends on |
| --- | --- | --- |
| [Domain contracts](implementation/01-domain-contracts.md) | `noema-workspaces`, `noema-tasks` | Contract freeze |
| [Store and events](implementation/02-store-and-events.md) | Work-related `noema-store` modules | Domain contracts |
| [Runtime and tools](implementation/03-runtime-and-tools.md) | Work-related `noema-runtime` modules | Domain contracts and store signatures |
| [GraphQL API](implementation/04-graphql-api.md) | Work-related `noema-api` modules | Domain contracts and store signatures |
| [Work UI](implementation/05-work-ui.md) | Work/task/shell areas of `apps/web` | Frozen GraphQL schema |
| [Integration](implementation/06-integration.md) | Manifests, generated files, cross-crate integration, final gates | All packets |

The program coordinator owns contract changes and generated files. Parallel
agents may not create temporary duplicate contracts to bypass a dependency.
