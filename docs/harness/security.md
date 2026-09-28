# Harness Security Model

This document describes the current Go backend, audited on September 28, 2026.
It separates implemented controls from required information handling and controls that do not exist.
The earlier version mixed current behavior with a broader policy design.

Noema checks tool inputs, connection policy, saved action state, and execution ownership in code.
For actions routed to model review, a model judges human authorization and risk.
Code then applies a fixed decision matrix to that judgment.
The matrix is deterministic; the judgment is not.

Noema does not implement a general information-flow security system.
It does not track every private value from its source to every possible destination.
It also does not provide general project, agent, relationship, or resource access grants.

Public authentication and local socket access are covered by [server security](../server-security.md).
This document covers the agent runtime after access to Noema has been established.

## Current authority boundaries

The implemented personal-agent paths use the local human identity, `human:local`.
Tasks, projects, conversations, and runs have identifiers and ownership relationships.
Those relationships are not a general multi-user access-control hierarchy.

Connector availability comes from current connections, enabled tools, authentication, and published operation definitions.
Runtime roles also limit which tool categories are exposed.
Loading a deferred tool definition does not grant additional permission to execute it.
The connector catalog is not filtered through the proposed per-agent resource-grant system.
Connector OAuth scopes exist, but they are different from that proposed Noema grant hierarchy.

Sources: [MCP catalog and invocation](../../internal/mcp/service.go),
[HTTP adapter service](../../internal/adapter/service.go), and
[deferred tool loading](../../internal/runtime/tool_loading.go).

## Connection policy and action routing

HTTP API and MCP connections combine operation behavior with two connection settings:

- Data sharing: allow automatically, or review every call.
- Risky actions: always ask, let the reviewer decide, or never ask.

The current routing rule treats an operation as requiring review when either condition holds:

```text
not read-only AND (destructive OR open-world)
OR connection data sharing is review_every_call
```

The risky-action setting then selects immediate execution, human review, or model review.
Connection settings reject the combination of review-every-call sharing and never-ask execution.
A read-only operation can therefore send arguments to its service without an action review.
A write can also execute immediately when its behavior and connection policy permit that route.

These checks use operation metadata and connection settings.
They do not classify each argument as ordinary or private information.
An inaccurate operation definition or MCP behavior hint can affect the selected route.
Connection and tool configuration are part of the trusted setup.

Sources: `reviewRoute` in the [HTTP adapter](../../internal/adapter/service.go) and
[MCP service](../../internal/mcp/service.go).

## Reviewed actions

Reviewed Chat and Task calls create durable action requests with exact arguments.
The store checks the originating tool call and active turn or Task run before admitting a request.

The reviewer receives bounded evidence of human authorization:

- Chat: up to seven completed user-facing messages, ending at the triggering authenticated human item.
- Chat-created Tasks: a saved copy of that excerpt.
- Manually created Tasks: authenticated title and document inputs.
- Task continuations: eligible human replies from the current Task generation.

Task replies are limited to the current run, its parent, and replies not yet consumed.
The context rejects more than 64 eligible replies and retains an overall size limit.
Human Task edits can replace saved authority; agent document edits cannot.
Child Tasks inherit their parent's saved authority. Recurring Tasks inherit their template's snapshot.

This preserves the source of authority for review.
It does not prove that a proposed action follows the human's meaning.
That interpretation remains the reviewer's job when model review is selected.

The reviewer has no tools and returns authorization and risk classifications.
Code applies this matrix:

| Authorization | Risk | Result |
| --- | --- | --- |
| Explicit or substantive | Low or medium | Execute |
| Weak | Low | Execute |
| Other valid combinations | Any | Ask the human |
| Invalid or unavailable review | — | Ask the human |

A human decision applies to the saved action revision.
Execution admission consumes the approval and checks the saved arguments and current origin.
Connector execution also revalidates its current binding, policy, authentication, and schema.
Browser calls have additional session and snapshot checks.

These controls prevent stale or changed calls from reusing an approval.
They do not guarantee that an approved action is harmless or that a remote effect occurs exactly once.
An uncertain recorded outcome is distinct from failure and must not be automatically replayed.

Sources: [action records and decision matrix](../../internal/store/action_requests.go),
[human evidence](../../internal/store/task_authorization.go),
[reviewer input](../../internal/runtime/action_reviewer.go), and
[browser execution](../../internal/runtime/web_tools.go).
See [action governance](action-governance.md) for the detailed action lifecycle.

## Paths outside action review

There is no single egress gate for everything a run emits.

| Path | Current boundary |
| --- | --- |
| API and MCP calls | Connection policy can select immediate execution instead of creating an action request. |
| Web search | Sends the query to the configured provider without a human action decision. |
| Observed URL fetch | An eligible exact observed URL can bypass action review. Fetch validation still applies. |
| Observed URL download | An observed URL can bypass action review. Download and local file checks still apply. |
| Chat replies and model requests | Follow context construction and provider routing, not a universal destination-and-audience permission check. |
| Local Task files and Task updates | Follow their tool-specific path, input, and current-run checks. |
| Memory publication | Uses the memory change validator and publisher. It is not an external-action approval. |
| Delegation | Uses Task creation and run rules, including inherited authority. It has no separate compatible-grants decision. |

