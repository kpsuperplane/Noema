# Unified API and MCP Integration Management

**Mode:** Plan only

**Status:** Decision complete

**Date:** 2026-07-27

## Summary

Noema should present APIs and MCPs as two source kinds inside one integration
management system. They already converge at runtime through the same
`CapabilityBinding` catalog, router, governed-action gateway, and invoker
registry. This work moves the management and policy boundary to the same shared
layer without forcing API and MCP setup into one lifecycle.

The product hierarchy is:

```text
Integration definition
  API definition | MCP transport/discovery definition
    Connection
      source-specific authentication and configuration
      connection-specific sharing and approval policy
      Tools
        enabled state
        read-only, idempotent, destructive, and open-world policy
        source provenance and human overrides
```

Settings keeps separate **APIs** and **MCPs** destinations because their setup
jobs differ, but both pages use the same definition groups, connection rows,
connection detail surface, policy controls, tool editor, GraphQL management
contract, and source-neutral policy resolution.

## Decisions

- Rename Settings **Connections** to **APIs** and replace
  `/settings/tools/connections` with `/settings/tools/apis`. Do not retain a
  compatibility route in this pre-V1 product.
- Keep **APIs** and **MCPs** as sibling Settings pages backed by one shared
  management surface filtered by source kind.
- Group both sources as definition -> connections -> tools. One definition can
  own several independently authenticated connections.
- Create MCP grouping only through explicit setup. Adding an MCP creates a new
  definition and its first connection; **Add connection** on an existing MCP
  definition reuses that exact transport/discovery contract. Never group by
  name, URL text, descriptions, or discovered prose.
- Store sharing and approval policy per connection. Definitions do not supply
  inherited defaults, and connections do not inherit policy from one another.
- Apply the same enabled state and four behavior hints to API and MCP tools.
  Human overrides are supported for both and are fenced to the exact source
  revision they reviewed.
- MCP annotations seed hints when present. The existing classifier fills
  missing MCP hints. API definition proposals carry structured model-generated
  hints; the same classifier fills any missing hints before review. Failed
  classification uses the existing pessimistic safe defaults.
- Keep source-specific facts outside the common policy: API origin, HTTP
  method/path, OAuth scopes, authentication, grants, pagination, and transport
  retry; MCP command/endpoint, transport, discovery metadata, and OAuth setup.
- Keep API definitions/connections filesystem-canonical and MCP state
  SQLite-canonical. Share the domain model, management service, GraphQL API,
  policy resolver, and UI rather than introducing a second canonical store.

## Product and interaction model

The focal object is an integration definition and its concrete connections;
source metadata and tool internals remain progressively disclosed. Settings
uses compact management density and one-column ordering on narrow screens.

### Integration list

Both `/settings/tools/apis` and `/settings/tools/mcps` render the same list
component with a required source-kind filter.

- A definition group shows display name, status, connection count, and a
  source-specific summary. Its expansion contains connection rows with
  account/installation label, readiness, auth/health state, available tool
  count, sharing/approval summary, and supported row actions.
- **Add connection** belongs to the definition group. API definitions reuse the
  reviewed definition and begin source-specific credential setup. MCP
  definitions reuse the exact immutable transport/discovery revision.
- Pending API definitions appear before connected groups. An empty APIs page
  directs the human to ask the agent to research an official API; it does not
  fabricate a generic URL importer. Definition review shows the four proposed
  hints and provenance before approval. Adding an MCP definition remains an
  explicit Settings action and creates its first connection after verification.

### Connection detail

Use an addressable shared connection detail surface instead of extending the
current multi-step MCP policy modal:

```text
/settings/tools/apis/$connectionId
/settings/tools/mcps/$connectionId
```

The active navigation item remains APIs or MCPs. Mobile uses the route body
rather than compressing the editor into a dialog.

1. **Connection:** account/installation identity, readiness, health/auth state,
   and source-specific reconnect or completion action.
2. **Sharing and approvals:** the two required connection policies and one save
   boundary.
3. **Tools:** enabled state, effective four-hint summary, provenance, and an
   editor for one exact tool revision.
4. **Source details:** API scopes/origin/grants or MCP transport/discovery data,
   disclosed after the current action.

