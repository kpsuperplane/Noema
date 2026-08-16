# Harness Security Model

Noema's runtime harness should assume that agents are useful but not trusted
security authorities. Models can reason about risk, summarize permissions, and
suggest safer alternatives, but the harness must enforce the actual boundary
between context and effect.

The security model is built around one central idea:

```text
Label and contain untrusted content at ingress.
Enforce permission and data-flow rules at egress.
```

Ingress protection matters, but egress protection is the decisive control.
Noema should assume that some untrusted, misleading, or malicious content will
reach a model. The harness must prevent that content from causing unauthorized
disclosure, mutation, notification, publication, or delegation.

## Security goals

The harness should protect against:

- Cross-scope data leakage.
- Unexpected outbound sharing.
- Prompt injection from external content.
- Confused-deputy tool use.
- Overbroad connector access.
- Silent privilege escalation.
- Accidental external writes.
- Unauthorized memory use.
- Unauthorized memory creation.
- Proactive actions beyond the user's comfort level.
- Connector output being treated as trusted instruction.
- Agent-to-agent delegation that bypasses policy.
- Tool results contaminating future context without provenance.
- Missing audit trails for high-impact actions.

The harness cannot make model reasoning perfectly safe. It can make effects
governed, inspectable, reversible where possible, and attributable.

## Trust boundaries

Noema should treat the following boundaries as explicit:

| Boundary | Risk |
| --- | --- |
| Human interface -> trigger router | Spoofed or malformed requests |
| External connector -> Noema | Prompt injection, malicious content, stale data |
| Filesystem import -> context | Embedded instructions, private or secret data, provenance loss |
| Memory runtime -> model context | Over-broad retrieval, stale or contested memory |
| Model output -> harness proposal | Hallucinated permissions, unsafe actions |
| Harness -> capability adapter | Unauthorized read/write or confused deputy |
| Capability result -> context | Tool output treated as trusted instruction |
| Harness -> external destination | Data exfiltration, irreversible side effects |
| Agent -> agent handoff | Privilege laundering through another agent |
| Derived index -> retrieval | Stale or incorrectly scoped data |

Every boundary should have structured metadata and policy checks where trust
depends on it.

## Principals, scopes, and resources

The security model uses three core questions:

```text
Who is acting?
Inside which scopes?
Against which resource or destination?
```

### Principals

Principals include:

- Humans.
- Agents.
- Groups.
- Tools.
- Services.
- Importers.
- System jobs.

Agents should be principals, but they should not be owners of human truth.
They act under grants, delegations, and run envelopes.

### Scopes

Scopes define boundaries for visibility, retrieval, proactivity, permissions,
tool access, and auditability.

Important scope types:

- System.
- Human.
- Workspace.
- Project.
- Conversation.
- Agent.
- Relationship.
- Tool.
- Custom.

A run can have multiple active scopes. The harness should avoid silently
expanding active scopes. If a run needs additional scope, it should request
policy elevation or human clarification.

### Resources

Resources are objects capabilities can access or mutate:

- Files.
- Directories.
- Documents.
- Emails.
- Calendar events.
- Contacts.
- Tasks.
- Conversations.
- Memories.
- Databases.
- External APIs.
- Notifications.
- Published links.
- Agent-owned artifacts.

Resource access should be grantable at the narrowest useful level. A connector
being installed should not imply every agent can use every resource exposed by
that connector.

## Ingress model

Ingress is anything that enters Noema:

- Human messages.
- Uploaded files.
- Imported documents.
- Connector results.
- Emails.
- Web pages.
- Calendar events.
- Tool outputs.
- API payloads.
- Model outputs from previous runs.
- Agent-generated summaries.

Ingress handling should:

- Parse through typed adapters where possible.
- Preserve original source references.
- Assign trust labels.
- Assign the ordinary/private information class where applicable; route any
  detected secret through secret exclusion.
- Record source, timestamp, and owner.
- Detect high-risk prompt-injection patterns.
- Strip or isolate active content where appropriate.
- Avoid turning external text into system instructions.
- Store raw content separately from derived summaries when retention allows.

Ingress screening should improve safety, but it should not be trusted as the
only defense.

