# Unified API and MCP Integration Management

**Mode:** Plan only

**Status:** Complete

**Date:** 2026-07-27

## Outcome

Present APIs and MCPs as two source kinds inside one integration management
system. They retain different setup, authentication, transport, and canonical
stores, but share the same definition -> connections -> tools hierarchy,
connection policies, four-field tool behavior, execution-decision resolver,
management service, GraphQL contract, and Settings components.

This plan starts only after the runtime-restoration plan has removed
effect/admission semantics from the shared capability path. It must not
reintroduce them in adapter manifests, management types, UI, review records, or
tests.

The product hierarchy is:

```text
Integration definition
  API definition | MCP transport/discovery definition
    Connection
      source-specific authentication and configuration
      data-sharing policy
      unsafe-action policy
      Tools
        enabled state
        readOnly, idempotent, destructive, openWorld
        source/model/default/human provenance
```

## Decisions

- Rename Settings **Connections** to **APIs** and replace
  `/settings/tools/connections` with `/settings/tools/apis`. Do not retain a
  compatibility route in this pre-V1 product.
- Keep **APIs** and **MCPs** as sibling Settings pages because setup differs,
  while both use one source-filtered management surface.
- Group both sources as definition -> connections -> tools. One definition can
  own several independently authenticated connections.
- Create MCP grouping only through explicit setup. Adding an MCP creates a new
  definition and first connection; **Add connection** on an existing group
  reuses that exact transport/discovery revision. Never infer grouping from
  names, URLs, descriptions, or discovered prose.
- Store both trust-policy choices per connection. Definitions do not supply
  inherited defaults, and sibling connections do not inherit from one another.
- Apply the same enabled state and four behavior hints to API and MCP tools.
  Human overrides are supported for both and fenced to the exact source
  revision reviewed.
- MCP annotations seed hints when present. The existing classifier fills only
  missing hints. API proposals carry structured model-generated hints; the same
  source-neutral classifier fills missing fields. Classification failure uses
  the current pessimistic defaults.
- Keep source-specific facts outside common policy: API origin, method/path,
  OAuth scopes, authentication, grants, pagination, and transport retry; MCP
  command/endpoint, transport, discovery metadata, and OAuth setup.
- Keep API definitions/connections filesystem-canonical and MCP state
  SQLite-canonical. Share domain contracts and behavior rather than creating a
  universal integration store or credential format.

## Product and interaction model

The human is configuring one concrete connection. The focal job is confirming
what data may be shared, how unsafe calls are handled, and whether each enabled
tool's behavior is accurate. Health/authentication state comes next; scopes,
transport, fingerprints, and raw source details stay behind disclosure.

### Integration list

Both `/settings/tools/apis` and `/settings/tools/mcps` render the same grouped
list with a required source-kind filter.

- A definition group shows its name, readiness, connection count, and one
  source-specific summary. Expansion contains compact connection rows with
  account/installation label, health/auth state, available tool count, policy
  summary, and supported actions.
- **Add connection** belongs to the definition group. APIs reuse the reviewed
  immutable definition and begin fresh credential setup. MCPs reuse the exact
  immutable transport/discovery revision and perform fresh authentication and
  discovery.
- Pending API definitions appear before connected groups. The empty APIs state
  directs the human to ask the agent to research an official API; it does not
  expose a generic arbitrary-URL importer. Definition review shows proposed
  behavior and provenance before approval.
- Adding an MCP definition remains an explicit Settings action and creates its
  first connection only after setup/discovery succeeds.

### Connection detail

Use an addressable shared detail route rather than extending the current
multi-step MCP policy modal:

```text
/settings/tools/apis/$connectionId
/settings/tools/mcps/$connectionId
```

The active navigation item remains APIs or MCPs. Mobile uses the route body
rather than compressing the editor into a dialog.

Information order:

1. **Connection:** identity, readiness, health/authentication state, and the
   source-specific reconnect or completion action.
2. **Sharing and unsafe calls:** the two required connection choices with one
   save boundary.