New connections are not callable until the human explicitly saves both policy
choices and every enabled tool has a complete behavior policy. The UI may
prefill `allow_automatically` and `reviewer_may_approve` as recommendations,
matching today's MCP flow, but nothing becomes authoritative until Save.

The shared tool editor shows all four hints, each effective source, and active
human overrides. **Reset** returns all four fields to current source-revision
defaults without reusing values from an older revision.

## Shared domain and policy authority

Move the MCP-specific policy vocabulary into `noema-capabilities`. The model is:

- `CapabilityIntegrationKind`: `Api` or `Mcp`.
- `CapabilityDefinitionKey`: kind plus source-owned stable definition ID.
- `CapabilityConnectionKey`: kind plus source-owned stable connection ID.
- `CapabilityToolKey`: connection key plus source-owned tool ID.
- `CapabilityConnectionPolicy`: data-sharing policy, unsafe-action policy, and
  monotonic revision.
- `CapabilityToolPolicy`: enabled/status, four provenance-bearing boolean
  hints, source revision, and monotonic policy revision.
- `CapabilityToolHintSource`: `Annotation`, `Model`, `SafeDefault`, or `Human`.

Reuse the existing MCP wire values:

- Data sharing: `allow_automatically` or `review_every_call`.
- Unsafe actions: `always_ask`, `reviewer_may_approve`, or `never_ask`.
- Tool state: `pending`, `ready`, `defaulted`, or `disabled`.

Retain the invalid `review_every_call + never_ask` combination. Missing policy,
pending hints, stale revisions, no enabled tools, failed auth, or unhealthy
transport prevents a connection from advertising bindings.

The shared classifier receives only bounded, prompt-safe tool metadata and
never credentials, imported source bytes, account data, or tool results.

### Effective policy resolution

One source-neutral resolver owns risk and admission derivation for both source
kinds:

```text
risky = destructive
     or (not readOnly and openWorld)
     or dataSharing == review_every_call

if not risky and readOnly:                 effect = ReadOnly
if not risky and not readOnly:             effect = ExternalWrite
if risky and readOnly:                     effect = ExternalExport
if risky and not readOnly and openWorld:   effect = ExternalWriteAndExport
otherwise:                                 effect = ExternalWrite

if not risky:                              admission = Direct
if risky and unsafeActions == always_ask:  admission = AlwaysAsk
if risky and unsafeActions == reviewer_may_approve:
                                             admission = ReviewerMayApprove
if risky and unsafeActions == never_ask:   admission = PolicyAuthorizedDirect
```

The adapter compiler must stop retaining `effect` and `admission` as parallel
runtime authorities once tool behavior moves into the shared policy. API
definition review covers structured behavior defaults; connection policy and
human tool overrides determine the effective binding at catalog time.

`idempotent` is behavior evidence, not transport permission. API automatic
retry remains allowed only when the reviewed operation's source-specific retry
contract and HTTP method support it; the compiler must also require the
effective tool policy to be idempotent. MCP calls gain no automatic retry in
this slice.

Human overrides are complete four-field snapshots fenced by source and policy
revision. An API operation-digest or MCP metadata-fingerprint change stales the
override and blocks the tool until reclassification/defaulting completes.

## Source-specific models

### APIs

- `definition_id` is the stable integration identity; semantic digests are
  immutable reviewed revisions beneath it.
- Connections remain pinned to an exact semantic digest. A later definition
  revision shows **Update available** and never silently rebinds credentials or
  grants.
- Each operation declares structured candidate values for the four behavior
  hints. Model-generated values carry `model` provenance. Missing values enter
  the common classifier; safe defaults remain visible as `safe_default`.
- The connection descriptor owns connection policy, enabled operations, human
  overrides, and their revision. Credential material remains in immutable
  private generations.
- Scopes, grants, account kind, origin, method/path, retry, pagination, and
  authentication remain API-specific detail.

### MCPs

- Introduce a stable random MCP definition ID that owns display name,
  transport kind, non-secret command/endpoint configuration, and an immutable
  definition revision.
- A concrete MCP connection owns authentication references, account/install
  identity, connection policy, health/auth state, and an authority generation.
- Tools and discovered schemas remain connection-specific because accounts or
  installations can expose different tool sets.
