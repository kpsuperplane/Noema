# Harness Runtime Architecture

The runtime architecture defines how Noema turns a trigger into a governed run,
executes agent work, persists the run ledger, and recovers from interruption.

The central design principle is that every kind of agent work should pass
through the same runtime spine:

```text
trigger -> run envelope -> context packet -> governed execution
        -> capability/memory/output mediation -> event ledger
```

This keeps user conversations, task automation, scheduled work, proactive
suggestions, imports, and agent handoffs compatible with one another.

## Current Rust daemon slice

The current Rust implementation is intentionally much smaller than the full
runtime described below. It introduces the process boundary that later runtime
features can grow into:

- `noema start` runs a foreground, user-level daemon.
- The daemon listens on `$NOEMA_HOME/run/noema.sock`, defaulting to
  `~/.noema/run/noema.sock`, using newline-delimited JSON.
- `noema config` initializes the Noema directory and writes a default
  Codex-oriented `config.yaml` when one is not already present.
- `noema start` runs the same initialization path when the Noema directory or
  default config is missing.
- The daemon owns provider runtime state instead of recreating it for every CLI
  message.
- The daemon starts `codex app-server --listen stdio://` lazily on the first
  chat conversation.
- `noema chat` connects to the daemon, creates a fresh Noema conversation, and
  the daemon maps that conversation to one Codex thread.
- If `noema chat` cannot reach a daemon, it starts a temporary daemon for that
  chat session and shuts it down when chat exits.

This slice does not yet create durable conversation rows, run envelopes, event
ledger entries, approvals, tools, or durable capability state. Conversation IDs
and Codex thread mappings are in-memory daemon state only. Memory records,
chat-turn provenance, and Postgres-backed context graph inspection are covered by
the memory runtime docs and the frontend current contract.

The daemon intentionally uses the stable Codex app-server flow: initialize the
connection, start a thread with the chat client's current working directory,
send text-only `turn/start` requests, collect assistant output from streamed
notifications, and wait for `turn/completed`. It does not send experimental
Codex permission fields; Codex's own config continues to own sandbox and
permission policy.

## Runtime components

### Trigger router

The trigger router accepts incoming requests from human interfaces, schedules,
tasks, external events, imports, tools, and internal system jobs.

It should normalize each trigger into a structured request with:

- Trigger type.
- Trigger source.
- Requesting principal.
- Initial payload.
- Initial object links.
- Desired agent, if specified.
- Desired home scope, if specified.
- Idempotency key, if available.
- Timestamp.
- Trust labels.

The trigger router should not execute agent logic. It should create or resume a
run through the run coordinator.

### Run coordinator

The run coordinator creates run envelopes, writes initial ledger events, and
places work on the local worker queue.

It should:

- Select the executing agent.
- Resolve the home scope.
- Resolve active context scopes.
- Check whether the trigger is permitted to create a run.
- Create the durable run record.
- Write `run.started`.
- Add queue metadata.
- Link parent and child runs.
- Deduplicate retries using idempotency keys where available.

The run coordinator is responsible for making run creation deterministic and
auditable.

### Local worker kernel

The worker kernel executes leased runs.

It should:

- Claim queued runs with a lease.
- Load the current run projection.
- Read the run ledger.
- Resume at the next safe step.
- Execute bounded work.
- Persist every meaningful transition.
- Release, renew, or fail the lease.
- Respect cancellation, pause, and approval waits.

The worker should not be the only place where important state exists. If the
process dies, Noema should be able to reconstruct the run from the ledger and
projection tables.

### Context assembler

The context assembler builds a context packet from the run envelope.

It should gather:

- Trigger payload.
- Conversation state.
- Task state.
- Project state.
- Workspace state.
- Relevant files and artifacts.
- Retrieved memories.
- Agent instructions and skills.
- Capability summaries.
- Policy constraints.
- Prior approvals or pending approval state.

The context assembler should produce a manifest that explains every included
piece of context. The manifest is critical for debugging and trust.

### Agent executor

The agent executor runs the selected agent against the context packet.

It may call:

- Model providers.
- Agent planners.
- Tool-selection logic.
- Response generators.
- Memory proposal extractors.
- Handoff selectors.

The agent executor emits proposals. The harness turns allowed proposals into
effects.

### Capability gateway

The capability gateway mediates all tool and integration calls.

It should:

- Validate operation schemas.
- Resolve resource selectors.
- Classify side effects and egress.
- Ask the governance runtime for a policy decision.
- Create approval requests when needed.
- Invoke adapters only after approval and policy checks.
- Normalize outputs.
- Label outputs with trust and sensitivity.
- Store artifacts.
- Write invocation events.

### Governance runtime

The governance runtime evaluates policies.

It should be consulted before:

- Creating runs from non-human triggers.
- Retrieving memory.
- Showing memory to agents.
- Invoking capabilities.
- Writing files.
- Sending messages.
- Mutating tasks.
- Creating or updating memory proposals.
- Producing external effects.
- Performing proactive actions.

The governance runtime may depend on memory or policy documents as input, but
the final enforcement result should be structured and auditable.

### Approval service

The approval service stores, displays, expires, and resolves approval gates.

It should support:

- Approval requests attached to runs.
- Approval requests attached to tasks.
- Operation-specific approvals.
- Scoped approvals.
- One-time approvals.
- Time-limited approvals.
- Approval denial.
- Approval revocation.
- Approval comments.
- Resume signals for waiting runs.

Approvals must be durable. A run should be able to pause, wait for approval,
and resume later.

### Event writer

The event writer appends run events.

It should enforce:

- Monotonic per-run sequence numbers.
- Valid event schemas.
- Actor attribution.
- Scope attribution.
- Payload validation.
- Redaction rules.
- Optional integrity hashes.

No meaningful runtime transition should happen without an event.

### Projection builder

The projection builder maintains query-friendly current state derived from the
ledger.

Projections may include:

- Current run status.
- Current approval status.
- Current worker lease.
- Last error.
- Latest output.
- Tool call summaries.
- Memory-use summaries.
- Task activity summaries.
- Dashboard timelines.

Projections improve performance and UX. They are not a substitute for the
ledger.

## RunEnvelope

A `RunEnvelope` is the normalized execution contract for a run.

Conceptual shape:

```json
{
  "run_id": "run_01...",
  "trigger": {
    "type": "human_message",
    "source": { "object_type": "conversation", "object_id": "conversation_1" },
    "source_item": {
      "object_type": "conversation_item",
      "object_id": "item_01..."
    },
    "payload_ref": {
      "object_type": "conversation_turn",
      "object_id": "turn_01..."
    },
    "trust": "human_authored"
  },
  "requested_by": { "object_type": "human", "object_id": "human:kevin" },
  "executing_agent": { "object_type": "agent", "object_id": "agent:primary" },
  "owner": { "object_type": "human", "object_id": "human:kevin" },
  "home_object": { "object_type": "conversation", "object_id": "conversation_1" },
  "active_object_refs": [
    {"object_type": "human", "object_id": "human:kevin"},
    {"object_type": "agent", "object_id": "agent:primary"},
    {"object_type": "conversation", "object_id": "conversation_1"},
    {"object_type": "workspace", "object_id": "workspace:noema"},
    {"object_type": "project", "object_id": "project:harness"}
  ],
  "links": {
    "conversation_id": "conversation_01...",
    "project_id": "project_harness",
    "task_id": null,
    "parent_run_id": null
  },
  "proactivity": {
    "trigger_level": 1,
    "max_allowed_level": 2
  },
  "egress": {
    "allowed_destinations": ["conversation:conversation_01..."],
    "requires_approval_for_external": true
  },
  "created_at": "2026-06-23T20:00:00Z"
}
```

The exact wire format can evolve. The required semantics should not.

The envelope should answer:

- Why does this run exist?
- Which agent is acting?
- On whose behalf?
- Inside which scopes?
- With which object links?
- Under which proactivity level?
- Under which egress constraints?
- With which parent or child relationships?

## Trigger types

Initial trigger types should include:

| Trigger | Meaning |
| --- | --- |
| `human_message` | A human sent a message in a conversation |
| `human_command` | A human invoked a command palette or dashboard action |
| `task_event` | A task entered a state that requires agent work |
| `schedule` | A cron or recurring schedule fired |
| `proactive_rule` | A proactivity policy matched context |
| `external_event` | A connector, webhook, import, or watched source changed |
| `agent_handoff` | Another agent delegated work |
| `tool_callback` | A long-running tool or integration produced a callback |
| `system_recovery` | The system resumed or repaired interrupted work |
| `system_maintenance` | Noema initiated internal maintenance |

All triggers should produce a run envelope or an explicit denial event.

## Run statuses

Run status should be a projection from events.

Recommended statuses:

| Status | Meaning |
| --- | --- |
| `queued` | Run exists and is waiting for a worker |
| `assembling_context` | Context packet is being built |
| `executing` | Agent/model loop is active |
| `waiting_on_tool` | A tool invocation is in progress |
| `waiting_on_approval` | Run is paused on a durable approval gate |
| `waiting_on_human` | Run needs human clarification or input |
| `suspended` | Run is intentionally paused without failure |
| `retrying` | Run is recovering from a retryable failure |
| `completed` | Run finished successfully |
| `failed` | Run ended in a non-retryable failure |
| `cancelled` | Run was cancelled by an authorized principal |

Statuses should be simple enough for dashboard UX. The ledger should contain
the richer detail.

## Lifecycle

### 1. Accept trigger

The trigger router receives a trigger and validates its shape.

It should:

- Authenticate the source where applicable.
- Assign trust labels.
- Store or reference raw payloads.
- Derive an idempotency key if possible.
- Reject malformed triggers with a durable denial event where appropriate.

### 2. Create run envelope

The run coordinator resolves the agent, owner object, home governable context,
and active governable contexts.

It should:

- Prefer explicit user-chosen context.
- Fall back to conversation, task, project, workspace, or human defaults.
- Avoid silently broadening scope.
- Record when inferred scope selection occurs.
- Ask for clarification if multiple high-impact scopes are plausible.

### 3. Persist run start

The coordinator creates the durable run and writes `run.started`.

At this point, Noema should have enough information to show the run in a
dashboard even if no worker has executed it yet.

### 4. Lease work

A local worker claims the run.

The lease should include:

- Worker ID.
- Lease start.
- Lease expiration.
- Heartbeat time.
- Attempt number.

If the worker stops heartbeating, the run becomes eligible for recovery.

### 5. Assemble context

The context assembler builds a context packet and context manifest.

It should:

- Resolve deterministic state first.
- Request memory through the memory runtime.
- Retrieve files or artifacts through governed references.
- Summarize or compact context when needed.
- Label trust boundaries.
- Record why context was included.

The harness writes context events before model execution.

### 6. Preflight policy

The governance runtime evaluates whether the run may proceed.

Preflight may deny or require approval before any model call if the trigger is
too proactive, the agent is not allowed in scope, the requested action is
sensitive, or required context grants are missing.

### 7. Execute agent loop

The agent executor invokes the model/planner.

The loop may produce:

- A final reply.
- A tool invocation proposal.
- A request for approval.
- A request for human clarification.
- A memory proposal.
- A task update proposal.
- A handoff proposal.
- A cancellation or suspension recommendation.

The model should not directly execute effects.

### 8. Mediate tool calls

Tool calls go through the capability gateway.

If a tool call requires approval, the run pauses. The approval request should
include enough detail for a human to make the decision without trusting the
model's summary alone.

### 9. Review output and egress

Before a reply, notification, file write, external API mutation, email, share,
or publish action leaves the run boundary, the governance runtime should
classify and check egress.

For ordinary chat replies to the initiating human, the policy may be simple.
For any external or cross-scope destination, the policy should be explicit.

### 10. Record memory use and proposals

The harness records:

- Which memories were retrieved.
- Which memories were shown to the agent.
- Which memories influenced output or action.
- Which new memory proposals were generated.

The memory subsystem owns promotion and lifecycle.

### 11. Complete or pause

The run ends in one of several durable outcomes:

- Completed with output.
- Waiting on approval.
- Waiting on human input.
- Suspended.
- Cancelled.
- Failed with retryable error.
- Failed with terminal error.

The ledger should make the outcome understandable.

## Worker kernel details

The initial slice should use a local worker kernel, probably inside the same app or service
process as the scheduler. The worker boundary should still be explicit.

### Worker responsibilities

Workers should:

- Poll or receive queued run work.
- Claim leases transactionally.
- Execute runs in bounded phases.
- Renew leases during long-running work.
- Persist events after each phase.
- Release leases on completion.
- Pause runs on approvals.
- Surface cancellation quickly.
- Avoid holding database write locks across slow model or tool calls.

### Worker non-responsibilities

Workers should not:

- Store canonical run state only in memory.
- Bypass policy for "internal" actions.
- Call tools directly.
- Write memory directly.
- Make durable approval decisions.
- Own trigger normalization.
- Own dashboard presentation.

### Crash recovery

On startup, recovery should:

- Find expired leases.
- Mark affected attempts as interrupted.
- Determine the last completed safe step from the ledger.
- Resume idempotent steps.
- Require review for non-idempotent or ambiguous external effects.