3. **Tools:** enabled state, effective four-hint summary, provenance, and an
   editor for one exact tool revision.
4. **Source details:** API scopes/origin/grants or MCP transport/discovery data,
   disclosed after the current action.

New connections expose no callable tools until the human explicitly saves both
policy choices and every enabled tool has complete behavior. The UI may prefill
`allow_automatically` and `reviewer_may_approve` as recommendations, matching
today's MCP flow, but they are not authoritative until Save.

The tool editor shows every hint, its current provenance, and any active human
override. **Reset** returns all four fields to the current source-revision
defaults; it never revives values from an older revision.

Use existing Settings rail, row, detail, disclosure, and Astryx control
patterns. Keep management density compact, preserve one-column semantic order
at narrow widths, and do not add cards or headings that do not represent an
independent object or decision boundary.

## Shared domain and policy authority

After the prerequisite plan, `noema-capabilities` already owns complete tool
behavior and the three execution decisions. Move the remaining MCP-specific
management vocabulary there as source-neutral types:

- `CapabilityIntegrationKind`: `Api` or `Mcp`;
- definition, connection, and tool keys that pair structured kind with the
  source-owned stable ID;
- `CapabilityConnectionPolicy`: data-sharing policy, unsafe-action policy, and
  monotonic revision;
- `CapabilityToolPolicy`: enabled/status, four provenance-bearing hints, exact
  source revision, and monotonic policy revision;
- `CapabilityToolHintSource`: `Annotation`, `Model`, `SafeDefault`, or `Human`.

Reuse the existing persisted values:

- data sharing: `allow_automatically` or `review_every_call`;
- unsafe calls: `always_ask`, `reviewer_may_approve`, or `never_ask`;
- tool state: `pending`, `ready`, `defaulted`, or `disabled`.

Reject `review_every_call + never_ask`. Missing connection policy, incomplete
hints, stale revisions, no enabled tools, failed authentication, or unhealthy
transport prevents that connection from advertising bindings.

The classifier receives only bounded prompt-safe tool name, description, and
schema field metadata. It never receives credentials, imported source bytes,
account data, tool arguments, or results. Root capability code owns pure
metadata/prompt/response/defaulting behavior; source services retain their
existing asynchronous completion scheduling and canonical persistence.

### Execution decision

Use the restored original resolver for both source kinds:

```text
risky = !readOnly && (destructive || openWorld)
unsafe = risky || dataSharing == review_every_call

if !unsafe:                              EXECUTE_IMMEDIATELY
if unsafe && unsafeActions == never_ask: EXECUTE_IMMEDIATELY
if unsafe && unsafeActions == always_ask: HUMAN_REVIEW
otherwise:                               LLM_REVIEW
```

No API or MCP type stores read/write/export effects or admission modes. The
connection-bound catalog captures the decision, behavior revision, provider
policy revision, source-tool revision, destination, and operation target. The
invoker revalidates the same authority immediately before a send.

`idempotent` remains behavior evidence. For APIs, automatic transport retry is
available only when all three conditions hold:

1. the immutable definition has a reviewed transport retry contract;
2. the HTTP method satisfies that contract; and
3. the connection's current effective tool behavior has `idempotent == true`.

The definition compiler validates conditions one and two. Catalog compilation
joins condition three into the connection-bound operation authority, and the
invoker revalidates its exact policy revision before choosing one or two
attempts. MCP receives no automatic retry in this plan.

## Source-specific models

### APIs

- `definition_id` remains the stable integration identity; immutable semantic
  digests are reviewed revisions beneath it.
- Connections stay pinned to one semantic digest. A newer definition shows
  **Update available** and never silently rebinds credentials or grants.
- Advance the canonical definition to manifest v3. Each operation contains
  structured candidates for the four behavior hints and their provenance;
  `effect` and `admission` are absent.
- Advance the connection descriptor to v2. It owns connection policy, enabled
  operations, human overrides, behavior/policy revisions, and readiness.
  Credential material remains in immutable private generations.