- **Add connection** copies only the exact non-secret definition revision. It
  performs fresh authentication and discovery and never copies credentials,
  tool policies, or connection policy.
- Endpoint/command editing is deferred; the human creates a new definition.

## Management service and GraphQL contract

Add one concrete management service composed from the existing adapter and MCP
services. It normalizes reads and dispatches commands by the structured kind;
do not add a generic provider registry or infer routing from ID prefixes.

Expose these GraphQL operations and retire the MCP-only policy/settings reads
and the Settings-only adapter definitions read after the frontend migrates:

```graphql
enum CapabilityIntegrationKind { API MCP }

input CapabilityConnectionRefInput {
  kind: CapabilityIntegrationKind!
  connectionId: String!
}

capabilityIntegrations(kind: CapabilityIntegrationKind!): [CapabilityIntegration!]!
capabilityConnection(ref: CapabilityConnectionRefInput!): CapabilityConnection
capabilityTools(ref: CapabilityConnectionRefInput!): [CapabilityManagedTool!]!

saveCapabilityConnectionPolicy(input: SaveCapabilityConnectionPolicyInput!): CapabilityConnection!
saveCapabilityToolOverride(input: SaveCapabilityToolOverrideInput!): CapabilityManagedTool!
resetCapabilityToolPolicy(input: ResetCapabilityToolPolicyInput!): CapabilityManagedTool!
setCapabilityToolEnabled(input: SetCapabilityToolEnabledInput!): CapabilityManagedTool!
```

The integration result contains its key, name, kind, review/readiness state,
source revision, connection summaries, and source-details union. A connection
contains its keys, label, readiness, health/auth state, tool counts, optional
policy/revision, and source details. A tool contains source ID/revision, safe
name/description, enabled state, hints/provenance, and policy revision.

Every mutation carries the structured source kind, exact connection ID, current
connection-policy revision, exact tool source revision (API operation digest or
MCP metadata fingerprint), and current tool-policy revision when applicable.
The server re-resolves source ownership and rejects stale or mismatched input
before writing. GraphQL never accepts source selection through a display name,
prefix, URL, or English phrase.

Keep source-specific setup mutations for API definition approval, credential
import/OAuth, MCP definition creation, MCP connection authentication, discovery,
reauthentication, and removal. They return normalized connection references so
the shared UI can continue into policy setup without a second read model.

## Persistence and migration

SQLite receives a forward migration that creates one definition and one
connection for each existing MCP server. Preserve the server ID as connection
ID; rewrite tool, auth-request, governed-action destination, and policy
references; move non-secret transport config to the definition; keep secret
references and health/auth state on the connection. Preserve fingerprints,
overrides, revisions, authority generations, pending auth, and enabled state.

Advance adapter schemas directly. A bounded one-time filesystem upgrader
preserves credential generations, creates a replacement definition with
pessimistic candidates where old data cannot prove a hint, atomically repoints
the descriptor, and marks it policy-required/pending review. It never copies
secret bytes through SQLite or logs. Normal readers accept only the new schema
after upgrade; there is no indefinite compatibility path.

SQLite's adapter rows remain body-free rebuildable projections. Rebuilding the
database from filesystem state must reproduce API definitions, connection
policy revisions, enabled tools, overrides, scopes, and statuses without
credentials or raw manifest bodies entering SQLite.

## Implementation sequence and budgets

Each milestone is a separate committed unit with its own size report. Stop if a
unit exceeds its production or test estimate by 50% or 500 lines, whichever is
smaller, or if it needs a third policy/storage authority.

1. **Shared policy core and catalog resolution** — Move and rename the MCP
   policy types, classification/defaulting helpers, readiness checks, and
   effect/admission resolver into `noema-capabilities`. Adapt MCP catalog
   compilation without changing behavior, then add the API compiler inputs.
   Budget: +450 production, +250 tests, at most six tests.
2. **API policy authority** — Add structured behavior candidates to API
   operations, connection-level policies and overrides to the canonical
   descriptor, revision-fenced mutations, filesystem upgrade/recovery, and
   shared-policy catalog compilation. Remove API `effect`/`admission` as a
   parallel authority. Budget: +850 production, +450 tests, at most eight tests.
