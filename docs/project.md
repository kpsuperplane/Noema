# Noema

An open-source personal agent operating system.

Noema is an always-on, self-hosted platform for multiple humans, agents, conversations, workspaces, projects, tools, tasks, and memory. It should feel polished and opinionated by default, while remaining inspectable and customizable as the human grows into the system.

## Values

- Opinionated in construction, impartial in use
- Privacy and security thoughtfully integrated throughout
- Simple initial setup; complexity grows with the human
- Self-hosted by default, with a personal server that can run continuously
- Transparent systems over mysterious AI state
- Human-owned data, context, and memory
- Deterministic controls wherever trust depends on them

## Core product goals

- Support multiple agents, defaulting to one primary agent
- Support multiple humans, defaulting to one primary human
- Treat conversations, workspaces, and projects as first-class coordination surfaces
- Provide a full-featured task system with Kanban, dependencies, delegation, approvals, and multi-agent orchestration
- Support tools and integrations through explicit permissions and audit trails
- Make memory automatic enough to feel effortless and inspectable enough to feel trustworthy
- Make proactivity customizable globally, by scope, by agent, and by project
- Expose reliable export, deletion, rebuild, restore, and audit operations

## Filesystem and storage architecture

Noema should keep a clean split between canonical structured state and durable
object-owned files. The default Noema directory is `~/.noema`; users can point
Noema at another directory with `NOEMA_HOME`.

```
~/.noema/
  config.yaml

  db/
    surrealdb/            # embedded canonical structured store

  humans/
    [human_id]/
      docs/
      imports/
      artifacts/

  agents/
    [agent_id]/
      docs/
      skills/
      runs/
        [run_id]/
          artifacts/

  conversations/
    [conversation_id]/
      attachments/
      artifacts/

  workspaces/
    [workspace_id]/
      docs/
      projects/
        [project_id]/
          docs/
          artifacts/

  system/                 # derived and rebuildable
    indexes/
    cache/
    tmp/
```

Source-of-truth rules:

Concrete object rows are the canonical structured state. Shared concepts such
as actor/principal, governable scope, provenance source, and transcript item
are interfaces implemented by concrete objects rather than universal parent
tables.

| Data | Source of truth |
| --- | --- |
| Structured state: humans, agents, tools, conversations, transcript items, graph claims, provenance, grants, tasks, permissions, retrieval packets, and audit events | Embedded SurrealDB |
| Human-authored docs, imported files, attachments, and durable artifacts | filesystem |
| Indexes, caches, temporary files, and derived search/vector state | `system/` |
| Introspection into database-backed state | chat/work drill-ins, advanced inspection, and explicit export tools |

## Primary objects

| Object | Purpose |
| --- | --- |
| Humans | People who own, use, collaborate through, or are represented inside Noema |
| Agents | Specialized assistants with skills, tools, policies, and operational memory |
| Conversations | Interaction history, working context, decisions, tool calls, and candidate memories |
| Workspaces | Shared environments for humans, agents, tools, policies, and projects |
| Projects | Goal-oriented spaces with decisions, tasks, documents, open loops, and agent activity |
| Tools | Capabilities agents can invoke under permission and audit rules |

## Core architecture

```
Human Interfaces
  chat, workspaces, command palette, filesystem view, desktop/mobile clients,
  advanced inspection
        │
        ▼
Agent Runtime
  agents, planners, schedulers, tool use, task workers, automations
        │
        ▼
Governance Runtime
  permissions, scopes, policies, approvals, audit, proactivity limits
        │
        ▼
Context Runtime
  conversations, projects, workspaces, humans, tools, tasks, memory retrieval
        │
        ▼
Canonical Store
  Embedded SurrealDB + durable object-owned files
        │
        └── System State
              indexes, caches, vectors, temp files
```

## First-class governable contexts

Governable scope is a behavior contract implemented by concrete objects. These
contexts govern visibility, permissions, memory, proactivity, tool access,
auditability, and default behavior without requiring a universal `scopes` root
table.

```
System
Human
Agent
Conversation
Workspace
Project
Task
Cron
Tool
Relationship
```

## Tasks and orchestration

Noema should include a first-party task system, not just chat reminders.

Task system requirements:

- Kanban views
- Dependencies and blocking relationships
- Human and agent assignees
- Approval checkpoints
- Recurring tasks
- Project-linked and conversation-linked tasks
- Tool-call audit trails
- Agent handoffs
- Task provenance

## Memory as a subsystem

Memory is critical, but it is one subsystem inside the broader agent OS.

Default ownership rules:

```
Human facts live with the human.
Project facts live with the project.
Task-local execution context and open loops live with the task.
Scheduled trigger state lives with the cron.
Workspace facts live with the workspace.
Conversation-local context lives with the conversation.
Agent skills live with the agent.
Interaction preferences live with the relationship.
Participants link memories across conversations without changing ownership.
Provenance links everything.
```

## Proactivity

Proactivity should be customizable and explainable.

```
0. Never use proactively
1. Use only when human asks
2. Use silently to personalize responses
3. Surface suggestions inside chat
4. Send proactive notifications
5. Propose external actions
6. Take approved automatic actions
```

## Frontend surfaces

These are target product surfaces. The initial frontend should start with chat as
the primary experience. Memory, settings, inspection, workspaces, projects,
tasks, tools, approvals, and audit should reveal incrementally from chat/work
events and become full management surfaces only when backed state and user
intent require them.

- Chat and conversation threads
- Inline memory, task, tool, approval, denial, and recovery events
- Memory settings and review
- Workspaces
- Projects
- Tasks
- Tools
- Permissions
- Humans and agents
- Proactivity
- Audit log
- Exports and restore
- Advanced owner/admin inspection

## Backup and portability

Noema should assume humans can back up the entire Noema directory, defaulting
to `~/.noema/`, with their preferred backup tool.

A complete backup includes:

```
config.yaml
db/
humans/
agents/
conversations/
workspaces/
```

`system/` is rebuildable. Exports are separate from backups and should support machine-readable and human-readable formats.

## Current slice recommendation

Build Noema around the root objects first:

```
humans
agents
conversations
workspaces/projects
tools
tasks
memory
```

Use embedded SurrealDB as the canonical structured store. Use the filesystem for durable object-owned documents and artifacts. Use chat/work drill-ins, advanced inspection, and explicit export tools for introspection into database-backed state.

[Memory System Plan](memory.md)

[Runtime Harness Architecture](harness.md)

[Retired Postgres Schema](postgres.md)
