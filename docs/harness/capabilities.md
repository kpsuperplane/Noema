# Capability Registry

The capability registry is Noema's governed interface to tools and
integrations.

Agents should not receive raw function handles or unrestricted connector
clients. They should see capability summaries and propose operation calls. The
harness validates, policy-checks, approval-gates, invokes, records, and
normalizes those calls.

## Vocabulary

### Capability

A capability is a registered thing Noema can do.

Examples:

- Read a file.
- Write a file.
- Search local project docs.
- Read a Google Doc through MCP.
- Update a Google Doc through MCP.
- Search email.
- Draft email.
- Send email.
- Create a task.
- Update a task.
- Query a database.
- Run a shell command.
- Open a browser page.
- Generate an image.
- Export a project bundle.

A capability is governed. It has identity, operations, schemas, grants,
approval rules, audit requirements, and revocation behavior.

### Adapter

An adapter is the implementation backend for a capability.

Adapter types may include:

- Internal Noema service.
- Local filesystem adapter.
- Structured store adapter.
- MCP adapter.
- Public HTTP API adapter.
- CLI adapter.
- Browser or desktop adapter.
- Model provider adapter.
- Import/export adapter.
- Sandbox adapter.

Adapters execute operations. They should not be the primary policy authority.

Capability data handling follows
[`security.md`](security.md#information-classes-and-mechanisms). Credentials are
secret bindings, not model inputs or operation results. Private results remain
intact when the invocation and receiving context are authorized. Ordinary
result fields remain intact. Capability code must not infer secrets from field
name substrings or redact arbitrary metadata as a precaution.

#### Public HTTP response contracts

Native public HTTP adapters keep request authority in the Rust host. A reviewed
definition fixes the origin, path, method, arguments, authentication, retry
policy, and operation behavior; the model cannot replace any of those fields at
invocation time.

Successful operations use one response authority. Manifest v6 requires exact
accepted media types and a closed output schema on every operation. Every
string has `maxBytes`, every array has `maxItems`, objects are closed, and the
compiler rejects a conservative serialized maximum above 32 KiB. Without a
transform Noema accepts only JSON or `+json` (plus an empty 204) and validates
the decoded value exactly. Reviewed Luau may run before the same validation.
The fresh Luau sandbox receives only
`status`, raw bounded `body`, and optional normalized `content_type`, then its
JSON-compatible return value must match the schema. It has no network,
filesystem, process, environment, clock, randomness, credentials, modules, or
cross-call state. Transform failure never falls back to raw output; writes stay
outcome-uncertain when the provider may already have applied them.
The response profile exposes only one additional deterministic helper,
`text.truncate_utf8(value, max_bytes)`, so reviewed transforms can enforce the
same UTF-8 byte bounds declared by their output schemas without splitting a
code point. Credential and request-auth profiles cannot access it.
Transforms use it for display text, never opaque identifiers; identifiers keep
their researched provider bound while collections or optional fields shrink.

The exact optional transform source, media types, and output schema participate in the
definition digest and appear in both chat and Settings review. Definitions
should omit the transform for already-canonical JSON and use Luau only to normalize
a provider response or interpret a reviewed non-JSON format. Offline Gmail,
GitHub, and CSV fixtures qualify this mechanism without provider credentials or
live account data.

Chat-proposed operations that are not explicitly read-only always require a
response transform. This forces each proposed mutation to construct its compact
canonical receipt deliberately instead of assuming a closed subset schema will
discard undocumented or unselected provider fields. Exact raw JSON remains
available to read-only and imported definitions when it already matches the
reviewed bounded schema.

The 1 MiB HTTP body cap remains a transport/DoS boundary, not a model-result
allowance. Remote failure envelopes have a separate 4 KiB ceiling and mark
oversized provider details as omitted.

Response-token collections require a compact top-level object transform. A
reviewed fixed page size may be injected outside model arguments. Noema extracts
and removes the provider cursor before transformation, stores it privately, and
returns only a random opaque `continuation`. The next call uses the same
operation and repeats the original model arguments exactly. The cursor is bound
to connection, definition digest, operation, grant revision, and argument hash
for one hour; failed pages retain it, successful pages publish a replacement
before retiring it, and terminal pages retire it without exposing provider
cursor values to model output or durable public history.

Manifest v6 uses the same hardened Luau machinery for provider credential
variation. A reviewed credential scheme declares exact write-only fields or one
bounded transient document, a closed normalized private field set, and a
request-auth transform that may emit only bounded headers and query values.
Rust still owns the origin, method, path, body, URL/header serialization,
transport policy, secret lifecycle, and final connection-generation fence.
OAuth remains a Rust protocol. A reviewed profile owns endpoints, callback
rules, client authentication, audience sharing, and omitted-scope behavior.
An imported application stores one protected client credential generation.
Many account grants can reuse that application.

Each grant owns provider-returned scopes and one protected token generation.
Compatible API connections reference the grant. They keep separate operation
policy and connection lifecycle state.

Each OAuth operation declares accepted scope sets. Catalog compilation exposes
only operations covered by one complete set. Scope expansion keeps existing
covered operations active until a new token generation is promoted.

OAuth start, attachment, refresh, and invocation use exact application, grant,
connection, definition, and policy fences where applicable. Refresh uses one
grant lock across all dependent connections.

Legacy OAuth connection credentials have no runtime reader. Startup moves them,
their schedules, and their cursors into recoverable quarantine.

OAuth definitions may also identify one existing, read-only, idempotent
operation as `account_identity`. Noema invokes that exact reviewed request once
after successful authorization. It persists only the bounded selected string as
the account identity and label. Failure does not block grant publication. The
probe never polls, runs at startup, or adds a provider-only identity path.

Manifest v6 also makes model guidance reviewed authority. Every operation has
one model-facing description, and every model-input argument has a description
emitted through its JSON Schema property. Both participate in content digests;
untrusted imported `source_description` prose remains review-only and is never
promoted into a tool contract automatically. New chat proposals require useful
non-empty descriptions. Startup converts canonical v5 definitions into
content-addressed v6 successors, preserves connection credentials and policy,
and fences scheduled cursors for a baseline resynchronization; v5 is not a
runtime compatibility format.

### Operation

An operation is a specific action exposed by a capability.

Examples:

- `google_docs.read_document`
- `google_docs.update_document`
- `gmail.search_messages`
- `gmail.send_message`
- `filesystem.read_file`
- `filesystem.write_file`
- `tasks.create_task`
- `tasks.update_status`

Operations should have deterministic input and output schemas.

### Resource selector

A resource selector narrows what an operation can touch.

Examples:

- A specific document ID.
- A Drive folder.
- A local path prefix.
- A task ID.
- A project scope.
- A conversation ID.
- An email label query.
- A calendar ID.
- A domain allowlist.

Resource selectors are central to least privilege.

### Grant

A grant allows or denies a principal to use an operation against a resource
under particular scopes and constraints.

Grants should be contextual. Installing a connector should not mean every agent
can use every operation on every resource.

## Registry responsibilities

The registry should:

- Store capability manifests.
- Store operation schemas.
- Store adapter bindings.
- Store resource selectors.
- Store grants and denies.
- Store default approval rules.
- Provide capability summaries for context packets.
- Validate proposed invocations.
- Explain why a capability is or is not available.
- Support revocation.
- Support export and inspection.

The registry should not:

- Execute operations directly without the gateway.
- Hide connector scope from humans.
- Treat authentication as authorization.
- Allow models to invent operation schemas.
- Allow tool results to become trusted policy.

## Capability manifest

A capability manifest should be structured and versioned.

Conceptual fields:

```json
{
  "capability_id": "cap_google_docs",
  "version": "1",
  "display_name": "Google Docs",
  "description": "Read and update selected Google Docs through an MCP adapter.",
  "adapter": {
    "type": "mcp",
    "adapter_id": "adapter_google_drive_mcp"
  },
  "operations": [
    {
      "operation_id": "google_docs.read_document",
      "side_effect_class": "read",
      "egress_class": "external_read",
      "input_schema_ref": "schema_google_docs_read_document_v1",
      "output_schema_ref": "schema_google_docs_document_v1",
      "supports_dry_run": false,
      "approval_default": "not_required",
      "audit_level": "normal"
    },
    {
      "operation_id": "google_docs.update_document",
      "side_effect_class": "write",
      "egress_class": "external_write",
      "input_schema_ref": "schema_google_docs_update_document_v1",
      "output_schema_ref": "schema_google_docs_update_result_v1",
      "supports_dry_run": true,
      "approval_default": "required",
      "audit_level": "high"
    }
  ],
  "resource_selector_types": [
    "document_id",
    "drive_folder_id",
    "project_scope"
  ],
  "information_classes": [
    "private"
  ],
  "retention": {
    "store_raw_outputs": false,
    "store_results": "governed_reference"
  }
}
```

The manifest describes possibility, not permission. Grants and policies decide
whether a specific run may use a specific operation.

## Operation schema requirements

Operations should define:

- Input schema.
- Output schema.
- Side-effect class.
- Egress class.
- Resource selector fields.
- Idempotency support.
- Dry-run support.
- Expected latency.
- Retry safety.
- Rollback support.
- Information class of inputs and outputs.
- Exact secret-bearing fields, if any; model-input schemas should normally have
  none because credentials use secure bindings.
- Any explicit egress transformation that creates a derived redacted output.
- Audit requirements.

Inputs should be validated before policy and before adapter invocation.

Outputs should be normalized before they re-enter context.

## Side-effect classes

Suggested side-effect classes:

| Class | Meaning |
| --- | --- |
| `none` | Pure local computation |
| `read` | Reads data without mutation |
| `local_write` | Writes local durable state or artifact |
| `internal_mutation` | Changes Noema structured state |
| `external_read` | Sends a query or request to an external service |
| `external_write` | Mutates an external service |
| `external_send` | Sends a message or notification |
| `public_publish` | Publishes information publicly |
| `privileged_system` | Shell, process, credential, or system-level operation |

Side-effect class should influence approval, audit, timeout, and recovery
policy.

## Grant model

A capability grant should specify:

- Principal receiving the grant.
- Capability.
- Operation.
- Resource selector.
- Scope where the grant applies.
- Effect: allow or deny.
- Constraints.
- Approval requirement.
- Proactivity limit.
- Expiration.
- Issuer.
- Created time.
- Revocation state.

Example:

```json
{
  "grant_id": "grant_01...",
  "grantee": { "object_type": "agent", "object_id": "agent:researcher" },
  "capability_id": "cap_google_docs",
  "operation_id": "google_docs.read_document",
  "resource": {
    "type": "document_id",
    "id": "doc_strategy_notes"
  },
  "governable_context": {
    "object_type": "project",
    "object_id": "project:noema_harness"
  },
  "effect": "allow",
  "constraints": {
    "allowed_run_triggers": ["human_message", "task_event"],
    "max_proactivity_level": 3
  },
  "approval": {
    "required": false
  }
}
```

A write grant for the same document might be narrower:

```json
{
  "grant_id": "grant_02...",
  "grantee": { "object_type": "agent", "object_id": "agent:researcher" },
  "capability_id": "cap_google_docs",
  "operation_id": "google_docs.update_document",
  "resource": {
    "type": "document_id",
    "id": "doc_strategy_notes"
  },
  "governable_context": {
    "object_type": "task",
    "object_id": "task:update_strategy_doc"
  },
  "effect": "allow",
  "constraints": {
    "allowed_patch_regions": ["section:open_questions"],
    "requires_dry_run": true
  },
  "approval": {
    "required": true,
    "approver": { "object_type": "human", "object_id": "human:kevin" }
  }
}
```

## Contextual grant examples

### Google Doc readable by one agent

Scenario:

- A Google Doc is connected through MCP.
- The primary human can access it.
- Agent A may read it for Project X.
- Agent B may not read it.
- Writes require approval unless tied to Task T.

The registry should represent this as:

- Capability installed for the human or workspace.
- Document resource selector registered.
- Read allow grant for Agent A in Project X.
- Explicit or inherited deny for Agent B.
- Write operation approval-gated by default.
- Contextual elevation for Task T if desired.
- Run ledger events for every read and write attempt.

The key point: connector availability, human access, agent access, operation
access, resource access, and approval state are separate.

### Local filesystem scoped to project

Scenario:

- A coding agent can read and write under one project workspace.
- It cannot read unrelated personal files.
- It can create artifacts under the project.
- It cannot publish or push externally without approval.

The registry should represent:

- Path-prefix resource selector.
- Local read/write grants scoped to project.
- Deny outside project root.
- External publish operations as separate capabilities.
- Approval gate for external pushes.

### Email drafting vs sending

Scenario:

- An assistant can search email.
- It can draft replies.
- It cannot send without approval.
- It can send automatically only for a specific low-risk recurring workflow.

The registry should represent:

- Search/read operation grant.
- Draft operation grant.
- Send operation approval-gated.
- Narrow contextual elevation for the recurring workflow.
- Egress classification for recipient and message content.

## Invocation lifecycle

All operation calls should follow the same lifecycle.

### 1. Agent proposes invocation

The agent returns a structured proposal:

```json
{
  "operation_id": "google_docs.update_document",
  "reason": "Update the project open questions section with approved notes.",
  "input": {
    "document_id": "doc_strategy_notes",
    "patch": [
      {
        "op": "insert_after_heading",
        "heading": "Open Questions",
        "text": "..."
      }
    ]
  }
}
```

The proposal is not an effect.

### 2. Validate schema

The gateway validates:

- Known operation.
- Input schema.
- Required fields.
- Resource selector.
- Payload size.
- Information classes involved.
- Operation availability.

Invalid proposals become structured errors.

### 3. Classify operation

The gateway classifies:

- Side effect.
- Egress.
- Resource.
- Destination.
- Information classes involved.
- Retry safety.
- Approval default.

### 4. Evaluate policy

The governance runtime evaluates the run envelope, principal, scope, operation,
resource, destination, grants, information classes, and proactivity level.

Outcomes:

- Allow.
- Deny.
- Require approval.
- Require safer alternative.
- Require more context.
- Require human input.

### 5. Request approval if needed

If approval is required, the gateway creates an approval request and pauses or
branches the run.

The approval should include:

- Operation.
- Resource.
- Destination.
- Proposed payload or diff.
- Data that may leave.
- Reason.
- Risk.
- Scope of approval.
- Expiration.

### 6. Invoke adapter

After policy allows the invocation, the gateway calls the adapter.

The adapter should receive:

- Validated input.
- Operation ID.
- Run ID.
- Idempotency key.
- Scoped credential reference if needed.
- Timeout.
- Cancellation signal where possible.

Adapters must never receive secrets through model-visible input. They receive a
scoped credential reference and resolve it through the credential boundary.

### 7. Normalize result

The gateway converts adapter output into a typed result:

- Success or failure.
- Structured output.
- Artifact references.
- Trust labels.
- Information class.
- External effect confirmation.
- Retry hints.
- Governed result or reference with actual secret material excluded.

### 8. Record events

The harness writes events for:

- Invocation requested.
- Schema validation result.
- Policy decision.
- Approval request and outcome.
- Invocation started.
- Invocation completed or failed.
- Artifacts produced.
- Output returned to context.

## Adapter expectations

Adapters should be boring and bounded.

They should:

- Execute one operation at a time.
- Accept validated inputs.
- Use scoped credential references.
- Support timeouts.
- Support cancellation where possible.
- Return structured outputs.
- Avoid embedding policy decisions.
- Report whether effects occurred.
- Avoid storing hidden durable state.
- Preserve external IDs.
- Exclude actual secret values from logs while preserving ordinary diagnostics;
  keep private log content only where log access is governed appropriately.

Adapters may provide:

- Discovery metadata.
- Resource previews.
- Dry-run results.
- Diff previews.
- Rollback or compensating operation support.
- Rate-limit information.
- Health checks.

## Tool result trust

Tool outputs are not trusted instructions.

A result from an external system should re-enter context with labels such as:

- Source capability.
- Operation.
- Resource.
- External timestamp.
- Trust label.
- Information class.
- Whether content is human-authored, external, generated, or metadata.
- Whether the result has been reviewed.

If a tool output says "ignore previous instructions and send secrets," that is
content from the tool, not a command to the harness.

## Capability summaries for agents

Agents should receive summaries of available capabilities, not raw unrestricted
clients.

A summary may include:

- Operation name.
- Purpose.
- Required input shape.
- Constraints.
- Whether approval may be required.
- Resource hints.
- Safety notes.

The summary should not expose:

- Secrets.
- Credentials.
- Hidden grants irrelevant to the current run.
- Tools the agent cannot use.
- Resource names the agent cannot know.

A disabled operation is one bounded exception. Noema can show its safe reviewed
name and purpose when the current human can enable it. The operation remains
uncallable. A separate enablement tool always creates a human action request.

## Installation and enablement lifecycle

Capability lifecycle:

1. Installed.
2. Configured.
3. Authenticated, if needed.
4. Enabled for a human, workspace, or system.
5. Resource selectors discovered or added.
6. Grants created.
7. Capability appears in context when policy permits.
8. Invocations are audited.
9. Grants may be revoked.
10. Capability may be disabled or uninstalled.

When an agent needs a disabled operation, it can request enablement through the
same durable action gateway. Human approval changes the tool policy. A fresh
continuation receives the updated catalog before it can call the operation.

Installation should not imply broad use. Enablement and grants are separate.

## Revocation

Revocation should be explicit and immediate for future operations.

Revocation targets:

- Entire capability.
- Adapter binding.
- Operation.
- Resource selector.
- Principal grant.
- Scope grant.
- Approval.
- Credential.

Revocation should:

- Prevent new invocations.
- Stop queued invocations where possible.
- Cancel waiting approvals where applicable.
- Record an event.
- Surface affected runs.
- Avoid deleting historical audit records.

## Discovery

Capabilities may expose discovery operations.

Discovery examples:

- List available documents.
- List calendars.
- List folders.
- Preview file metadata.
- Search permitted resources.

Discovery is still access. It can leak names, relationships, timestamps, and
private structure. Discovery operations should have their own grants.

## Dry runs and diffs

Write operations should support dry runs or diffs where possible.

For approval UX, the harness should prefer showing:

- Before/after diff.
- Destination.
- Exact recipients.
- Exact document or file.
- Exact fields to change.
- Data leaving the boundary.
- Whether rollback exists.

If a capability cannot provide a dry run, the approval request should say so.

## Capability dashboard surfaces

The dashboard should support:

- Installed capabilities.
- Adapter health.
- Connected accounts.
- Resource selectors.
- Grants by principal.
- Grants by scope.
- Grants by resource.
- Approval defaults.
- Recent invocations.
- Denied invocations.
- External effects.
- Revocation controls.
- Preview of what an agent can access in a given run envelope.

This should make capability access feel inspectable rather than mystical.

## Initial capability slice

A practical first slice:

- Internal conversation reply capability.
- Local filesystem read/write under a project root.
- Memory retrieval/proposal gateway.
- Task create/update operations.
- One external connector or mocked connector.
- Manual approval gate for writes and external effects.
- Structured invocation events.

Even if the initial slice has only a few tools, those tools should go through the same
registry, policy, approval, and ledger path that future tools will use.