- Scopes, grants, account kind, origin, method/path, retry, pagination, and
  authentication remain API-specific.

Existing v1/v2 definitions use a migration-only reader. Convert the current
operation classification conservatively: retain only behavior that the old
record proves, apply pessimistic values for everything else, and mark every
affected connection policy-required with tools pending explicit behavior
review. Do not let a historical classification silently authorize execution.

### MCPs

- Introduce a stable random MCP definition ID owning display name, transport
  kind, non-secret command/endpoint configuration, and immutable revision.
- A concrete connection owns authentication references, account/install
  identity, connection policy, health/auth state, and authority generation.
- Tools and discovered schemas remain connection-specific because accounts or
  installations can expose different tool sets.
- **Add connection** copies only the exact non-secret definition revision. It
  performs fresh authentication and discovery and never copies credentials,
  tool policies, overrides, or connection policy.
- Endpoint/command editing remains deferred; the human creates a new
  definition.

## Management service and GraphQL

Add one concrete management service composed from the existing adapter and MCP
services. It normalizes reads and dispatches commands by structured kind. Do
not add a generic provider registry or route from prefixes/display values.

Expose source-neutral management operations and retire MCP-only policy/settings
reads plus the Settings-only adapter definition read after frontend migration:

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

Integration results contain key, name, kind, review/readiness state, source
revision, connection summaries, and a source-details union. Connections contain
identity, readiness, health/auth state, tool counts, optional policy/revision,
and source details. Tools contain source ID/revision, safe name/description,
enabled state, behavior/provenance, decision preview, and policy revision.

Every mutation carries structured kind, exact connection ID, current connection
policy revision, exact source-tool revision, and current tool-policy revision
when applicable. The server re-resolves ownership and rejects stale or
mismatched input before writing.

Keep source-specific setup commands for definition approval, credential
import/OAuth, MCP definition/connection setup, reauthentication, and removal.
Return a normalized setup target containing a definition reference, optional
connection reference, and next step; definition approval cannot pretend a
connection already exists.

## Persistence and migration

Append a forward SQLite migration that creates one MCP definition and one
connection for each existing server. Preserve the server ID as connection ID;
rewrite tool, authentication-request, reviewed-action destination, and policy
references; move non-secret transport configuration to the definition; keep
secret references and health/auth state on the connection. Preserve
fingerprints, overrides, policy revisions, authority generations, pending
authentication, and enabled state.

Extend the existing pre-SQLite adapter `prepare_filesystem` authority instead of
adding another upgrader:

1. recover interrupted connection/schedule replacements;
2. accept skipped-release manifest v1 and current manifest v2;
3. install the immutable manifest-v3 replacement;
4. atomically rewrite connection-v1 descriptors to connection v2 with missing
   connection policy and pending behavior review;
5. rebind schedules and preserve credential/cursor generations exactly;
6. quarantine superseded definition objects only after no live reference
   remains;
7. reconcile body-free SQLite projections after the filesystem is coherent.

Normal readers accept only manifest v3 and connection v2 after startup. A
failed identified migration blocks startup rather than partially advertising
old authority. Rebuilding SQLite from the filesystem must reproduce definition,
connection, policy, enabled-tool, override, scope, and readiness projections
without credential or raw manifest bodies entering SQLite.

## Implementation sequence and budgets

Each unit is one commit with its own size report.

1. **Shared management policy** — Move/rename MCP policy, provenance,
   classification/defaulting, and readiness contracts into
   `noema-capabilities`; adapt MCP without changing the restored execution
   matrix. Budget: +350 production, +220 tests, at most five tests.
2. **API policy and schema v3/v2** — Add behavior candidates, connection
   policies/overrides, exact revision mutations, chained filesystem migration,
   connection-bound retry gating, and shared catalog compilation; delete API
   effect/admission runtime authority. Budget: +900 production, +480 tests, at
   most eight tests.
