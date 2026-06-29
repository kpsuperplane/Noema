# Memory Graph Page Design

Approved design for adding a web UI page where a human can inspect the graph of
their Noema memories.

## Context

Noema already stores durable memories as graph claims in embedded SurrealDB.
GraphQL exposes bounded claim list/detail inspection through `memoryClaims` and
`memoryClaim`, and the CLI uses those fields for `noema memory list` and
`noema memory show <id>`.

The current React web UI is still a chat-first shell with no memory-management
route. Frontend IA docs describe `/memory`, `/memory/:id`, and future richer
graph inspection, but the implemented web app has not yet added those routes.

This design adds the first human-facing memory graph page while keeping chat as
the default product surface.

## Goals

- Add a memory-management page where a human can inspect their accessible memory
  graph.
- Avoid "admin" framing. This is a normal memory-management capability, with
  advanced provenance details available where backed.
- Show entity nodes connected by claim edges as the primary graph.
- Keep evidence and provenance out of the main canvas by default and show them
  in a selected-claim detail panel.
- Add a bounded backend graph read model instead of making the frontend infer a
  graph from claim list/detail queries.
- Support pan, zoom, selection, and fit-to-view in the first graph canvas.
- Preserve backend-owned access and redaction rules.
- Keep the implementation scoped enough for one plan and one unit of work.

## Non-Goals

- Do not build the full future memory-management surface in this slice.
- Do not expose graph inspection as an owner/admin-only dashboard.
- Do not add memory mutation controls unless a matching backend operation
  already exists.
- Do not show evidence or conversation-item source nodes directly in the graph
  canvas by default.
- Do not return the entire unbounded memory corpus in one response.
- Do not let frontend graph assembly infer private labels, facts, or object
  existence from partially redacted data.

## Product Framing

The route should live under memory management:

```text
/memory
/memory/graph
```

`/memory` can start as a restrained memory-management entry point with Graph as
the first available view and room for future List, Review, Access, and Settings
views. `/memory/graph` renders the graph page itself.

The page copy should use human-facing terms:

- Memory Graph
- Memories
- Things Noema remembers
- Evidence
- Source
- Status
- Sensitivity

It should avoid requiring the user to think in owner/admin terms. Future
multi-human access rules can still gate the graph to memories the current human
is allowed to inspect.

## Default Graph Scope

The first load means a bounded view of accessible memories, not the entire
database.

Defaults:

- Statuses: `candidate`, `active`, and `confirmed`.
- Limit: 150 claims.
- Canvas: entity nodes plus claim edges.
- Detail: lazy claim detail fetch when an edge is selected.

Filters should expose backed status values such as `disputed`, `superseded`,
`archived`, and `deleted`. Search and filters refine the bounded graph. If the
response is truncated, the page should still render the returned graph and show
the applied limit.

## Backend Design

Add a bounded graph read model named `memoryGraph`.

The GraphQL query should be `memoryGraph(input: GraphqlMemoryGraphInput)` and
accept one input object rather than a long positional argument list:

```graphql
input GraphqlMemoryGraphInput {
  query: String
  statuses: [String!]
  predicateId: String
  sensitivity: String
  limit: Int
}

type GraphqlMemoryGraph {
  nodes: [GraphqlMemoryGraphNode!]!
  edges: [GraphqlMemoryGraphEdge!]!
  summary: GraphqlMemoryGraphSummary!
}

type GraphqlMemoryGraphNode {
  nodeId: String!
  entityId: String!
  label: String!
  entityType: String!
  redacted: Boolean!
  claimCount: Int!
}

type GraphqlMemoryGraphEdge {
  claimId: String!
  sourceNodeId: String!
  targetNodeId: String!
  predicateId: String!
  predicateLabel: String!
  fact: String!
  factRedacted: Boolean!
  status: String!
  sensitivity: String!
  confidence: Float
  evidenceCount: Int!
  createdAt: String!
  updatedAt: String!
}

type GraphqlMemoryGraphSummary {
  returnedClaimCount: Int!
  returnedNodeCount: Int!
  limit: Int!
  truncated: Boolean!
}
```

The exact names may shift to match existing GraphQL style, but the contract
should remain graph-shaped.

Default resolver behavior:

- If `statuses` is omitted, use `candidate`, `active`, and `confirmed`.
- If `limit` is omitted, use 150.
- Reject `limit < 1`.
- Reject `limit > 500` with a clear GraphQL error.
- Parse status and sensitivity values with the same validation style as
  `memoryClaims`.
- Return only claims the current human can inspect.
- Apply list-level redaction in Rust before returning node labels or edge facts.
- Treat status filters as an OR list.
- Set `summary.truncated` to true when more accessible matching claims exist
  than the returned edge set includes.

The store API should return graph rows directly or return claim records that the
backend folds into graph nodes and edges. Either way, the frontend receives a
display-ready projection.

Objectless claims should not create durable fake entities. If a backing claim
lacks an object entity, the graph projection should still return a selectable
self-edge on the subject node, with the fact and evidence available through the
detail panel.

## Frontend Design