The observed-URL lookup is instance-wide. It does not require the current Task or conversation to have discovered the URL.
An observed URL is not trusted content or a general authorization grant.
Network checks restrict supported requests to allowed targets.
They do not establish that a query contains no private information.
Noema must not claim that every private search query or model prompt receives a separate disclosure review.

Sources: [web routing](../../internal/runtime/web_tools.go),
[downloads](../../internal/runtime/file_download.go),
[observed URLs](../../internal/store/web_tools.go),
[public-network checks](../../internal/netpolicy/public.go), and
[Task delegation](../../internal/runtime/task_tools.go).

## Prompt injection and provenance

External pages, tool results, memory, and model-written Task documents can reach model context.
They can contain hostile instructions or false statements.
Noema does not guarantee that the acting model or reviewer will ignore those instructions.

Context roles, source identifiers, and descriptive annotations provide some provenance.
The earlier proposed trust-label vocabulary is not a universal runtime classification system.
There is no general ingress detector that proves prompt-injection content has been removed.
Memory's root page is currently inserted as developer-role context with a descriptive memory label.
That label is not a separate isolation boundary.

Saved human evidence remains separate from mutable Task documents during action review.
Exact input checks, current bindings, and approval consumption still apply when a model follows malicious content.
Those controls reduce specific attack paths; they do not prove that all unintended disclosure is blocked.

Sources: [context assembly](../../internal/runtime/memory_tools.go),
[reviewer input](../../internal/runtime/action_reviewer.go), and
[Task authority admission](../../internal/store/task_authorization.go).

## Information classes and mechanisms


The required handling contract has exactly three information classes.
These are engineering requirements, not labels automatically attached to every runtime value.
They describe information, not permission to access it:

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


## Credential handling: implementation and limits

Noema-managed connector credentials use protected stores and adapter-side injection.
The model-facing connection uses identifiers and metadata instead of credential values.
HTTP adapter results receive sanitization using known secret values and declared credential locations.
MCP handling also has secret stores, sanitized views, and configured persistence behavior.

This is not proof that arbitrary text is secret-free.
A human can paste a credential into a message.
A remote service can return an undeclared credential inside ordinary prose.
The three-class contract above requires excluding those secrets, but classification is not universally enforced for arbitrary content.

Treat any such exposure as a defect against the handling contract.
Do not describe the contract as an implemented universal secret detector.
Preserve ordinary values and authorized private information when correcting a defect.

Sources: [HTTP request and result handling](../../internal/adapter/service.go),
[MCP secret store](../../internal/mcp/secrets.go), and
[MCP persistence views](../../internal/mcp/binding_policy.go).

## Memory safety

Memory belongs to the local human.
It has no private memory scopes or memory-specific access grants.
The root page and its child-page list can enter Chat context; other tools read and search the same memory store.

Memory updates use a model to propose page changes.
Code checks editable pages, expected page state, paths, citation identifiers, and citation-marker consistency before publication.
Eligible human messages and saved tool results can support citations.
Previously stored citation identifiers also enter the allowed set.

These are structural checks, not factual verification.
A valid source identifier does not prove that its source supports a nearby claim.
The validator does not prove that every sentence has evidence or that cited external material is true.
The document must not promise that every published claim is independently verified.

Memory cannot create independent human authority in the action-review contract.
It can still influence the acting model because it is part of context.

Sources: [memory context and tools](../../internal/runtime/memory_tools.go),
[evidence selection](../../internal/runtime/memory_consolidation.go),
[change validation](../../internal/runtime/memory_changes.go), and
[publication](../../internal/memory/publish.go).

## Audit and recovery

Reviewed actions retain their arguments, assessments, human decisions, execution state, and outcome events.
Task and Chat records also retain tool activity and run history.
Immediate connector calls do not all create the reviewed-action ledger described above.
These records are not a complete information-flow ledger for every value, recipient, or memory influence.

Current-run and generation checks reject stale Task work.
Changed connector bindings can invalidate pending execution.
Cancellation cannot undo an external effect that already happened.
Rollback and compensation depend on the tool or remote service.

Sources: [action ledger](../../internal/store/action_requests.go),
[Task execution](../../internal/store/task_execution.go),
[API actions](../../internal/runtime/adapter_actions.go), and
[MCP actions](../../internal/runtime/action_requests.go).

## Controls that are not implemented as a general system

Do not present these as current product guarantees:

- Per-human, workspace, project, agent, relationship, and resource grant composition.
- The proposed deny/revocation/grant precedence hierarchy.
- Contextual elevation with resource selectors, expiration, and revocation records.
- Universal trust labels on all model-visible content.
- Automatic ordinary/private/secret classification of every value.
- Tracking private information through transformations and agent handoffs.
- A destination-and-audience disclosure check for every model request, reply, search, notification, file write, or delegation.
- Deterministic prevention of all prompt injection or memory poisoning.
- A complete audit trail that proves every output's information sources and authorized audience.

Specific subsystems enforce narrower controls described above.
New capabilities must follow the information-handling contract without claiming these broader systems already exist.