3. **MCP definition/connection hierarchy** — Add explicit definition ownership,
   split connection state, implement Add connection, and run the SQLite
   migration while retaining exact tool/auth/review fences. Budget: +900
   production, +500 tests, at most eight tests.
4. **Unified management API** — Compose reads and commands, expose GraphQL,
   return normalized setup targets, migrate the frontend, then remove
   superseded Settings-only operations. Budget: +600 production, +300 tests, at
   most six tests.
5. **Shared Settings experience** — Rename route/navigation to APIs, build the
   grouped list and connection detail route, reuse source setup components, and
   replace the MCP-only permissions modal. Budget: +700 TypeScript production;
   no frontend tests.
6. **Consolidation** — Delete obsolete source-specific management models and
   components, update durable contracts and `docs/context/current.md`, and
   measure the complete slice. This unit must be net-negative outside generated
   artifacts, schema migration SQL, and docs.

Stop a unit if production or test changes exceed its estimate by 50% or 500
lines, whichever is smaller, or if implementation needs a third policy/storage
authority.

## Focused tests and acceptance

- Shared policy tests prove source-neutral readiness, provenance, pessimistic
  defaults, exact-revision overrides/reset, and the invalid policy pair. Do not
  duplicate the execution matrix already proven by the prerequisite plan.
- API store/migration tests prove v1 and v2 definitions reach v3, connection v1
  reaches v2, credentials/cursors never change, stale revisions fail,
  interrupted replacement recovers, and SQLite rebuild remains body-free.
- API invocation tests prove two connections on one definition can make
  different policy/behavior decisions; retry requires current effective
  idempotency plus the immutable transport contract.
- MCP migration tests preserve IDs, fingerprints, overrides, auth references,
  reviewed-action destinations, pending authentication, and callable behavior.
- MCP grouping tests prove only explicit definition selection can add a
  connection and that authentication/discovery/policies remain independent.
- Management/GraphQL tests cover structured kind/ID ownership, exact revisions,
  normalized reads/setup targets, safe errors, and equivalent API/MCP behavior.
- Frontend validation uses generated types, lint, and production build. Static
  review covers empty, pending, authentication-required, policy-required,
  ready/defaulted, disabled, stale, error, many-connection, long-name, and
  narrow-screen states. Browser inspection requires separate authorization.

Acceptance scenarios:

1. One API definition with two OAuth accounts renders one group and two
   independently configurable connections.
2. One MCP definition with two explicitly added installations renders the same
   hierarchy without sharing credentials, discovery results, or policies.
3. Equivalent API and MCP behavior/policies produce the same
   `EXECUTE_IMMEDIATELY`, `HUMAN_REVIEW`, or `LLM_REVIEW` decision.
4. Model-prefilled API hints show model provenance, accept human overrides, and
   become stale when the operation digest changes.
5. Missing policy or incomplete enabled-tool behavior advertises no capability.
6. Delayed review/authentication cannot resume after definition, connection,
   credential, grant, source-tool, behavior, or policy revision changes.
7. Existing MCP state survives without reconnecting. Existing API credentials
   and cursors survive migration, but tools remain unavailable until behavior
   and both connection policies are confirmed.

## Stop conditions and deferred work

Stop if implementation introduces effect/admission terminology, a second
execution-decision resolver, a universal credential/transport model, policy
inference from prose or prefixes, a duplicate capability catalog, secrets in
GraphQL/SQLite, or automatic account grouping. Do not reintroduce operation
result privacy, route, retention, or model/durable payload policies; the single
canonical connection-result contract remains authoritative.

Do not weaken DNS, redirect, proxy, canonical payload, outcome-uncertainty,
review, authentication, or revision-fence protections. Defer definition-level
policy inheritance, bulk editing, provider catalog/signing, automatic MCP
identity matching, endpoint editing, multi-human sharing, durable OAuth
attempts, refresh, audit-history UI, and cross-connection tool deduplication.
The stuck chat OAuth intervention card remains a separate lifecycle bug rather
than a reason to expand this management plan.
