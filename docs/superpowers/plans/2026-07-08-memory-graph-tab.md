# Memory Graph Tab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore `/memory` as a top-level graph view of Supermemory memories about the local human.

**Architecture:** Noema Core owns all Supermemory traffic. The web UI queries GraphQL, GraphQL resolves the configured Supermemory connection, the Supermemory client fetches graph documents, and the frontend renders `@supermemory/memory-graph`.

**Tech Stack:** Rust, async-graphql, reqwest, React 19, Apollo Client, TanStack Router, StyleX, `@supermemory/memory-graph`.

## Global Constraints

- Work on `main`.
- Preserve unrelated dirty worktree changes.
- No migration path or backward compatibility layer.
- No local graph database.
- No direct web UI connection to Supermemory.
- No memory markers and no `/remember`.
- Do not modify or bypass `sccache` or `CARGO_BUILD_RUSTC_WRAPPER`.
- Do not write new UI tests unless an existing test must be updated for the route model.

---

## File Structure

- Modify `crates/noema-core/src/supermemory/client.rs`: add graph request/response types and the `/v3/documents/documents` client method.
- Modify `crates/noema-core/src/supermemory/tests.rs`: add request-shape and response-decoding coverage for graph documents.
- Modify `crates/noema-core/src/graphql/memory.rs`: add GraphQL types and a `memory_graph` resolver that resolves managed or external Supermemory connections.
- Modify `crates/noema-core/src/graphql/schema.rs`: expose the query and add resolver tests.
- Modify `crates/noema-core/web/src/graphql/operations.ts`: add the `MemoryGraph` query.
- Create `crates/noema-core/web/src/pages/MemoryPage.tsx`: query GraphQL and render the graph.
- Modify `crates/noema-core/web/src/routes/memory.tsx`: render `MemoryPage`.
- Delete `crates/noema-core/web/src/routes/memory/index.tsx`: stop redirecting `/memory/` to settings.
- Modify `crates/noema-core/web/src/app/routes.ts`: make `memory` a first-class non-settings app route.
- Modify `crates/noema-core/web/src/app/routes.test.ts`: update route expectations.
- Modify `crates/noema-core/web/src/components/shell/shellNavigation.ts`: restore top-level Memory navigation.
- Modify `crates/noema-core/web/package.json` and `crates/noema-core/web/bun.lock`: add `@supermemory/memory-graph`.
- Regenerate `crates/noema-core/web/src/generated/schema.graphql`, `crates/noema-core/web/src/generated/graphql.ts`, and `crates/noema-core/web/src/routeTree.gen.ts`.

---

### Task 1: Supermemory Graph Client Boundary

**Files:**
- Modify: `crates/noema-core/src/supermemory/client.rs`
- Modify: `crates/noema-core/src/supermemory/tests.rs`

**Interfaces:**
- Consumes: `SupermemoryClient::new(base_url, api_key)`.
- Produces:
  - `SupermemoryClient::list_memory_graph_documents(request: SupermemoryGraphDocumentsRequest) -> Result<SupermemoryGraphDocumentsResponse, SupermemoryClientError>`
  - `SupermemoryGraphDocumentsRequest { container_tag: String, page: u32, limit: u32 }`
  - `SupermemoryGraphDocument` and `SupermemoryGraphMemoryEntry` fields matching the Memory Graph component.

- [ ] **Step 1: Write the failing client test**

Add this test to `crates/noema-core/src/supermemory/tests.rs`:

```rust
#[tokio::test]
async fn supermemory_graph_documents_posts_v3_documents_documents() {
    let server = FakeSupermemoryServer::start(
        "/v3/documents/documents",
        serde_json::json!({
            "documents": [{
                "id": "doc_1",
                "customId": "human-profile",
                "title": "Human profile",
                "content": "Kevin likes local-first tools",
                "summary": "Preference summary",
                "url": null,
                "source": "noema",
                "type": "note",
                "status": "done",
                "metadata": {"scope": "human"},
                "createdAt": "2026-07-08T00:00:00.000Z",
                "updatedAt": "2026-07-08T00:01:00.000Z",
                "memoryEntries": [{
                    "id": "mem_1",
                    "documentId": "doc_1",
                    "content": "Kevin prefers local-first tools",
                    "summary": "Local-first preference",
                    "title": "Preference",
                    "type": "fact",
                    "metadata": {"confidence": 0.9},
                    "createdAt": "2026-07-08T00:00:30.000Z",
                    "updatedAt": "2026-07-08T00:01:00.000Z",
                    "spaceContainerTag": "human:local",
                    "relation": "extends",
                    "isLatest": true,
                    "spaceId": "human:local"
                }]
            }],
            "pagination": {"page": 1, "limit": 25, "hasMore": true, "total": 42}
        }),
    )
    .await;
    let client = crate::SupermemoryClient::new(server.base_url(), Some("sm_test".to_string()));

    let response = client
        .list_memory_graph_documents(crate::SupermemoryGraphDocumentsRequest {
            container_tag: "human:local".to_string(),
            page: 1,
            limit: 25,
        })
        .await
        .expect("graph documents");

    assert_eq!(response.documents[0].id, "doc_1");
    assert_eq!(response.documents[0].memory_entries[0].id, "mem_1");
    assert_eq!(response.pagination.has_more, Some(true));
    assert_eq!(
        server.last_authorization().await.as_deref(),
        Some("Bearer sm_test")
    );
    let request_body = server.last_body_json().await;
    assert_eq!(request_body["containerTag"], "human:local");
    assert_eq!(request_body["containerTags"], serde_json::json!(["human:local"]));
    assert_eq!(request_body["page"], 1);
    assert_eq!(request_body["limit"], 25);
    assert_eq!(request_body["sort"], "createdAt");
    assert_eq!(request_body["order"], "desc");
}
```