## Trust labels

Noema should attach trust labels to content in model context.

Suggested labels:

| Label | Meaning |
| --- | --- |
| `system_policy` | Noema-authored policy or deterministic runtime instruction |
| `human_authored` | Direct statement by an authenticated human |
| `trusted_project_doc` | Document explicitly trusted in a scope |
| `internal_state` | Noema-owned structured state |
| `connector_metadata` | Metadata from a connector adapter |
| `external_content` | Content from outside Noema |
| `tool_output` | Output from a capability invocation |
| `model_generated` | Text generated by a model |
| `imported_unreviewed` | Imported content not yet reviewed |
| `unknown` | Trust cannot be established |

Trust labels should be visible to policy and, when useful, to the model as
context annotations.

The model may read untrusted content as data. It should not treat untrusted
content as instructions that override Noema, the human, the active task, or the
harness.

## Egress model

Egress is anything leaving the current run boundary.

Egress includes:

- A chat reply.
- A notification.
- An email.
- A message in another system.
- A file write.
- A shared document update.
- A published artifact.
- A task mutation visible to others.
- A memory write that changes future behavior.
- A tool call that sends data to an external service.
- A connector write.
- A handoff to another agent with context.
- A search query containing private context.
- A log entry exported outside the local system.

Egress checks should answer:

```text
What information is leaving?
Where is it going?
Who will be able to see it?
Which scopes does it come from?
Which policy allows it?
Does it require approval?
Was approval granted?
Must the egress be blocked, sent intact, or intentionally transformed?
What must be recorded?
```

The harness should treat egress as the key control point because egress is
where private context becomes externally visible or produces side effects.

## Egress classes

Suggested egress classes:

| Class | Examples | Default posture |
| --- | --- | --- |
| `internal_context` | Passing context within same run | Allow if scoped |
| `human_reply` | Reply to initiating human | Allow if no cross-scope leak |
| `local_artifact` | Draft file under owned project | Allow if scoped |
| `memory_proposal` | Candidate memory write | Allow if provenance exists |
| `task_mutation` | Update task status or notes | Policy-check |
| `agent_handoff` | Send context to another agent | Policy-check |
| `external_read` | Query external API with context | Policy-check the destination and data; preserve allowed content |
| `external_write` | Update doc, send email, create ticket | Use current connection and review policy |
| `external_share` | Share file/link with another person | Use current connection and review policy |
| `public_publish` | Publish web page, repo, package, post | Use current connection and review policy |
| `secret_exposure` | Send secret or credential anywhere | Deny by default |

Policy may refine defaults by human, workspace, project, task, agent,
relationship, tool, destination, and operation.