Adapters should provide idempotency keys where possible. If a tool cannot prove
whether an external write happened, the run should surface a recovery issue
instead of retrying blindly.

### Cancellation

Cancellation should be a policy-checked event.

Possible cancellation sources:

- Human owner.
- Workspace admin.
- Task owner.
- System timeout.
- Worker shutdown.
- Policy revocation.

Cancellation should try to stop future effects. It cannot always undo external
effects that already happened. The ledger should make that clear.

## Agent execution model

Agents in Noema should be actor-capable concrete objects with skills,
preferences, policies, and operational memory. They are not unconstrained
background processes.

An agent execution step should receive:

- A context packet.
- A tool/capability summary, not raw unrestricted function handles.
- Output constraints.
- Security labels.
- Approval state.
- A bounded budget.

An agent execution step may return:

- Natural language output.
- Structured tool proposals.
- Structured memory proposals.
- Structured task proposals.
- Structured handoff proposals.
- Structured approval rationale.

The harness should prefer structured proposals for anything that might become
an effect.

## Context packet model

A context packet is a point-in-time execution input.

It should include:

- `packet_id`
- `run_id`
- `created_at`
- `included_sources`
- `included_memories`
- `included_artifacts`
- `included_conversation_items`
- `included_tasks`
- `included_capabilities`
- `policy_summary`
- `trust_labels`
- `sensitivity_summary`
- `token_or_size_budget`
- `omitted_context_summary`

The packet manifest should be persisted or reconstructible from persisted
events. If a run produces an unexpected answer, the human should be able to ask
"what did the agent see?" and get a concrete answer.

## Handoffs and child runs

Agent handoffs should create child runs rather than silently switching agents
inside the same run.

A handoff proposal should state:

- Target agent.
- Reason for handoff.
- Requested governable context.
- Context to transfer.
- Capabilities needed.
- Expected output.
- Whether human approval is required.

The harness should:

- Check whether the source agent can delegate.
- Check whether the target agent can operate in the requested scopes.
- Create a child run with a parent link.
- Transfer context by reference where possible.
- Record handoff events.

This design keeps multi-agent work auditable.

## Proactive and scheduled runs

Proactive and scheduled runs should use the same run model.

Additional requirements:

- The trigger must include the proactivity rule or schedule that caused it.
- The run envelope must include the allowed proactivity level.
- Egress should default to suggestion or notification, not external action.
- External effects should require approval unless explicitly pre-authorized.
- Cooldowns and recurrence rules should be visible in the ledger.

A proactive run that merely prepares a suggestion can complete without
notifying the user if policy says not to surface it.

## Artifacts

Runs may produce artifacts:

- Draft files.
- Tool outputs.
- Reports.
- Patches.
- Images.
- Imported documents.
- Export bundles.
- Logs intended for inspection.

Durable artifact files should live under the appropriate object-owned folder,
not under `system/`.

The harness should register:

- Artifact ID.
- Owner scope.
- Producing run.
- Path or object reference.
- Content type.
- Sensitivity.
- Provenance.
- Retention policy.
- Whether the artifact was shown, exported, or shared.

## Timeouts and budgets

Runs should have explicit budgets.

Budget dimensions may include:

- Wall-clock time.
- Model tokens.
- Tool calls.
- Tool cost.
- External writes.
- File bytes read.
- File bytes written.
- Memory retrieval count.
- Child run count.

Budget exhaustion should become a structured event. The run can then complete
with partial output, pause for approval, or fail gracefully.

## Error model

Errors should be structured.

Recommended categories:

- `validation_error`
- `policy_denied`
- `approval_denied`
- `missing_context`
- `memory_unavailable`
- `capability_unavailable`
- `adapter_error`
- `model_error`
- `timeout`
- `lease_interrupted`
- `egress_blocked`
- `conflict_detected`
- `unknown_error`

Each error event should include:

- Whether it is retryable.
- Which component produced it.
- Whether any external effect may have occurred.
- Whether human action is needed.
- Suggested recovery path.

## Initial runtime slice

A practical first runtime slice:

- Conversation-triggered runs.
- One local worker.
- Postgres-backed run records and events.
- Basic context packet assembly.
- Memory retrieval request and memory-use recording.
- Capability registry with a small local/internal set.
- Durable manual approvals.
- Chat reply egress checks.
- Dashboard or CLI timeline inspection.

This slice should still use the same envelope, ledger, policy, and capability
boundaries that future task, schedule, proactive, and multi-agent runs will use.