- [ ] **Step 2: Run the focused failing test**

Run:

```bash
cargo test -p noema-core supermemory_graph_documents_posts_v3_documents_documents --no-fail-fast
```

Expected: fails because `SupermemoryGraphDocumentsRequest` and `list_memory_graph_documents` do not exist.

- [ ] **Step 3: Implement the client method and types**

In `client.rs`, add `list_memory_graph_documents` beside the existing client methods. It should `POST {base_url}/v3/documents/documents`, include both `containerTag` and `containerTags`, include `page`, `limit`, `sort: "createdAt"`, `order: "desc"`, and apply bearer auth when configured.

- [ ] **Step 4: Run the focused passing test**

Run:

```bash
cargo test -p noema-core supermemory_graph_documents_posts_v3_documents_documents --no-fail-fast
```

Expected: pass.

---

### Task 2: GraphQL Memory Graph Proxy

**Files:**
- Modify: `crates/noema-core/src/graphql/memory.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`

**Interfaces:**
- Consumes: `SupermemoryClient::list_memory_graph_documents`.
- Produces:
  - Query: `memoryGraph(input: MemoryGraphInput): MemoryGraph!`
  - `MemoryGraph.status: MemoryServiceStatus!`
  - `MemoryGraph.documents: [MemoryGraphDocument!]!`
  - `MemoryGraph.pageInfo: MemoryGraphPageInfo!`

- [ ] **Step 1: Write the unavailable GraphQL test**

Add a schema test that executes:

```graphql
{
  memoryGraph(input: { page: 1, limit: 25 }) {
    status { status lastErrorCode }
    documents { id }
    pageInfo { page limit hasMore }
  }
}
```

Expected data: `status.status == "UNAVAILABLE"`, `lastErrorCode == "supermemory_unavailable"`, no documents, `hasMore == false`.

- [ ] **Step 2: Write the available GraphQL test**

Add a schema test with a local fake Supermemory listener and external memory settings saved to its base URL. Execute the same query and assert that document id `doc_1` and memory entry id `mem_1` are returned.

- [ ] **Step 3: Run focused failing GraphQL tests**

Run:

```bash
cargo test -p noema-core memory_graph --no-fail-fast
```

Expected: fails because the GraphQL query and types do not exist.

- [ ] **Step 4: Implement GraphQL types and resolver**

Add the input/object types to `memory.rs`. The resolver should:

- Read `memory_service_settings`.
- For managed mode, use `state.supermemory_connection()`.
- For external mode, build `SupermemoryConnection::new(base_url, None)` from settings.
- Use `human:local` as the fixed `container_tag`.
- Clamp input limit to a sane range such as `1..=100`.
- Return structured unavailable data instead of throwing for missing Supermemory.
- Convert Supermemory client errors through `sanitized_code` and `sanitized_message`.

- [ ] **Step 5: Expose the query in `QueryRoot`**

Import `GraphqlMemoryGraph` and `GraphqlMemoryGraphInput`, then add:

```rust
async fn memory_graph(
    &self,
    ctx: &Context<'_>,
    input: Option<GraphqlMemoryGraphInput>,
) -> Result<GraphqlMemoryGraph> {
    let state = ctx.data_unchecked::<GraphqlState>();
    memory::memory_graph(state, input.unwrap_or_default()).await
}
```

- [ ] **Step 6: Run focused passing GraphQL tests**

Run:

```bash
cargo test -p noema-core memory_graph --no-fail-fast
```

Expected: pass.

---

### Task 3: Frontend Route, Navigation, and Graph Page