An external write does not categorically require a human decision. Current
connection policy selects immediate execution, human review, or LLM review. A
successful LLM review can clear execution under the deterministic matrix in
[the action request contract](action-governance.md#llm-review).

## Information classes and mechanisms

Noema uses exactly three information classes. These classes describe the
information, not whether a particular principal may access it:

| Class | Examples | Persistence | Model access | Egress |
| --- | --- | --- | --- | --- |
| `secret` | Passwords, API keys, access/refresh tokens, private keys, session cookies, authorization codes, PKCE verifiers, recovery codes, or any bearer value whose possession grants authority | Only explicit credential stores or protected transient-auth stores | Never; adapters receive secure bindings or references | Never as content |
| `private` | Personal, medical, financial, legal, relationship, workspace, project, conversation, or business-confidential content | Preserve in its governed source | Include intact only when scope, purpose, participant policy, and grants authorize the run | Apply normal egress policy for the exact destination and audience |
| `ordinary` | Non-secret content and metadata, including IDs, statuses, counts, schemas, non-credential URLs, paths, hostnames, ports, model names, service endpoints, and safe diagnostics | Preserve normally | Include when functionally relevant | Apply the operation's normal egress policy without precautionary redaction |

Trust labels, provenance, retention, memory status, action-triggering behavior,
and side-effect risk are orthogonal attributes. They must not silently promote
ordinary or private information into the secret class. An opaque ID is secret
only when possession of that ID itself grants authority.

The enforcement mechanisms are also distinct:

- **Secret exclusion** keeps secret material out of every sink except an
  explicit credential store or protected transient-auth store. Use typed
  secret wrappers, exact schema annotations, credential-store
  provenance, and secure bindings. If a result accidentally contains a known
  secret, remove only that value or fail the result when a safe source value
  cannot be produced.
- **Authorization** decides whether private information may be retrieved or
  shown in a context. Authorized private information remains intact.
  Unauthorized information is omitted or denied at the boundary; the stored
  source is not replaced with a redacted copy.
- **Egress policy** decides whether authorized private or ordinary information
  may cross to the exact destination and audience. It may block the operation,
  require approval, send the content intact, or intentionally create a
  redacted derivative. The source remains unchanged.
- **Redaction** is therefore a narrow transformation, not a general privacy
  posture. Never redact solely because a value is a path, URL, identifier,
  technical detail, high-entropy string, or has a field name containing words
  such as `authorization`, `secret`, `token`, or `cookie`.

Classification must come from the authoritative type, schema, credential
source, or explicit policy metadata. English-name substring matching and
entropy heuristics are not classification authorities. Defense-in-depth secret
scanners may block a forbidden sink, but they must not silently rewrite
saved tool results, memories, or model context based on a guess.

At protocol boundaries, a small exact sanitizer is an allowed
defense-in-depth measure. It may remove URL userinfo, exact standard credential
headers and OAuth or signed-URL parameters, and exact header or query names
declared by a reviewed connection definition. Matching is case-insensitive but
never partial: for example, `authorization` is removed while
`oauth_authorization_supported` is preserved. The same sanitized result must be
used for model output and ordinary persistence.

This sanitizer is deliberately not a general secret detector. It cannot prove
that arbitrary third-party prose or a neutral, undeclared field does not
contain a credential. Noema-managed credentials still rely on credential-store
provenance and secure injection below the model boundary for their hard
guarantee. An integration that needs a stronger guarantee for returned data
must supply a reviewed output schema or another exact secret location; adding
substring or entropy guesses is not an acceptable substitute.

## Policy composition

Noema should use least privilege plus contextual elevation.

Default posture:

- A connected tool is not globally available to every agent.
- A readable resource is not automatically writable.
- A project grant does not automatically grant another project.
- A conversation grant does not automatically grant proactive external action.
- A prior approval does not automatically authorize a different destination.
- A model's claim that something is allowed does not make it allowed.

Policy should compose from:

- System defaults.
- Human preferences.
- Workspace policy.
- Project policy.
- Task policy.
- Conversation policy.
- Agent grants.
- Relationship rules.
- Tool/capability rules.
- Resource-specific grants.
- Approval state.
- Proactivity level.
- Information class and trust labels.

Recommended precedence:

```text
explicit deny
> legal/security hard rule
> active revocation
> scope-specific approval or grant
> task/project contextual elevation
> agent grant
> workspace/human default
> system default
```

Precedence should be deterministic and explainable. When a decision is denied
or approval-gated, the dashboard should be able to show why.

## Contextual elevation

Contextual elevation lets Noema grant narrow extra authority in a particular
run context.

Examples:

- An agent can normally read a Google Doc but can write it only when acting on
  a specific project task.
- A research agent can use web search for a project but cannot send emails.
- A personal assistant can create calendar holds. An attendee invitation uses
  the connection's current execution and review policy.
- A coding agent can write inside a project workspace. A remote push uses the
  connection's current execution and review policy.
- A proactive rule can draft a message but cannot send it.

Contextual elevation should always include:

- Scope.
- Principal.
- Capability.
- Operation.
- Resource selector.
- Destination, if any.
- Duration or expiration.
- Approval requirement.
- Audit requirement.
- Revocation path.

## Durable approval gates

Action requests and human decisions are durable. They do not depend on an open
chat request, worker claim, or client connection.

A request that needs human review includes:

- The action ID and exact revision.
- The owner and requesting agent.
- The originating conversation or Task.
- The capability, operation, resource, and destination.
- The saved exact arguments or diff.
- Information leaving Noema and its expected audience.
- The reviewer classification and policy reason.
- A human-readable summary and bounded details.

Production supports `Approve once` and `Decline`. Approval permits one execution
after live revalidation. Execution admission consumes it. A stale request,
changed policy, or invalid origin supersedes the decision before invocation.

Changing the proposal requires a new action revision and review.

## Prompt injection model

Prompt injection should be treated as an expected property of the environment,
not as an exceptional failure.

External content may say:

- Ignore prior instructions.
- Reveal secrets.
- Call a tool.
- Send data somewhere.
- Store a false memory.
- Trust a malicious link.
- Change project policy.
- Impersonate the user.

The harness should defend by:

- Labeling external content.
- Separating content from instructions.
- Not granting tools directly to the model.
- Requiring structured tool proposals.
- Checking proposals through policy.
- Preventing untrusted content from expanding scopes.
- Blocking unexpected egress.
- Recording source provenance.
- Making high-risk tool outputs reviewable.

Prompt text alone is not a security boundary. The harness boundary is.

## Confused deputy protection

A confused deputy attack occurs when an agent or tool uses its authority on
behalf of untrusted content.

Example:

```text
An imported document instructs the agent to email private project notes
to an external address. The agent has email access. Without egress controls,
the document can exploit the agent as a deputy.
```

The harness should prevent this by requiring:

- A clear requesting principal.
- An active run scope.
- A capability operation.
- A resource and destination.
- A policy decision.
- An approval if required.
- A ledger event.

The source of the request matters. A tool output or imported document cannot
become the requester for an external action.

## Secret-backed capabilities

Secret values never enter model context, prompts, conversation history, run
events, ordinary diagnostics, artifacts, or exports. A secret-backed operation
is authorized using its non-secret capability, account, scope, and policy
metadata; the adapter then resolves the credential through a secure binding.
Logs and debugging surfaces may record the credential kind, owning connection,
generation, or presence state, but never the secret value. Secret access is
auditable through those references.

## Memory safety

Memory has special security implications because it shapes future runs.

Current memory belongs to the local human. It does not implement private memory
scopes or memory-specific access grants.

- Human messages and exact saved tool results can provide memory evidence.
- Assistant messages and external prose cannot provide evidence independently.
- Every published claim retains exact evidence identifiers.
- Secret material cannot enter memory. A non-secret capability reference may.
- External content cannot directly create confirmed memory.
- Memory content remains untrusted for creating action authority.

## Agent handoff safety

Agent handoffs are egress.

When one agent hands context to another, Noema should check:

- Whether the source agent may delegate.
- Whether the target agent may operate in the requested scopes.
- Whether the transferred context includes private information.
- Whether the target agent has compatible grants.
- Whether the handoff changes proactivity level.
- Whether human approval is required.

Handoffs should transfer context by reference where possible. Copying broad
context into another agent's durable space should require stronger policy.

## Audit requirements

Security-relevant events should be structured.

The audit trail should answer:

- Who acted?
- Which agent acted?
- Which run caused it?
- Which trigger caused the run?
- Which scopes were active?
- Which policy allowed or denied it?
- Was approval required?
- Who approved it?
- Which resource was touched?
- Which destination received data?
- Which memory was used?
- Which tool adapter executed the operation?
- What result was returned?

Audit entries must exclude secrets. Private payloads may be stored behind
governed references or protected artifacts when the event does not need an
inline copy. Ordinary audit metadata should remain intact for diagnosis.

## Fencing and rollback

Disabling a capability, removing authentication, changing policy, or replacing
a credential revision fences future invocation. A pending action request is
superseded when its live authority no longer matches.

Rollback remains adapter-specific. Noema distinguishes preventing a future
action from reverting state or creating a compensating action.

## Inspection surfaces

Current action request surfaces show the target, saved arguments, destination,
information egress, reviewer classification, and human decision. Secrets remain
excluded. Detailed policy evidence stays behind authenticated disclosure.

## Security acceptance scenarios

- A tool result containing instructions remains untrusted content.
- A secret-backed tool keeps credential values outside model context.
- A reviewed external write follows the current deterministic review matrix.
- A run can resume after one saved human decision.
- Live revalidation supersedes a stale request before invocation.
- An external write with an unknown outcome is not retried automatically.
- Exact sanitization preserves ordinary technical values.