Add a small routing layer so `/` remains chat and memory pages become
addressable. A heavyweight app router is not required for the current route
count; a focused route helper based on `window.location.pathname` and
`history.pushState` is enough unless implementation pressure suggests otherwise.

Primary frontend modules:

- `MemoryHomePage`: memory-management entry point, initially centered on Graph
  and future-ready for List, Review, and Access views.
- `MemoryGraphPage`: owns query variables, route-level loading/error/empty
  states, and selected claim state.
- `MemoryGraphControls`: search, status chips, sensitivity filter,
  predicate/search inputs where backed, and returned-limit indicator.
- `MemoryGraphCanvas`: renders graph nodes/edges, pan/zoom controls, selection,
  and fit-to-view.
- `MemoryGraphDetailPanel`: selected edge detail and lazy `memoryClaim` fetch
  for full fact and evidence/provenance.
- `memoryGraphLayout.ts`: pure helper that converts graph data to stable node
  positions.
- `memoryGraph.ts`: pure normalization helpers for generated GraphQL result
  data.

The UI should stay information-dense and restrained: a compact header, controls,
large graph workspace, and right-side detail panel on desktop. On mobile, the
detail panel can become a bottom sheet or below-canvas panel so the canvas
retains usable height.

## Graph Library Choice

Use `@xyflow/react` (React Flow) for the first graph canvas.

Reasons:

- Built-in pan, zoom, fit-to-view, selection, keyboard affordances, controls,
  minimap support, and custom React nodes/edges.
- Works naturally with React and Tailwind/shadcn-style components.
- Better accessibility support than most canvas/WebGL graph options.
- Leaves layout as a separate pure helper, which fits Noema's testability
  goals.

Add `d3-force` for a deterministic frozen layout helper unless implementation
finds a simpler layout is sufficient. The layout should sort nodes by stable id,
seed deterministic initial positions, run a fixed number of ticks, and return
frozen coordinates. The graph should not keep simulating during ordinary use.

Alternatives considered:

- Cytoscape.js: stronger graph-native analysis engine and built-in layouts, but
  less natural inside the current React component system.
- Sigma.js with Graphology: better for very large WebGL graphs, but lower-level
  and more work for the first memory-management page.
- React Force Graph: fast to prototype, but canvas/WebGL rendering is less
  suitable for accessible, inspectable memory details.

References:

- React Flow: https://reactflow.dev/
- React Flow API: https://reactflow.dev/api-reference/react-flow
- React Flow accessibility: https://reactflow.dev/learn/advanced-use/accessibility
- React Flow layouting: https://reactflow.dev/learn/layouting/layouting
- Cytoscape.js: https://js.cytoscape.org/
- Sigma.js: https://www.sigmajs.org/docs/
- React Force Graph: https://github.com/vasturiano/react-force-graph

## Privacy And Redaction

The backend owns privacy decisions. The frontend must treat graph labels, facts,
and redaction flags as display-ready.

Rules:

- Return only memories the current human may inspect.
- Redact non-public list-level facts and labels at the GraphQL boundary.
- Do not let the frontend reconstruct hidden facts or names from ids.
- Selecting a claim uses the explicit detail pathway through `memoryClaim`.
- If detail fetch is denied or unavailable, the detail panel explains that the
  graph remains usable but the evidence cannot be shown.
- Sensitive or secret reveal controls are future backed operations, not implied
  by this slice.

## Error Handling

Page states:

- Memory storage unavailable: show an unavailable memory state, not an empty
  graph.
- No returned graph rows: show an empty memory graph state with a path back to
  chat and "remember this" usage.
- Graph query error: show a page-level error with retry.
- Truncated response: render returned graph and show the applied limit.
- Claim detail error: keep the graph usable and show the error only in the
  detail panel.
- Unknown route under `/memory`: show the memory-management entry point or a
  small not-found state with a link to Graph.

## Testing

Backend tests:

- Store/read-model tests for default statuses, explicit statuses, query filter,
  predicate filter, sensitivity filter, limit handling, and truncation.
- Resolver tests for malformed limits/statuses/sensitivity.
- Redaction tests proving non-public facts and labels remain conservative in
  graph responses.
- Schema export test coverage through existing GraphQL schema generation.

Frontend tests:

- Route helper tests for `/`, `/memory`, and `/memory/graph`.
- Graph normalization tests for node/edge creation, self-edge fallback, counts,
  and redacted values.
- Layout helper tests for deterministic output.
- Component render tests for loading, empty, error, truncated, selected-edge,
  and detail-fetch error states.

Validation:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

If socket-binding tests fail under sandboxing, rerun the same test command with
the required socket permissions and report that distinction.

## Rollout

Implement in one focused slice:

1. Backend graph read model and GraphQL query.
2. Generated web GraphQL types and operation document.
3. Minimal memory routes.
4. React Flow graph page with filters, pan/zoom, selection, and side panel.
5. Tests and validation.

Future follow-ups:

- Full `/memory` list view.
- Review queue and backed mutation actions.
- Provenance-node toggle for forensic inspection.
- Context packet overlays when retrieval packets are consistently populated.
- Larger-graph virtualization or WebGL engine if real corpora outgrow React
  Flow.
