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

## Information handling

Noema distinguishes secrets, private information, and ordinary information.
Secrets stay outside model context and ordinary persistence. Private information
is preserved and controlled through scope-aware authorization plus egress
policy. Ordinary information is preserved without precautionary redaction.
Redaction never substitutes for authorization; the detailed authority is
[`docs/harness/security.md`](harness/security.md#information-classes-and-mechanisms).
Browser, native-client, recovery, public-ingress, and process-execution
boundaries are defined in [Server Authentication and Public Access](server-security.md).

## Core product goals

- Support multiple agents, defaulting to one primary agent
- Support multiple humans, defaulting to one primary human
- Treat conversations, workspaces, and projects as first-class coordination surfaces
- Provide a full-featured task system with an operational queue, dependencies, delegation, approvals, and multi-agent orchestration
- Support tools and integrations through explicit permissions and audit trails
- Make memory automatic enough to feel effortless and inspectable enough to feel trustworthy
- Make proactivity customizable globally, by scope, by agent, and by project
- Expose reliable export, deletion, rebuild, restore, and audit operations

## Filesystem and storage architecture

Noema keeps stored state separate from durable object-owned files. The default
Noema directory is `~/.noema`. `NOEMA_HOME` can select another directory.

```
~/.noema/
  config.yaml

  noema.sqlite3           # Noema-owned structured state

  run/
    capability-auth/      # protected exact arguments for active auth pauses
    browser-session.key   # protected browser cookie key
    native-oauth-retries.json # protected short-lived refresh responses
    graphql.sock          # process-local GraphQL socket when enabled

  notifications/
    apns-provider.json     # protected APNs authority, metadata, revision, and tombstone

  adapters/
    definitions/          # immutable source manifests by semantic SHA-256
    sources/              # optional exact imported descriptions by source SHA-256
    oauth-profiles/       # reviewed public OAuth protocol profiles
    oauth-applications/   # application descriptors and protected client secrets
    external-accounts/    # stable provider account descriptors
    oauth-grants/         # grant descriptors and protected token generations
    connections/          # descriptors and protected credential generations
    quarantine/           # invalid or intentionally removed adapter objects

  mcp/                    # MCP server configuration and protected credentials

  providers/              # hosted-provider account credentials

  memory/
    human/                # source local-human Markdown memory tree

  models/
    blobs/                # checksum-verified, content-addressed GGUF files
    downloads/            # resumable partial model transfers

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

  tasks/
    [task_id]/
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

Concrete object rows are the stored state. Shared concepts such
as actor/principal, governable scope, provenance source, and transcript item
are interfaces implemented by concrete objects rather than universal parent
tables.

| Data | Source of truth |
| --- | --- |
| Structured state: humans, human passkeys, browser sessions, agents, tools, conversations, transcript items, provider accounts, local-model installations, MCP setup, tasks, permissions, approvals, and audit events | SQLite |
| Active capability-authentication metadata and exact private replay arguments | SQLite metadata plus `${NOEMA_HOME}/run/capability-auth/` protected files; in-flight state is not database-rebuildable |
| APNs provider authority, metadata, revision, and removal tombstone | `${NOEMA_HOME}/notifications/apns-provider.json` protected file; the private key never enters SQLite |
| Client notification registrations, Tasks Live Activity projections, and durable delivery queues | SQLite; every native registration is bound to its authenticated OAuth client |
| Adapter definitions, source bytes, connections, OAuth profiles, applications, accounts, grants, and protected generations | `${NOEMA_HOME}/adapters/`; SQLite adapter tables are disposable public projections |
| Memory prose, semantic metadata, provenance, and consolidation state | `memory/human/` Markdown |
| Verified local model weights | `${NOEMA_HOME}/models/blobs/` |
| Human-authored docs, imported files, attachments, and durable artifacts | filesystem |
| Indexes, caches, temporary files, and derived search/vector state | `system/`, including rebuildable memory FTS |
| Introspection into database-backed state | chat/task drill-ins, advanced inspection, and explicit export tools |

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
Stored Data
  SQLite + source memory Markdown + durable object-owned files
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

- Operational queue with inline history
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

`PROJECT.md` is the project context authority. It lives in the working folder,
or under Noema's workspace files when the project has no folder.

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
tasks, tools, approvals, and audit should reveal incrementally from chat/task
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

Stop Noema before backup or restore. Copy the complete Noema directory with the
preferred backup tool. The directory defaults to `~/.noema/`.

Do not use a partial directory list as a complete backup. The home contains
SQLite state, memory, credentials, adapter authority, models, and artifacts.
Some `system/` data is rebuildable, but a complete backup includes it.

Restore the complete directory before starting Noema. Exports are separate
from backups. Exports should support machine-readable and human-readable forms.

## Current storage model

SQLite owns stored state. The native Markdown tree owns durable memory prose.
A separate SQLite FTS projection supports rebuildable memory search. The
filesystem owns durable documents, protected credentials, and artifacts.

Chat and task details provide focused inspection. Advanced inspection and
explicit export tools expose broader database-backed state.

[Memory Contract Index](memory.md)

[Tasks Contract](tasks.md)

[Runtime Harness Architecture](harness.md)