3. **MCP definition/connection hierarchy** — Add the explicit definition model,
   split connection ownership, implement add-connection reuse, run the SQLite
   migration, and retain exact auth/tool/action fences. Budget: +900 production,
   +500 tests, at most eight tests.
4. **Unified management API** — Compose normalized reads and common policy/tool
   commands, expose the GraphQL contract, return normalized references from
   source setup, and remove superseded Settings-only reads/mutations. Budget:
   +600 production, +300 tests, at most six tests.
5. **Shared Settings experience** — Rename the route/navigation to APIs, build
   the common grouped list and connection detail route, reuse source setup
   components, replace the MCP-only permissions modal, and preserve compact
   responsive/accessibility behavior. Budget: +700 TypeScript production;
   no frontend tests unless separately requested.
6. **Consolidation and documentation** — Delete obsolete MCP/API management
   models and components, update current frontend/capability contracts and
   `docs/context/current.md`, then measure the complete slice. This milestone
   must be net-negative outside generated artifacts and docs.

## Test and acceptance matrix

Tests stay at the lowest authority and do not repeat one mapping at every layer.

- Shared table-driven policy tests prove all meaningful sharing, approval, and
  four-hint combinations resolve to the expected readiness, effect, and
  admission, including the invalid policy pair.
- Classification tests prove source provenance, model fill, pessimistic
  defaults, exact-revision human overrides, and reset behavior.
- API store tests prove policy/override publication is atomic, credentials
  never enter policy files or SQLite, stale revisions fail, interrupted upgrade
  recovers, and database rebuild preserves the body-free policy projection.
- API catalog/invocation tests prove connection policy and tool hints govern
  bindings, transport retry still requires the hard reviewed contract, and a
  stale definition/operation/policy cannot execute.
- MCP migration tests prove each old server becomes one definition plus one
  connection without changing IDs, fingerprints, overrides, auth references,
  governed-action destinations, or callable behavior.
- MCP grouping tests prove adding a connection reuses only an explicitly chosen
  definition, performs fresh discovery/auth, and cannot group through matching
  names, URLs, or metadata.
- Management tests prove kind/ID ownership, ordering, source details, stale
  mutation rejection, and equivalent API/MCP policy results.
- GraphQL tests cover authorization, exact revision inputs, normalized reads,
  source command routing, and safe errors. Pass-through field mirrors do not
  receive separate tests.
- Frontend receives generated-type, lint, and production-build validation.
  Static review covers empty, pending, auth-required, policy-required,
  classified/defaulted, disabled, stale, error, many-connection, long-name, and
  narrow-screen states. Browser inspection at desktop and mobile widths occurs
  only with explicit authorization.

Acceptance scenarios:

1. One API definition with two OAuth accounts renders one group and two
   independently configurable connections; changing one connection's policy
   does not change the other.
2. One MCP definition with two explicitly added installations renders the same
   hierarchy; credentials, discovery results, policies, and tools remain
   connection-specific.
3. Equivalent API and MCP tool policies compile to equivalent capability effect
   and admission semantics while retaining different invokers and source
   details.
4. A model-prefilled API hint is visible with model provenance, can be
   overridden by the human, and becomes stale when the operation digest changes.
5. A connection missing explicit sharing/approval policy or complete enabled
   tool hints exposes no callable capability.
6. Delayed approval or authentication cannot resume after any definition,
   connection, credential, grant, source-tool, or policy revision changes.
7. Existing MCP state survives migration without reconnecting; existing API
   credentials survive filesystem upgrade but remain unavailable until new
   behavior review and connection policy confirmation are complete.

## Stop conditions and deferred work

Stop and reassess if the implementation needs one universal credential format,
one transport lifecycle, policy inference from text/prefixes, a duplicate
capability catalog, secrets in GraphQL/SQLite, or automatic account grouping.
Do not weaken current DNS, redirect, proxy, payload, outcome-uncertainty,
approval, or revision-fence protections to make the common UI simpler.

Defer definition-level policy inheritance, bulk policy editing, provider
catalog/signing, automatic MCP identity matching, endpoint editing, shared
multi-human connections, durable OAuth attempts, refresh, audit history, and
cross-connection tool deduplication until separately requested.