**Files:**
- Modify: `crates/noema-core/web/package.json`
- Modify: `crates/noema-core/web/bun.lock`
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Create: `crates/noema-core/web/src/pages/MemoryPage.tsx`
- Modify: `crates/noema-core/web/src/routes/memory.tsx`
- Delete: `crates/noema-core/web/src/routes/memory/index.tsx`
- Modify: `crates/noema-core/web/src/app/routes.ts`
- Modify: `crates/noema-core/web/src/app/routes.test.ts`
- Modify: `crates/noema-core/web/src/components/shell/shellNavigation.ts`

**Interfaces:**
- Consumes: GraphQL `MemoryGraph` generated operation.
- Produces: a top-level `/memory` page that renders `MemoryGraph`.

- [ ] **Step 1: Add the dependency**

Run:

```bash
cd crates/noema-core/web && bun add @supermemory/memory-graph
```

Expected: `package.json` and `bun.lock` update.

- [ ] **Step 2: Update the route-model test first**

Change the existing `/memory` route test to expect `{ kind: "memory" }`, and assert `pathForRoute({ kind: "memory" }) === "/memory"`.

- [ ] **Step 3: Run the focused failing frontend test**

Run:

```bash
cd crates/noema-core/web && bun test src/app/routes.test.ts
```

Expected: fail because `AppRoute` has no memory kind yet.

- [ ] **Step 4: Add the route model and shell nav**

Update `AppRoute`, `NonSettingsAppRoute`, `routeFromPathname`, `pathForRoute`, `activeL0ItemId`, `shellMenuLevelForRoute`, and `breadcrumbForRoute` so top-level Memory appears beside Home and is selected on `/memory`.

- [ ] **Step 5: Add the GraphQL operation**

Add `MemoryGraphDocument` to `operations.ts`, selecting `status`, `documents`, `memoryEntries`, and `pageInfo` fields needed by the graph component.

- [ ] **Step 6: Build the Memory page**

Create `MemoryPage.tsx` using Apollo `useQuery` for the first page and `fetchMore` for pagination. Pass `documents`, `isLoading`, `error`, `hasMore`, `loadMoreDocuments`, `totalLoaded`, and `showSpacesSelector={false}` to `@supermemory/memory-graph`.

- [ ] **Step 7: Replace redirect routes**

Make `routes/memory.tsx` render `<MemoryPage />` and delete `routes/memory/index.tsx`.

- [ ] **Step 8: Run the focused passing frontend route test**

Run:

```bash
cd crates/noema-core/web && bun test src/app/routes.test.ts
```

Expected: pass.

---

### Task 4: Generated Artifacts and Validation

**Files:**
- Modify: generated GraphQL and route artifacts.
- Modify: `docs/context/current.md` with a concise durable summary.

- [ ] **Step 1: Regenerate frontend artifacts**

Run:

```bash
cd crates/noema-core/web && bun run gen:types && bun run gen:routes
```

Expected: schema, GraphQL operation types, and route tree update.

- [ ] **Step 2: Format**

Run:

```bash
cargo fmt --all --check
```

Expected: pass. If it fails for formatting, run `cargo fmt --all`, then re-run the check.

- [ ] **Step 3: Run frontend lint**

Run:

```bash
cd crates/noema-core/web && bun run lint
```

Expected: pass.

- [ ] **Step 4: Run Rust validation**

Run:

```bash
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all pass.

- [ ] **Step 5: Update durable project context**

Append a short note to `docs/context/current.md` that `/memory` is a top-level Supermemory graph page proxied through Noema Core GraphQL.

- [ ] **Step 6: Inspect and commit**

Run:

```bash
git status --short --branch
git diff --check
git diff --stat
git add crates/noema-core/src/supermemory/client.rs crates/noema-core/src/supermemory/tests.rs crates/noema-core/src/graphql/memory.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/web/package.json crates/noema-core/web/bun.lock crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/pages/MemoryPage.tsx crates/noema-core/web/src/routes/memory.tsx crates/noema-core/web/src/routes/memory/index.tsx crates/noema-core/web/src/app/routes.ts crates/noema-core/web/src/app/routes.test.ts crates/noema-core/web/src/components/shell/shellNavigation.ts crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts crates/noema-core/web/src/routeTree.gen.ts docs/context/current.md
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: add memory graph tab"
```

Expected: one implementation commit after validation.

---

## Self-Review

- Spec coverage: the plan covers top-level `/memory`, Settings separation, Noema Core proxying, human container tag, official graph component, generated artifacts, validation, and no direct browser-to-Supermemory traffic.
- Placeholder scan: no `TBD`, `TODO`, `implement later`, or vague edge-case placeholders remain.
- Type consistency: the client `SupermemoryGraphDocumentsRequest` feeds GraphQL `GraphqlMemoryGraphInput`, and the generated `MemoryGraphDocument` operation feeds the React `MemoryPage`.
