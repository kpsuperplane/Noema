# Memory Graph Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a human-facing `/memory/graph` web page that renders accessible memory claims as a pannable, zoomable entity graph with claim detail and evidence drill-in.

**Architecture:** Add a bounded `memoryGraph(input:)` GraphQL read model backed by the embedded SurrealDB store, then consume it from the React web UI. The graph canvas uses React Flow; the backend owns access, status filtering, limits, and list-level redaction semantics while the frontend owns layout, route state, filters, selection, and detail fetches.

**Tech Stack:** Rust, async-graphql, SurrealDB store APIs, React 19, Apollo Client, TypeScript, Tailwind/shadcn UI primitives, `@xyflow/react`, `d3-force`, Bun.

---

## Scope Check

This plan covers one cohesive feature slice: a bounded memory graph read model plus the first web page that consumes it. It intentionally does not build the full memory list, review queue, mutation actions, provenance-node toggle, or context-packet overlays.

## File Structure

Backend files:

- `crates/noema-core/src/store/claims.rs`: internal graph projection types, `MemoryGraphFilter`, and `NoemaStore::memory_graph`.
- `crates/noema-core/src/store.rs`: public store exports for graph projection types.
- `crates/noema-core/src/lib.rs`: crate-level exports for GraphQL and tests.
- `crates/noema-core/src/store/tests/claims.rs`: store tests for graph defaults, filters, limits, truncation, node/edge construction, and non-public sensitivity tracking.
- `crates/noema-core/src/graphql/types.rs`: GraphQL input/output types for `memoryGraph` plus graph redaction conversion.
- `crates/noema-core/src/graphql/schema.rs`: resolver, input validation, sensitivity parsing, schema tests.

Frontend files:

- `crates/noema-core/web/package.json` and `crates/noema-core/web/bun.lock`: React Flow and layout dependencies.
- `crates/noema-core/web/src/graphql/operations.ts`: `MemoryGraph` and graph detail operation documents.
- `crates/noema-core/web/src/generated/schema.graphql` and `crates/noema-core/web/src/generated/graphql.ts`: generated outputs from `bun run gen:types`.
- `crates/noema-core/web/src/routes.ts`: small route parser/history helper for `/`, `/memory`, and `/memory/graph`.
- `crates/noema-core/web/src/routes.test.ts`: route helper tests.
- `crates/noema-core/web/src/memoryGraph.ts`: pure graph normalization and display helpers.
- `crates/noema-core/web/src/memoryGraph.test.ts`: normalization tests.
- `crates/noema-core/web/src/memoryGraphLayout.ts`: deterministic frozen layout helper.
- `crates/noema-core/web/src/memoryGraphLayout.test.ts`: layout determinism tests.
- `crates/noema-core/web/src/pages/MemoryHomePage.tsx`: lightweight memory-management entry point.
- `crates/noema-core/web/src/pages/MemoryGraphPage.tsx`: query variables, route state, graph empty/loading/error states, selected claim state.
- `crates/noema-core/web/src/components/memory/MemoryGraphControls.tsx`: search, status chips, limit/truncation display.
- `crates/noema-core/web/src/components/memory/MemoryGraphCanvas.tsx`: React Flow canvas wrapper.
- `crates/noema-core/web/src/components/memory/MemoryGraphDetailPanel.tsx`: selected claim/evidence panel.
- `crates/noema-core/web/src/App.tsx`: route-aware shell integration while keeping chat as the default surface.
- `crates/noema-core/web/src/App.test.ts`: route-level smoke tests for route parsing and chat send behavior.

## Task 1: Store Memory Graph Read Model

**Files:**
- Modify: `crates/noema-core/src/store/claims.rs`
- Modify: `crates/noema-core/src/store.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Test: `crates/noema-core/src/store/tests/claims.rs`

- [ ] **Step 1: Write the failing default graph test**

Add this test near the existing claim inspection tests in `crates/noema-core/src/store/tests/claims.rs`:

```rust
#[tokio::test]
async fn memory_graph_defaults_to_current_statuses_and_reports_truncation() {
    let store = test_store().await;
    let train_item = create_source_item(&store, "Kevin likes trains.").await;
    let coffee_item = create_source_item(&store, "Kevin prefers coffee.").await;
    let tea_item = create_source_item(&store, "Kevin prefers tea.").await;
    let notebook_item = create_source_item(&store, "Kevin keeps a Noema notebook.").await;
    let archived_item = create_source_item(&store, "Kevin liked old buses.").await;

    let mut train = train_claim(train_item.item_id);
    train.status = ClaimStatus::Candidate;
    store
        .create_or_reinforce_claim(train)
        .await
        .expect("create candidate claim");

    let mut coffee = train_claim(coffee_item.item_id);
    coffee.object = EntityCandidate::concept("coffee", "coffee");
    coffee.predicate_id = "prefers".to_string();
    coffee.fact = "Kevin prefers coffee.".to_string();
    coffee.status = ClaimStatus::Active;
    store
        .create_or_reinforce_claim(coffee)
        .await
        .expect("create active claim");

    let mut tea = train_claim(tea_item.item_id);
    tea.object = EntityCandidate::concept("tea", "tea");
    tea.predicate_id = "prefers".to_string();
    tea.fact = "Kevin prefers tea.".to_string();
    tea.status = ClaimStatus::Confirmed;
    store
        .create_or_reinforce_claim(tea)
        .await
        .expect("create confirmed claim");

    let mut notebook = train_claim(notebook_item.item_id);
    notebook.object = EntityCandidate::concept("noema-notebook", "Noema notebook");
    notebook.predicate_id = "has_note".to_string();
    notebook.fact = "Kevin keeps a Noema notebook.".to_string();
    notebook.status = ClaimStatus::Active;
    store
        .create_or_reinforce_claim(notebook)
        .await
        .expect("create second active claim");

    let mut archived = train_claim(archived_item.item_id);
    archived.object = EntityCandidate::concept("old-buses", "old buses");
    archived.fact = "Kevin liked old buses.".to_string();
    archived.status = ClaimStatus::Archived;
    store
        .create_or_reinforce_claim(archived)
        .await
        .expect("create archived claim");

    let full_graph = store
        .memory_graph(crate::MemoryGraphFilter {
            query: None,
            statuses: None,
            predicate_id: None,
            sensitivity: None,
            limit: Some(10),
        })
        .await
        .expect("full memory graph");

    assert_eq!(full_graph.edges.len(), 4);
    assert!(
        full_graph
            .edges
            .iter()
            .any(|edge| edge.status == ClaimStatus::Candidate)
    );
    assert!(
        full_graph
            .edges
            .iter()
            .any(|edge| edge.status == ClaimStatus::Active)
    );
    assert!(
        full_graph
            .edges
            .iter()
            .any(|edge| edge.status == ClaimStatus::Confirmed)
    );
    assert!(
        full_graph
            .edges
            .iter()
            .all(|edge| edge.status != ClaimStatus::Archived)
    );

    let graph = store
        .memory_graph(crate::MemoryGraphFilter {
            query: None,
            statuses: None,
            predicate_id: None,
            sensitivity: None,
            limit: Some(3),
        })
        .await
        .expect("memory graph");

    assert_eq!(graph.summary.limit, 3);
    assert_eq!(graph.edges.len(), 3);
    assert_eq!(graph.summary.returned_claim_count, 3);
    assert_eq!(graph.summary.returned_node_count, graph.nodes.len());
    assert_eq!(graph.summary.truncated, true);
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.entity_id == "human:local" && node.claim_count >= 1)
    );
}
```

- [ ] **Step 2: Run the failing default graph test**

Run:

```bash
cargo test -p noema-core memory_graph_defaults_to_current_statuses_and_reports_truncation
```

Expected: FAIL because `MemoryGraphFilter` and `NoemaStore::memory_graph` do not exist.

- [ ] **Step 3: Add graph projection structs**

In `crates/noema-core/src/store/claims.rs`, add these structs after `MemoryClaimEvidence`:

```rust
/// Read-only filters for the bounded memory graph projection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryGraphFilter {
    /// Optional text query matched against facts, predicate labels, and entity names.
    pub query: Option<String>,
    /// Optional OR-list of claim lifecycle statuses.
    pub statuses: Option<Vec<ClaimStatus>>,
    /// Optional predicate id.
    pub predicate_id: Option<String>,
    /// Optional sensitivity filter.
    pub sensitivity: Option<Sensitivity>,
    /// Optional bounded result limit.
    pub limit: Option<usize>,
}

/// Bounded memory graph projection.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryGraph {
    /// Entity nodes touched by returned claim edges.
    pub nodes: Vec<MemoryGraphNode>,
    /// Claim edges returned by the bounded graph query.
    pub edges: Vec<MemoryGraphEdge>,
    /// Result metadata.
    pub summary: MemoryGraphSummary,
}

/// Entity node in the memory graph projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryGraphNode {
    /// Stable frontend node id.
    pub node_id: String,
    /// Canonical entity id.
    pub entity_id: String,
    /// Canonical entity label before GraphQL redaction.
    pub label: String,
    /// Entity type label.
    pub entity_type: String,
    /// Highest sensitivity among returned incident edges.
    pub max_sensitivity: Sensitivity,
    /// Count of returned edges touching this node.
    pub claim_count: i64,
}

/// Claim edge in the memory graph projection.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryGraphEdge {
    /// Stable claim id.
    pub claim_id: String,
    /// Source graph node id.
    pub source_node_id: String,
    /// Target graph node id. Objectless claims use a self-edge.
    pub target_node_id: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Predicate label.
    pub predicate_label: String,
    /// Human-readable fact before GraphQL redaction.
    pub fact: String,
    /// Claim status.
    pub status: ClaimStatus,
    /// Claim sensitivity.
    pub sensitivity: Sensitivity,
    /// Claim confidence.
    pub confidence: Option<f64>,
    /// Count of support evidence rows.
    pub evidence_count: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

/// Summary for a bounded memory graph projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryGraphSummary {
    /// Count of returned claim edges.
    pub returned_claim_count: i64,
    /// Count of returned entity nodes.
    pub returned_node_count: i64,
    /// Applied limit.
    pub limit: usize,
    /// True when more matching accessible claims exist than returned edges include.
    pub truncated: bool,
}
```

- [ ] **Step 4: Add the store graph implementation**

In `crates/noema-core/src/store/claims.rs`, add constants and helper functions near `clamp_claim_inspection_limit`:

```rust
const DEFAULT_MEMORY_GRAPH_LIMIT: usize = 150;
const MAX_MEMORY_GRAPH_LIMIT: usize = 500;

fn clamp_memory_graph_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_MEMORY_GRAPH_LIMIT)
        .clamp(1, MAX_MEMORY_GRAPH_LIMIT)
}

fn default_memory_graph_statuses() -> Vec<ClaimStatus> {
    vec![
        ClaimStatus::Candidate,
        ClaimStatus::Active,
        ClaimStatus::Confirmed,
    ]
}

fn entity_node_id(entity_id: &str) -> String {
    format!("entity:{entity_id}")
}

fn stricter_sensitivity(left: Sensitivity, right: Sensitivity) -> Sensitivity {
    if sensitivity_rank(right) > sensitivity_rank(left) {
        right
    } else {
        left
    }
}

fn sensitivity_rank(value: Sensitivity) -> u8 {
    match value {
        Sensitivity::Public => 0,
        Sensitivity::Normal => 1,
        Sensitivity::Private => 2,
        Sensitivity::Sensitive => 3,
        Sensitivity::Secret => 4,
    }
}
```

Still in `impl NoemaStore`, add this method after `list_claims`:

```rust
/// Return a bounded entity/claim graph for memory management.
///
/// # Errors
///
/// Returns [`StoreError`] when stored enum data is invalid or the embedded
/// store read fails.
pub async fn memory_graph(
    &self,
    filter: MemoryGraphFilter,
) -> Result<MemoryGraph, StoreError> {
    let predicates = self.inspection_predicates().await?;
    let entities = self.inspection_entities().await?;
    let evidence_counts = self.inspection_evidence_counts().await?;
    let query = filter
        .query
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .map(str::to_ascii_lowercase);
    let statuses = filter
        .statuses
        .filter(|statuses| !statuses.is_empty())
        .unwrap_or_else(default_memory_graph_statuses);
    let limit = clamp_memory_graph_limit(filter.limit);
    let fetch_limit = limit.saturating_add(1);
    let db_limit = query.is_none().then_some(fetch_limit);

    let mut sql = String::from(
        r#"
        SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact,
          status, sensitivity, confidence, created_at, updated_at
        FROM claims
        "#,
    );
    let mut clauses = vec!["status IN $statuses"];
    if filter.predicate_id.is_some() {
        clauses.push("predicate_id = $predicate_id");
    }
    if filter.sensitivity.is_some() {
        clauses.push("sensitivity = $sensitivity");
    }
    sql.push_str("WHERE ");
    sql.push_str(&clauses.join(" AND "));
    sql.push('\n');
    sql.push_str("ORDER BY created_at DESC, claim_id ASC\n");
    if db_limit.is_some() {
        sql.push_str("LIMIT $limit\n");
    }
    sql.push(';');

    let status_labels = statuses
        .into_iter()
        .map(|status| status.as_str().to_string())
        .collect::<Vec<_>>();
    let mut statement = self.db.query(sql).bind(("statuses", status_labels));
    if let Some(predicate_id) = filter.predicate_id {
        statement = statement.bind(("predicate_id", predicate_id));
    }
    if let Some(sensitivity) = filter.sensitivity {
        statement = statement.bind(("sensitivity", sensitivity_to_store(sensitivity).to_string()));
    }
    if let Some(db_limit) = db_limit {
        statement = statement.bind(("limit", db_limit));
    }

    let mut response = statement.await?;
    let rows: Vec<InspectionClaimRow> = response.take(0)?;
    let mut claims = Vec::new();

    for row in rows {
        let claim = memory_claim_record(row, &predicates, &entities, &evidence_counts)?;
        if let Some(query) = query.as_deref()
            && !claim.matches_query(query)
        {
            continue;
        }
        claims.push(claim);
        if claims.len() > limit {
            break;
        }
    }

    let truncated = claims.len() > limit;
    if truncated {
        claims.truncate(limit);
    }

    let mut nodes_by_id: HashMap<String, MemoryGraphNode> = HashMap::new();
    let mut edges = Vec::new();

    for claim in claims {
        let source_node_id = entity_node_id(&claim.subject_entity_id);
        upsert_memory_graph_node(
            &mut nodes_by_id,
            source_node_id.clone(),
            claim.subject_entity_id.clone(),
            claim.subject_entity_name.clone(),
            claim.subject_entity_type.clone(),
            claim.sensitivity,
        );

        let target_node_id = claim
            .object_entity_id
            .as_deref()
            .map(entity_node_id)
            .unwrap_or_else(|| source_node_id.clone());
        if let Some(object_entity_id) = claim.object_entity_id.as_ref() {
            upsert_memory_graph_node(
                &mut nodes_by_id,
                target_node_id.clone(),
                object_entity_id.clone(),
                claim
                    .object_entity_name
                    .clone()
                    .unwrap_or_else(|| object_entity_id.clone()),
                claim
                    .object_entity_type
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                claim.sensitivity,
            );
        }

        edges.push(MemoryGraphEdge {
            claim_id: claim.claim_id,
            source_node_id,
            target_node_id,
            predicate_id: claim.predicate_id,
            predicate_label: claim.predicate_label,
            fact: claim.fact,
            status: claim.status,
            sensitivity: claim.sensitivity,
            confidence: claim.confidence,
            evidence_count: claim.evidence_count,
            created_at: claim.created_at,
            updated_at: claim.updated_at,
        });
    }

    let mut nodes = nodes_by_id.into_values().collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    edges.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));

    Ok(MemoryGraph {
        summary: MemoryGraphSummary {
            returned_claim_count: i64::try_from(edges.len()).unwrap_or(i64::MAX),
            returned_node_count: i64::try_from(nodes.len()).unwrap_or(i64::MAX),
            limit,
            truncated,
        },
        nodes,
        edges,
    })
}
```

Add the helper below `memory_claim_record`:

```rust
fn upsert_memory_graph_node(
    nodes_by_id: &mut HashMap<String, MemoryGraphNode>,
    node_id: String,
    entity_id: String,
    label: String,
    entity_type: String,
    sensitivity: Sensitivity,
) {
    nodes_by_id
        .entry(node_id.clone())
        .and_modify(|node| {
            node.claim_count += 1;
            node.max_sensitivity = stricter_sensitivity(node.max_sensitivity, sensitivity);
        })
        .or_insert(MemoryGraphNode {
            node_id,
            entity_id,
            label,
            entity_type,
            max_sensitivity: sensitivity,
            claim_count: 1,
        });
}
```

- [ ] **Step 5: Export the new store types**

In `crates/noema-core/src/store.rs`, extend the existing `pub use claims` block with:

```rust
MemoryGraph, MemoryGraphEdge, MemoryGraphFilter, MemoryGraphNode, MemoryGraphSummary,
```

In `crates/noema-core/src/lib.rs`, extend the existing `pub use store` block with the same five names.

- [ ] **Step 6: Run the store graph test**

Run:

```bash
cargo test -p noema-core memory_graph_defaults_to_current_statuses_and_reports_truncation
```

Expected: PASS.

- [ ] **Step 7: Add focused filter/redaction metadata test**

Add this second test in `crates/noema-core/src/store/tests/claims.rs`:

```rust
#[tokio::test]
async fn memory_graph_filters_query_predicate_and_sensitivity() {
    let store = test_store().await;
    let train_item = create_source_item(&store, "Kevin likes trains.").await;
    let private_item = create_source_item(&store, "Garage code is 1234.").await;

    store
        .create_or_reinforce_claim(train_claim(train_item.item_id))
        .await
        .expect("create train claim");

    let mut private_note = train_claim(private_item.item_id);
    private_note.object = EntityCandidate::concept("garage-code", "Garage code");
    private_note.predicate_id = "has_note".to_string();
    private_note.fact = "Garage code is 1234.".to_string();
    private_note.sensitivity = Sensitivity::Private;
    private_note.status = ClaimStatus::Confirmed;
    store
        .create_or_reinforce_claim(private_note)
        .await
        .expect("create private claim");

    let graph = store
        .memory_graph(crate::MemoryGraphFilter {
            query: Some("garage".to_string()),
            statuses: Some(vec![ClaimStatus::Confirmed]),
            predicate_id: Some("has_note".to_string()),
            sensitivity: Some(Sensitivity::Private),
            limit: Some(150),
        })
        .await
        .expect("memory graph");

    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].predicate_id, "has_note");
    assert_eq!(graph.edges[0].sensitivity, Sensitivity::Private);
    assert_eq!(graph.edges[0].fact, "Garage code is 1234.");
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.max_sensitivity == Sensitivity::Private)
    );
}
```

- [ ] **Step 8: Run all claim store tests**

Run:

```bash
cargo test -p noema-core store::tests::claims
```

Expected: PASS.

- [ ] **Step 9: Commit backend store read model**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/store/claims.rs crates/noema-core/src/store.rs crates/noema-core/src/lib.rs crates/noema-core/src/store/tests/claims.rs
git commit -m "feat: add memory graph store read model"
```

## Task 2: GraphQL Memory Graph Query

**Files:**
- Modify: `crates/noema-core/src/graphql/types.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`

- [ ] **Step 1: Write the failing GraphQL schema and query tests**

In `crates/noema-core/src/graphql/schema.rs`, extend `schema_sdl_exposes_initial_noema_fields` with:

```rust
assert!(sdl.contains("memoryGraph"));
assert!(sdl.contains("GraphqlMemoryGraph"));
assert!(sdl.contains("GraphqlMemoryGraphInput"));
```

Add this test near `memory_claims_list_redacts_non_public_facts`:

```rust
#[tokio::test]
async fn memory_graph_query_returns_redacted_nodes_edges_and_summary() {
    use crate::{
        ActorRef, ClaimStatus, ConversationItemKind, ConversationItemStatus, EntityCandidate,
        EvidenceAuthority, EvidenceCandidate, NewClaimCandidate, NewConversation,
        NewConversationItem, NewConversationTurn, memory::Sensitivity,
        store::tests::test_store,
    };

    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({}),
        })
        .await
        .expect("turn");
    let item = store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id,
            turn_id: Some(turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("Garage code is 1234.".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("source item");
    let summary = store
        .create_or_reinforce_claim(NewClaimCandidate {
            subject: EntityCandidate::local_human(),
            object: EntityCandidate::concept("garage-code", "Garage code"),
            predicate_id: "has_note".to_string(),
            fact: "Garage code is 1234.".to_string(),
            sensitivity: Sensitivity::Private,
            status: ClaimStatus::Confirmed,
            confidence: Some(0.9),
            evidence: EvidenceCandidate {
                source_item_id: item.item_id,
                authority: EvidenceAuthority::ExplicitHumanStatement,
                excerpt: Some("Garage code is 1234.".to_string()),
            },
            retrieval_hints: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("claim");

    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let response = schema
        .execute(async_graphql::Request::new(
            r#"
            {
              memoryGraph(input: { statuses: ["confirmed"], limit: 150 }) {
                nodes { nodeId label redacted claimCount }
                edges { claimId fact factRedacted predicateLabel sensitivity }
                summary { returnedClaimCount returnedNodeCount limit truncated }
              }
            }
            "#,
        ))
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    assert_eq!(data["memoryGraph"]["summary"]["returnedClaimCount"], 1);
    assert_eq!(data["memoryGraph"]["summary"]["limit"], 150);
    assert_eq!(data["memoryGraph"]["summary"]["truncated"], false);
    assert_eq!(data["memoryGraph"]["edges"][0]["claimId"], summary.claim_id);
    assert_eq!(
        data["memoryGraph"]["edges"][0]["fact"],
        "[redacted; use memoryClaim(claimId) for detail]"
    );
    assert_eq!(data["memoryGraph"]["edges"][0]["factRedacted"], true);
    assert_eq!(data["memoryGraph"]["nodes"][0]["redacted"], true);
}
```

Add this validation test:

```rust
#[tokio::test]
async fn memory_graph_rejects_invalid_limit_and_status() {
    let schema = build_schema(GraphqlState::for_tests());
    let low_limit = schema
        .execute(async_graphql::Request::new(
            r#"
            { memoryGraph(input: { limit: 0 }) { summary { limit } } }
            "#,
        ))
        .await;
    assert_eq!(low_limit.errors.len(), 1);
    assert!(
        low_limit.errors[0]
            .message
            .contains("memoryGraph limit must be at least 1")
    );

    let bad_status = schema
        .execute(async_graphql::Request::new(
            r#"
            { memoryGraph(input: { statuses: ["sleepy"] }) { summary { limit } } }
            "#,
        ))
        .await;
    assert_eq!(bad_status.errors.len(), 1);
    assert!(
        bad_status.errors[0]
            .message
            .contains("unknown memory claim status: sleepy")
    );
}
```

- [ ] **Step 2: Run the failing GraphQL tests**

Run:

```bash
cargo test -p noema-core memory_graph
```

Expected: FAIL because GraphQL types and resolver do not exist.

- [ ] **Step 3: Add GraphQL graph input/output types**

In `crates/noema-core/src/graphql/types.rs`, extend the existing `use crate` list with:

```rust
MemoryGraph, MemoryGraphEdge, MemoryGraphNode, MemoryGraphSummary,
```

Add these types after `GraphqlMemoryClaimDetail`:

```rust
/// Input for the bounded memory graph query.
#[derive(Clone, Debug, Default, InputObject)]
pub struct GraphqlMemoryGraphInput {
    /// Optional text query.
    pub query: Option<String>,
    /// Optional OR-list of claim statuses.
    pub statuses: Option<Vec<String>>,
    /// Optional predicate id.
    pub predicate_id: Option<String>,
    /// Optional sensitivity filter.
    pub sensitivity: Option<String>,
    /// Optional result limit.
    pub limit: Option<i32>,
}

/// Bounded memory graph exposed to the web UI.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryGraph {
    /// Entity nodes touched by returned edges.
    pub nodes: Vec<GraphqlMemoryGraphNode>,
    /// Claim edges.
    pub edges: Vec<GraphqlMemoryGraphEdge>,
    /// Result summary.
    pub summary: GraphqlMemoryGraphSummary,
}

/// Entity node exposed in the memory graph.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryGraphNode {
    /// Stable frontend node id.
    pub node_id: String,
    /// Stable entity id.
    pub entity_id: String,
    /// Display label, redacted when needed.
    pub label: String,
    /// Entity type.
    pub entity_type: String,
    /// Whether the label was redacted at the GraphQL boundary.
    pub redacted: bool,
    /// Count of returned claim edges touching this node.
    pub claim_count: i64,
}

/// Claim edge exposed in the memory graph.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryGraphEdge {
    /// Stable claim id.
    pub claim_id: String,
    /// Source node id.
    pub source_node_id: String,
    /// Target node id.
    pub target_node_id: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Predicate label.
    pub predicate_label: String,
    /// Fact text, redacted when needed.
    pub fact: String,
    /// Whether fact text was redacted at the GraphQL boundary.
    pub fact_redacted: bool,
    /// Claim status.
    pub status: String,
    /// Claim sensitivity.
    pub sensitivity: String,
    /// Claim confidence.
    pub confidence: Option<f64>,
    /// Count of support evidence rows.
    pub evidence_count: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

/// Summary for the bounded memory graph.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryGraphSummary {
    /// Count of returned claim edges.
    pub returned_claim_count: i64,
    /// Count of returned entity nodes.
    pub returned_node_count: i64,
    /// Applied result limit.
    pub limit: i32,
    /// Whether more matching claims exist than were returned.
    pub truncated: bool,
}
```

Add conversion impls below those type definitions:

```rust
impl From<MemoryGraph> for GraphqlMemoryGraph {
    fn from(graph: MemoryGraph) -> Self {
        Self {
            nodes: graph.nodes.into_iter().map(Into::into).collect(),
            edges: graph.edges.into_iter().map(Into::into).collect(),
            summary: graph.summary.into(),
        }
    }
}

impl From<MemoryGraphNode> for GraphqlMemoryGraphNode {
    fn from(node: MemoryGraphNode) -> Self {
        let redacted = node.max_sensitivity != Sensitivity::Public;
        Self {
            node_id: node.node_id,
            entity_id: node.entity_id,
            label: graphql_list_display_name(node.label, node.max_sensitivity),
            entity_type: node.entity_type,
            redacted,
            claim_count: node.claim_count,
        }
    }
}

impl From<MemoryGraphEdge> for GraphqlMemoryGraphEdge {
    fn from(edge: MemoryGraphEdge) -> Self {
        let fact_redacted = edge.sensitivity != Sensitivity::Public;
        Self {
            claim_id: edge.claim_id,
            source_node_id: edge.source_node_id,
            target_node_id: edge.target_node_id,
            predicate_id: edge.predicate_id,
            predicate_label: edge.predicate_label,
            fact: graphql_list_fact(&edge.fact, edge.sensitivity),
            fact_redacted,
            status: claim_status_label(edge.status).to_string(),
            sensitivity: sensitivity_label(edge.sensitivity).to_string(),
            confidence: edge.confidence,
            evidence_count: edge.evidence_count,
            created_at: edge.created_at,
            updated_at: edge.updated_at,
        }
    }
}

impl From<MemoryGraphSummary> for GraphqlMemoryGraphSummary {
    fn from(summary: MemoryGraphSummary) -> Self {
        Self {
            returned_claim_count: summary.returned_claim_count,
            returned_node_count: summary.returned_node_count,
            limit: i32::try_from(summary.limit).unwrap_or(i32::MAX),
            truncated: summary.truncated,
        }
    }
}
```

- [ ] **Step 4: Add resolver validation helpers**

In `crates/noema-core/src/graphql/schema.rs`, extend the imported GraphQL types with:

```rust
GraphqlMemoryGraph, GraphqlMemoryGraphInput,
```

Add these helper functions next to `parse_graphql_claim_status`:

```rust
fn parse_graphql_sensitivity(value: &str) -> Result<crate::memory::Sensitivity> {
    match value {
        "public" => Ok(crate::memory::Sensitivity::Public),
        "normal" => Ok(crate::memory::Sensitivity::Normal),
        "private" => Ok(crate::memory::Sensitivity::Private),
        "sensitive" => Ok(crate::memory::Sensitivity::Sensitive),
        "secret" => Ok(crate::memory::Sensitivity::Secret),
        _ => Err(async_graphql::Error::new(format!(
            "unknown memory sensitivity: {value}"
        ))),
    }
}

fn parse_memory_graph_limit(limit: Option<i32>) -> Result<Option<usize>> {
    match limit {
        Some(value) if value < 1 => Err(async_graphql::Error::new(
            "memoryGraph limit must be at least 1",
        )),
        Some(value) if value > 500 => Err(async_graphql::Error::new(
            "memoryGraph limit must be at most 500",
        )),
        Some(value) => usize::try_from(value)
            .map(Some)
            .map_err(|_| async_graphql::Error::new("memoryGraph limit is too large")),
        None => Ok(None),
    }
}
```

- [ ] **Step 5: Add `memory_graph` resolver**

In the `impl QueryRoot` block in `crates/noema-core/src/graphql/schema.rs`, add this resolver after `memory_claim`:

```rust
/// Return a bounded memory graph for memory management.
async fn memory_graph(
    &self,
    ctx: &Context<'_>,
    input: Option<GraphqlMemoryGraphInput>,
) -> Result<GraphqlMemoryGraph> {
    let input = input.unwrap_or_default();
    let limit = parse_memory_graph_limit(input.limit)?;
    let statuses = input
        .statuses
        .map(|statuses| {
            if statuses.is_empty() {
                return Err(async_graphql::Error::new(
                    "memoryGraph statuses must not be empty",
                ));
            }
            statuses
                .iter()
                .map(|status| parse_graphql_claim_status(status))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    let sensitivity = input
        .sensitivity
        .as_deref()
        .map(parse_graphql_sensitivity)
        .transpose()?;
    let state = ctx.data_unchecked::<GraphqlState>();
    let graph = state
        .store()?
        .memory_graph(crate::MemoryGraphFilter {
            query: input.query,
            statuses,
            predicate_id: input.predicate_id,
            sensitivity,
            limit,
        })
        .await
        .map_err(graphql_error)?;

    Ok(graph.into())
}
```

- [ ] **Step 6: Run GraphQL tests**

Run:

```bash
cargo test -p noema-core memory_graph
```

Expected: PASS.

- [ ] **Step 7: Commit GraphQL query**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/graphql/types.rs crates/noema-core/src/graphql/schema.rs
git commit -m "feat: expose memory graph query"
```

## Task 3: Web Dependencies And Generated Operations

**Files:**
- Modify: `crates/noema-core/web/package.json`
- Modify: `crates/noema-core/web/bun.lock`
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Modify: `crates/noema-core/web/src/generated/schema.graphql`
- Modify: `crates/noema-core/web/src/generated/graphql.ts`

- [ ] **Step 1: Add graph rendering dependencies**

Run from `crates/noema-core/web`:

```bash
bun add @xyflow/react d3-force
bun add -d @types/d3-force
```

Expected: `package.json` and `bun.lock` update with `@xyflow/react`, `d3-force`, and `@types/d3-force`.

- [ ] **Step 2: Add GraphQL operation documents**

In `crates/noema-core/web/src/graphql/operations.ts`, add:

```ts
export const MemoryGraphDocument = gql`
  query MemoryGraph($input: GraphqlMemoryGraphInput) {
    memoryGraph(input: $input) {
      nodes {
        nodeId
        entityId
        label
        entityType
        redacted
        claimCount
      }
      edges {
        claimId
        sourceNodeId
        targetNodeId
        predicateId
        predicateLabel
        fact
        factRedacted
        status
        sensitivity
        confidence
        evidenceCount
        createdAt
        updatedAt
      }
      summary {
        returnedClaimCount
        returnedNodeCount
        limit
        truncated
      }
    }
  }
`;

export const MemoryGraphClaimDetailDocument = gql`
  query MemoryGraphClaimDetail($claimId: String!) {
    memoryClaim(claimId: $claimId) {
      claimId
      fact
      predicateId
      predicateLabel
      subjectEntityId
      subjectEntityName
      subjectEntityType
      objectEntityId
      objectEntityName
      objectEntityType
      status
      sensitivity
      confidence
      evidenceCount
      createdAt
      updatedAt
      evidence {
        evidenceId
        sourceItemId
        authority
        excerpt
        observedAt
        createdAt
      }
    }
  }
`;
```

- [ ] **Step 3: Generate schema and operation types**

Run from `crates/noema-core/web`:

```bash
bun run gen:types
```

Expected: generated schema includes `memoryGraph`, `GraphqlMemoryGraphInput`, `GraphqlMemoryGraphNode`, `GraphqlMemoryGraphEdge`, and `GraphqlMemoryGraphSummary`. Generated TypeScript includes `MemoryGraphDocument` and `MemoryGraphClaimDetailDocument`.

- [ ] **Step 4: Commit dependencies and generated operation types**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/package.json crates/noema-core/web/bun.lock crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts
git commit -m "feat: add memory graph web operations"
```

## Task 4: Route Helpers And Memory Route Shell

**Files:**
- Create: `crates/noema-core/web/src/routes.ts`
- Create: `crates/noema-core/web/src/routes.test.ts`
- Create: `crates/noema-core/web/src/pages/MemoryHomePage.tsx`
- Modify: `crates/noema-core/web/src/App.tsx`
- Modify: `crates/noema-core/web/src/App.test.ts`

- [ ] **Step 1: Write route helper tests**

Create `crates/noema-core/web/src/routes.test.ts`:

```ts
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { pathForRoute, routeFromPathname } from "./routes";

describe("routeFromPathname", () => {
  test("recognizes chat and memory routes", () => {
    assert.deepEqual(routeFromPathname("/"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/chat"), { kind: "chat" });
    assert.deepEqual(routeFromPathname("/memory"), { kind: "memory_home" });
    assert.deepEqual(routeFromPathname("/memory/graph"), { kind: "memory_graph" });
  });

  test("preserves unknown routes for not found states", () => {
    assert.deepEqual(routeFromPathname("/memory/nope"), {
      kind: "not_found",
      path: "/memory/nope"
    });
  });
});

describe("pathForRoute", () => {
  test("returns canonical paths", () => {
    assert.equal(pathForRoute({ kind: "chat" }), "/");
    assert.equal(pathForRoute({ kind: "memory_home" }), "/memory");
    assert.equal(pathForRoute({ kind: "memory_graph" }), "/memory/graph");
  });
});
```

- [ ] **Step 2: Run the failing route tests**

Run from `crates/noema-core/web`:

```bash
bun test src/routes.test.ts
```

Expected: FAIL because `src/routes.ts` does not exist.

- [ ] **Step 3: Implement route helpers**

Create `crates/noema-core/web/src/routes.ts`:

```ts
import React from "react";

export type AppRoute =
  | { kind: "chat" }
  | { kind: "memory_home" }
  | { kind: "memory_graph" }
  | { kind: "not_found"; path: string };

export function routeFromPathname(pathname: string): AppRoute {
  if (pathname === "/" || pathname === "/chat") {
    return { kind: "chat" };
  }
  if (pathname === "/memory") {
    return { kind: "memory_home" };
  }
  if (pathname === "/memory/graph") {
    return { kind: "memory_graph" };
  }
  return { kind: "not_found", path: pathname };
}

export function pathForRoute(route: Exclude<AppRoute, { kind: "not_found" }>): string {
  if (route.kind === "chat") {
    return "/";
  }
  if (route.kind === "memory_home") {
    return "/memory";
  }
  return "/memory/graph";
}

export function useBrowserRoute() {
  const [route, setRoute] = React.useState(() => routeFromPathname(window.location.pathname));

  React.useEffect(() => {
    const onPopState = () => setRoute(routeFromPathname(window.location.pathname));
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  const navigate = React.useCallback((nextRoute: Exclude<AppRoute, { kind: "not_found" }>) => {
    const nextPath = pathForRoute(nextRoute);
    window.history.pushState({}, "", nextPath);
    setRoute(routeFromPathname(nextPath));
  }, []);

  return { route, navigate };
}
```

- [ ] **Step 4: Create interim memory home page**

Create `crates/noema-core/web/src/pages/MemoryHomePage.tsx`:

```tsx
import { Network } from "lucide-react";
import { Button } from "@/components/ui/button";

export function MemoryHomePage({ onOpenGraph }: { onOpenGraph: () => void }) {
  return (
    <section className="mx-auto grid min-h-0 w-[min(960px,100%)] gap-5 px-6 py-7 max-[760px]:px-5">
      <div className="grid gap-2">
        <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">
          Memory
        </p>
        <h1 className="m-0 font-heading text-[32px] leading-[1.1] tracking-normal text-foreground">
          Memory management
        </h1>
        <p className="m-0 max-w-[640px] text-sm text-muted-foreground">
          Inspect what Noema remembers and how those memories connect.
        </p>
      </div>

      <div className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <div className="flex items-center gap-3">
          <Network className="size-5 text-[var(--text-accent)]" aria-hidden="true" />
          <div className="min-w-0">
            <strong className="block text-sm font-semibold">Memory Graph</strong>
            <span className="block text-sm text-muted-foreground">
              View entities and claim edges from accessible memories.
            </span>
          </div>
        </div>
        <Button type="button" className="w-fit" onClick={onOpenGraph}>
          Open graph
        </Button>
      </div>
    </section>
  );
}
```

- [ ] **Step 5: Make `App` route-aware without changing chat behavior**

In `crates/noema-core/web/src/App.tsx`:

1. Import the route hook and memory home:

```ts
import { MemoryHomePage } from "./pages/MemoryHomePage";
import { useBrowserRoute } from "./routes";
```

2. Inside `App`, add:

```ts
const { route, navigate } = useBrowserRoute();
const chatRoute = route.kind === "chat";
```

3. Change the conversation-starting effect guard from:

```ts
if (!onboarded || conversationId || startingConversationRef.current) {
  return;
}
```

to:

```ts
if (!chatRoute || !onboarded || conversationId || startingConversationRef.current) {
  return;
}
```

4. Before the final chat return block, add a memory-home route branch after onboarding succeeds:

```tsx
if (route.kind === "memory_home") {
  return (
    <main className="grid h-dvh min-h-screen grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-background">
      <AppHeader status={status} socketState={socketState} agentStatus={agentStatus} />
      <MemoryHomePage onOpenGraph={() => navigate({ kind: "memory_graph" })} />
    </main>
  );
}
```

5. For `route.kind === "memory_graph"`, temporarily render the same home page until Task 7 replaces it:

```tsx
if (route.kind === "memory_graph") {
  return (
    <main className="grid h-dvh min-h-screen grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-background">
      <AppHeader status={status} socketState={socketState} agentStatus={agentStatus} />
      <MemoryHomePage onOpenGraph={() => navigate({ kind: "memory_graph" })} />
    </main>
  );
}
```

- [ ] **Step 6: Run route tests and lint typecheck**

Run from `crates/noema-core/web`:

```bash
bun test src/routes.test.ts
bun run lint
```

Expected: PASS.

- [ ] **Step 7: Commit route shell**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src/routes.ts crates/noema-core/web/src/routes.test.ts crates/noema-core/web/src/pages/MemoryHomePage.tsx crates/noema-core/web/src/App.tsx crates/noema-core/web/src/App.test.ts
git commit -m "feat: add memory route shell"
```

## Task 5: Graph Normalization And Deterministic Layout

**Files:**
- Create: `crates/noema-core/web/src/memoryGraph.ts`
- Create: `crates/noema-core/web/src/memoryGraph.test.ts`
- Create: `crates/noema-core/web/src/memoryGraphLayout.ts`
- Create: `crates/noema-core/web/src/memoryGraphLayout.test.ts`

- [ ] **Step 1: Write normalization tests**

Create `crates/noema-core/web/src/memoryGraph.test.ts`:

```ts
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { graphStatusDefaults, graphStatusOptions, normalizeMemoryGraph } from "./memoryGraph";

describe("graphStatusDefaults", () => {
  test("defaults to candidate, active, and confirmed", () => {
    assert.deepEqual(graphStatusDefaults(), ["candidate", "active", "confirmed"]);
  });
});

describe("normalizeMemoryGraph", () => {
  test("keeps display-ready labels and creates edge selection labels", () => {
    const normalized = normalizeMemoryGraph({
      nodes: [
        {
          __typename: "GraphqlMemoryGraphNode",
          nodeId: "entity:human:local",
          entityId: "human:local",
          label: "[redacted; use memoryClaim(claimId) for detail]",
          entityType: "human",
          redacted: true,
          claimCount: 1
        }
      ],
      edges: [
        {
          __typename: "GraphqlMemoryGraphEdge",
          claimId: "claim:1",
          sourceNodeId: "entity:human:local",
          targetNodeId: "entity:human:local",
          predicateId: "has_note",
          predicateLabel: "has_note",
          fact: "[redacted; use memoryClaim(claimId) for detail]",
          factRedacted: true,
          status: "confirmed",
          sensitivity: "private",
          confidence: 0.9,
          evidenceCount: 2,
          createdAt: "2026-06-29T00:00:00Z",
          updatedAt: "2026-06-29T00:00:00Z"
        }
      ],
      summary: {
        __typename: "GraphqlMemoryGraphSummary",
        returnedClaimCount: 1,
        returnedNodeCount: 1,
        limit: 150,
        truncated: false
      }
    });

    assert.equal(normalized.nodes[0].label, "[redacted; use memoryClaim(claimId) for detail]");
    assert.equal(normalized.nodes[0].redacted, true);
    assert.equal(normalized.edges[0].label, "has_note");
    assert.equal(normalized.edges[0].claimId, "claim:1");
    assert.equal(normalized.summary.returnedClaimCount, 1);
  });
});

describe("graphStatusOptions", () => {
  test("includes all backed status filters", () => {
    assert.deepEqual(graphStatusOptions().map((status) => status.value), [
      "candidate",
      "active",
      "confirmed",
      "disputed",
      "superseded",
      "archived",
      "deleted"
    ]);
  });
});
```

- [ ] **Step 2: Run failing normalization tests**

Run from `crates/noema-core/web`:

```bash
bun test src/memoryGraph.test.ts
```

Expected: FAIL because `src/memoryGraph.ts` does not exist.

- [ ] **Step 3: Implement graph normalization helpers**

Create `crates/noema-core/web/src/memoryGraph.ts`:

```ts
import type { MemoryGraphQuery } from "./generated/graphql";

export type MemoryGraphResult = NonNullable<MemoryGraphQuery["memoryGraph"]>;
export type MemoryGraphNodeResult = MemoryGraphResult["nodes"][number];
export type MemoryGraphEdgeResult = MemoryGraphResult["edges"][number];

export type NormalizedMemoryGraphNode = MemoryGraphNodeResult;

export type NormalizedMemoryGraphEdge = MemoryGraphEdgeResult & {
  label: string;
};

export type NormalizedMemoryGraph = {
  nodes: NormalizedMemoryGraphNode[];
  edges: NormalizedMemoryGraphEdge[];
  summary: MemoryGraphResult["summary"];
};

export type GraphStatusOption = {
  value: string;
  label: string;
};

export function graphStatusDefaults() {
  return ["candidate", "active", "confirmed"];
}

export function graphStatusOptions(): GraphStatusOption[] {
  return [
    { value: "candidate", label: "Candidate" },
    { value: "active", label: "Active" },
    { value: "confirmed", label: "Confirmed" },
    { value: "disputed", label: "Disputed" },
    { value: "superseded", label: "Superseded" },
    { value: "archived", label: "Archived" },
    { value: "deleted", label: "Deleted" }
  ];
}

export function normalizeMemoryGraph(graph: MemoryGraphResult): NormalizedMemoryGraph {
  return {
    nodes: Array.from(graph.nodes).sort((left, right) => left.nodeId.localeCompare(right.nodeId)),
    edges: Array.from(graph.edges)
      .sort((left, right) => left.claimId.localeCompare(right.claimId))
      .map((edge) => Object.assign({}, edge, { label: edge.predicateLabel })),
    summary: graph.summary
  };
}

export function toggleStatus(current: string[], status: string) {
  if (current.includes(status)) {
    return current.filter((candidate) => candidate !== status);
  }
  return current.concat(status);
}
```

- [ ] **Step 4: Write layout determinism tests**

Create `crates/noema-core/web/src/memoryGraphLayout.test.ts`:

```ts
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { layoutMemoryGraph } from "./memoryGraphLayout";
import type { NormalizedMemoryGraph } from "./memoryGraph";

const graph: NormalizedMemoryGraph = {
  nodes: [
    {
      __typename: "GraphqlMemoryGraphNode",
      nodeId: "entity:b",
      entityId: "b",
      label: "B",
      entityType: "concept",
      redacted: false,
      claimCount: 1
    },
    {
      __typename: "GraphqlMemoryGraphNode",
      nodeId: "entity:a",
      entityId: "a",
      label: "A",
      entityType: "human",
      redacted: false,
      claimCount: 1
    }
  ],
  edges: [
    {
      __typename: "GraphqlMemoryGraphEdge",
      claimId: "claim:1",
      sourceNodeId: "entity:a",
      targetNodeId: "entity:b",
      predicateId: "likes",
      predicateLabel: "likes",
      fact: "A likes B.",
      factRedacted: false,
      status: "confirmed",
      sensitivity: "public",
      confidence: 0.9,
      evidenceCount: 1,
      createdAt: "2026-06-29T00:00:00Z",
      updatedAt: "2026-06-29T00:00:00Z",
      label: "likes"
    }
  ],
  summary: {
    __typename: "GraphqlMemoryGraphSummary",
    returnedClaimCount: 1,
    returnedNodeCount: 2,
    limit: 150,
    truncated: false
  }
};

describe("layoutMemoryGraph", () => {
  test("returns deterministic node positions and React Flow edges", () => {
    const first = layoutMemoryGraph(graph, { width: 800, height: 500 });
    const second = layoutMemoryGraph(graph, { width: 800, height: 500 });

    assert.deepEqual(first, second);
    assert.equal(first.nodes.length, 2);
    assert.equal(first.edges.length, 1);
    assert.equal(first.edges[0].id, "claim:1");
    assert.equal(first.edges[0].source, "entity:a");
    assert.equal(first.edges[0].target, "entity:b");
  });
});
```

- [ ] **Step 5: Implement deterministic layout helper**

Create `crates/noema-core/web/src/memoryGraphLayout.ts`:

```ts
import { forceCenter, forceLink, forceManyBody, forceSimulation } from "d3-force";
import type { Edge, Node } from "@xyflow/react";
import type { NormalizedMemoryGraph, NormalizedMemoryGraphEdge, NormalizedMemoryGraphNode } from "./memoryGraph";

export type MemoryGraphNodeData = NormalizedMemoryGraphNode;
export type MemoryGraphEdgeData = NormalizedMemoryGraphEdge;

type LayoutOptions = {
  width: number;
  height: number;
};

type SimulationNode = {
  id: string;
  x: number;
  y: number;
};

type SimulationLink = {
  source: string;
  target: string;
};

export function layoutMemoryGraph(
  graph: NormalizedMemoryGraph,
  options: LayoutOptions
): { nodes: Node<MemoryGraphNodeData>[]; edges: Edge<MemoryGraphEdgeData>[] } {
  const sortedNodes = Array.from(graph.nodes).sort((left, right) => left.nodeId.localeCompare(right.nodeId));
  const simulationNodes = sortedNodes.map((node, index) => initialSimulationNode(node.nodeId, index, sortedNodes.length, options));
  const simulationLinks = graph.edges.map((edge) => ({
    source: edge.sourceNodeId,
    target: edge.targetNodeId
  }));

  const simulation = forceSimulation<SimulationNode>(simulationNodes)
    .force(
      "link",
      forceLink<SimulationNode, SimulationLink>(simulationLinks)
        .id((node) => node.id)
        .distance(160)
    )
    .force("charge", forceManyBody().strength(-420))
    .force("center", forceCenter(options.width / 2, options.height / 2))
    .stop();

  for (let tick = 0; tick < 180; tick += 1) {
    simulation.tick();
  }

  const positions = new Map(simulationNodes.map((node) => [node.id, { x: node.x, y: node.y }]));
  return {
    nodes: sortedNodes.map((node) => ({
      id: node.nodeId,
      type: "memoryEntity",
      position: positions.get(node.nodeId) ?? { x: options.width / 2, y: options.height / 2 },
      data: node
    })),
    edges: graph.edges.map((edge) => ({
      id: edge.claimId,
      source: edge.sourceNodeId,
      target: edge.targetNodeId,
      type: "memoryClaim",
      label: edge.label,
      data: edge
    }))
  };
}

function initialSimulationNode(
  id: string,
  index: number,
  total: number,
  options: LayoutOptions
): SimulationNode {
  const radius = Math.max(120, Math.min(options.width, options.height) * 0.32);
  const angle = (Math.PI * 2 * index) / Math.max(1, total);
  return {
    id,
    x: options.width / 2 + Math.cos(angle) * radius,
    y: options.height / 2 + Math.sin(angle) * radius
  };
}
```

- [ ] **Step 6: Run helper tests**

Run from `crates/noema-core/web`:

```bash
bun test src/memoryGraph.test.ts src/memoryGraphLayout.test.ts
```

Expected: PASS.

- [ ] **Step 7: Commit graph helpers**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src/memoryGraph.ts crates/noema-core/web/src/memoryGraph.test.ts crates/noema-core/web/src/memoryGraphLayout.ts crates/noema-core/web/src/memoryGraphLayout.test.ts
git commit -m "feat: add memory graph layout helpers"
```

## Task 6: React Flow Canvas Components

**Files:**
- Create: `crates/noema-core/web/src/components/memory/MemoryGraphControls.tsx`
- Create: `crates/noema-core/web/src/components/memory/MemoryGraphCanvas.tsx`
- Create: `crates/noema-core/web/src/components/memory/MemoryGraphDetailPanel.tsx`

- [ ] **Step 1: Create graph controls**

Create `crates/noema-core/web/src/components/memory/MemoryGraphControls.tsx`:

```tsx
import { Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { graphStatusOptions, toggleStatus } from "@/memoryGraph";

export function MemoryGraphControls({
  query,
  statuses,
  limit,
  truncated,
  onQueryChange,
  onStatusesChange
}: {
  query: string;
  statuses: string[];
  limit: number;
  truncated: boolean;
  onQueryChange: (value: string) => void;
  onStatusesChange: (value: string[]) => void;
}) {
  return (
    <section className="grid gap-3 border-b border-[var(--border-subtle)] bg-white px-5 py-4" aria-label="Memory graph filters">
      <label className="flex min-h-10 items-center gap-2 rounded-md border border-[var(--border-subtle)] bg-[var(--surface-sunken)] px-3 text-sm">
        <Search className="size-4 text-muted-foreground" aria-hidden="true" />
        <input
          className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
          value={query}
          placeholder="Search memories"
          onChange={(event) => onQueryChange(event.target.value)}
        />
      </label>

      <div className="flex flex-wrap items-center gap-2">
        {graphStatusOptions().map((status) => {
          const active = statuses.includes(status.value);
          return (
            <Button
              key={status.value}
              type="button"
              variant={active ? "default" : "outline"}
              size="sm"
              onClick={() => onStatusesChange(toggleStatus(statuses, status.value))}
            >
              {status.label}
            </Button>
          );
        })}
        <span className="ml-auto text-xs text-muted-foreground">
          Limit {limit}
          {truncated ? " · truncated" : ""}
        </span>
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Create graph canvas with pan/zoom**

Create `crates/noema-core/web/src/components/memory/MemoryGraphCanvas.tsx`:

```tsx
import React from "react";
import {
  Background,
  Controls,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  type EdgeProps,
  type NodeProps
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { Edge, Node } from "@xyflow/react";
import type { MemoryGraphEdgeData, MemoryGraphNodeData } from "@/memoryGraphLayout";

function MemoryEntityNode({ data, selected }: NodeProps<Node<MemoryGraphNodeData>>) {
  return (
    <div
      className={[
        "min-w-[150px] max-w-[220px] rounded-md border bg-white px-3 py-2 shadow-sm",
        selected ? "border-[var(--pine-500)]" : "border-[var(--border-subtle)]"
      ].join(" ")}
    >
      <Handle type="target" position={Position.Left} className="opacity-0" />
      <strong className="block truncate text-sm font-semibold">{data.label}</strong>
      <span className="block truncate text-xs text-muted-foreground">
        {data.entityType} · {data.claimCount}
      </span>
      <Handle type="source" position={Position.Right} className="opacity-0" />
    </div>
  );
}

function MemoryClaimEdge(props: EdgeProps<Edge<MemoryGraphEdgeData>>) {
  const selected = props.selected;
  const stroke = selected ? "var(--pine-500)" : "rgba(23, 22, 15, 0.28)";
  return (
    <g>
      <path
        id={props.id}
        className="react-flow__edge-path"
        d={`M ${props.sourceX} ${props.sourceY} L ${props.targetX} ${props.targetY}`}
        stroke={stroke}
        strokeWidth={selected ? 2.5 : 1.5}
        fill="none"
      />
      <text className="fill-[var(--text-secondary)] text-[11px]">
        <textPath href={`#${props.id}`} startOffset="50%" textAnchor="middle">
          {props.label}
        </textPath>
      </text>
    </g>
  );
}

const nodeTypes = { memoryEntity: MemoryEntityNode };
const edgeTypes = { memoryClaim: MemoryClaimEdge };

export function MemoryGraphCanvas({
  nodes,
  edges,
  selectedClaimId,
  onSelectClaim
}: {
  nodes: Node<MemoryGraphNodeData>[];
  edges: Edge<MemoryGraphEdgeData>[];
  selectedClaimId: string | null;
  onSelectClaim: (claimId: string | null) => void;
}) {
  const selectedEdges = React.useMemo(
    () =>
      edges.map((edge) =>
        Object.assign({}, edge, {
          selected: edge.id === selectedClaimId
        })
      ),
    [edges, selectedClaimId]
  );

  return (
    <div className="h-full min-h-[420px] overflow-hidden bg-[var(--surface-sunken)]">
      <ReactFlow
        nodes={nodes}
        edges={selectedEdges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        fitView
        panOnDrag
        zoomOnScroll
        zoomOnPinch
        onEdgeClick={(_, edge) => onSelectClaim(edge.id)}
        onPaneClick={() => onSelectClaim(null)}
      >
        <Background />
        <Controls />
        <MiniMap pannable zoomable />
      </ReactFlow>
    </div>
  );
}
```

- [ ] **Step 3: Create selected claim detail panel**

Create `crates/noema-core/web/src/components/memory/MemoryGraphDetailPanel.tsx`:

```tsx
import { useQuery } from "@apollo/client/react";
import { MemoryGraphClaimDetailDocument } from "@/generated/graphql";
import type { NormalizedMemoryGraphEdge } from "@/memoryGraph";

export function MemoryGraphDetailPanel({ selectedEdge }: { selectedEdge: NormalizedMemoryGraphEdge | null }) {
  const detail = useQuery(MemoryGraphClaimDetailDocument, {
    variables: { claimId: selectedEdge?.claimId ?? "" },
    skip: !selectedEdge,
    fetchPolicy: "cache-and-network"
  });

  if (!selectedEdge) {
    return (
      <aside className="grid content-start gap-2 border-l border-[var(--border-subtle)] bg-white p-5">
        <h2 className="m-0 font-heading text-lg tracking-normal">Memory detail</h2>
        <p className="m-0 text-sm text-muted-foreground">Select a claim edge to inspect evidence.</p>
      </aside>
    );
  }

  const claim = detail.data?.memoryClaim;

  return (
    <aside className="grid content-start gap-4 border-l border-[var(--border-subtle)] bg-white p-5">
      <div className="grid gap-1">
        <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">
          {selectedEdge.status}
        </p>
        <h2 className="m-0 font-heading text-lg leading-snug tracking-normal">{selectedEdge.predicateLabel}</h2>
        <p className="m-0 text-sm text-muted-foreground">{claim?.fact ?? selectedEdge.fact}</p>
      </div>

      <dl className="grid grid-cols-2 gap-3 text-sm">
        <div>
          <dt className="text-xs text-muted-foreground">Sensitivity</dt>
          <dd className="m-0">{selectedEdge.sensitivity}</dd>
        </div>
        <div>
          <dt className="text-xs text-muted-foreground">Evidence</dt>
          <dd className="m-0">{selectedEdge.evidenceCount}</dd>
        </div>
      </dl>

      {detail.loading ? <p className="m-0 text-sm text-muted-foreground">Loading evidence…</p> : null}
      {detail.error ? <p className="m-0 text-sm text-destructive">{detail.error.message}</p> : null}

      {claim?.evidence.length ? (
        <div className="grid gap-2">
          <h3 className="m-0 text-sm font-semibold">Evidence</h3>
          {claim.evidence.map((evidence) => (
            <article key={evidence.evidenceId ?? `${evidence.sourceItemId}:${evidence.createdAt}`} className="rounded-md border border-[var(--border-subtle)] p-3">
              <strong className="block text-xs">{evidence.authority}</strong>
              {evidence.excerpt ? <p className="m-0 mt-1 text-sm text-muted-foreground">{evidence.excerpt}</p> : null}
              {evidence.sourceItemId ? <code className="mt-2 block text-[11px] text-muted-foreground">{evidence.sourceItemId}</code> : null}
            </article>
          ))}
        </div>
      ) : null}
    </aside>
  );
}
```

- [ ] **Step 4: Run TypeScript lint**

Run from `crates/noema-core/web`:

```bash
bun run lint
```

Expected: PASS after fixing any type import mismatch introduced by React Flow generics.

- [ ] **Step 5: Commit graph components**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src/components/memory/MemoryGraphControls.tsx crates/noema-core/web/src/components/memory/MemoryGraphCanvas.tsx crates/noema-core/web/src/components/memory/MemoryGraphDetailPanel.tsx
git commit -m "feat: add memory graph components"
```

## Task 7: Memory Graph Page Integration

**Files:**
- Create: `crates/noema-core/web/src/pages/MemoryGraphPage.tsx`
- Modify: `crates/noema-core/web/src/App.tsx`

- [ ] **Step 1: Create the graph page**

Create `crates/noema-core/web/src/pages/MemoryGraphPage.tsx`:

```tsx
import React from "react";
import { useQuery } from "@apollo/client/react";
import { MemoryGraphDocument } from "@/generated/graphql";
import { MemoryGraphCanvas } from "@/components/memory/MemoryGraphCanvas";
import { MemoryGraphControls } from "@/components/memory/MemoryGraphControls";
import { MemoryGraphDetailPanel } from "@/components/memory/MemoryGraphDetailPanel";
import { graphStatusDefaults, normalizeMemoryGraph } from "@/memoryGraph";
import { layoutMemoryGraph } from "@/memoryGraphLayout";

export function MemoryGraphPage() {
  const [query, setQuery] = React.useState("");
  const [statuses, setStatuses] = React.useState(graphStatusDefaults);
  const [selectedClaimId, setSelectedClaimId] = React.useState<string | null>(null);
  const result = useQuery(MemoryGraphDocument, {
    variables: {
      input: {
        query: query.trim() || null,
        statuses,
        limit: 150
      }
    },
    fetchPolicy: "cache-and-network"
  });

  const graph = result.data?.memoryGraph ? normalizeMemoryGraph(result.data.memoryGraph) : null;
  const laidOut = React.useMemo(
    () => (graph ? layoutMemoryGraph(graph, { width: 920, height: 620 }) : { nodes: [], edges: [] }),
    [graph]
  );
  const selectedEdge = graph?.edges.find((edge) => edge.claimId === selectedClaimId) ?? null;

  if (result.loading && !graph) {
    return (
      <section className="grid min-h-0 content-center px-6 text-center">
        <p className="m-0 text-sm text-muted-foreground">Loading memory graph…</p>
      </section>
    );
  }

  if (result.error) {
    return (
      <section className="grid min-h-0 content-center gap-3 px-6 text-center">
        <h1 className="m-0 font-heading text-2xl tracking-normal">Memory graph unavailable</h1>
        <p className="m-0 text-sm text-muted-foreground">{result.error.message}</p>
      </section>
    );
  }

  if (!graph || graph.edges.length === 0) {
    return (
      <section className="grid min-h-0 content-center gap-3 px-6 text-center">
        <h1 className="m-0 font-heading text-2xl tracking-normal">No memory graph yet</h1>
        <p className="m-0 text-sm text-muted-foreground">
          Start a chat or use “remember this” to create inspectable memory claims.
        </p>
      </section>
    );
  }

  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden" aria-label="Memory Graph">
      <MemoryGraphControls
        query={query}
        statuses={statuses}
        limit={graph.summary.limit}
        truncated={graph.summary.truncated}
        onQueryChange={setQuery}
        onStatusesChange={(next) => {
          setStatuses(next.length ? next : graphStatusDefaults());
          setSelectedClaimId(null);
        }}
      />
      <div className="grid min-h-0 grid-cols-[minmax(0,1fr)_340px] overflow-hidden max-[900px]:grid-cols-1 max-[900px]:grid-rows-[minmax(420px,1fr)_auto]">
        <MemoryGraphCanvas
          nodes={laidOut.nodes}
          edges={laidOut.edges}
          selectedClaimId={selectedClaimId}
          onSelectClaim={setSelectedClaimId}
        />
        <MemoryGraphDetailPanel selectedEdge={selectedEdge} />
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Wire `/memory/graph` to the graph page**

In `crates/noema-core/web/src/App.tsx`, import:

```ts
import { MemoryGraphPage } from "./pages/MemoryGraphPage";
```

Replace the temporary `route.kind === "memory_graph"` branch from Task 4 with:

```tsx
if (route.kind === "memory_graph") {
  return (
    <main className="grid h-dvh min-h-screen grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-background">
      <AppHeader status={status} socketState={socketState} agentStatus={agentStatus} />
      <MemoryGraphPage />
    </main>
  );
}
```

- [ ] **Step 3: Run frontend checks**

Run from `crates/noema-core/web`:

```bash
bun test src/routes.test.ts src/memoryGraph.test.ts src/memoryGraphLayout.test.ts
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 4: Commit graph page integration**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src/pages/MemoryGraphPage.tsx crates/noema-core/web/src/App.tsx
git commit -m "feat: add memory graph page"
```

## Task 8: Full Validation And Final Context Update

**Files:**
- Modify when implementation differs from the plan: `docs/context/current.md`

- [ ] **Step 1: Run backend validation**

Run from repository root:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: PASS. If any daemon/OpenAI provider test fails with local socket `PermissionDenied` under sandboxing, rerun the exact same command with socket permissions and record that distinction in the final summary.

- [ ] **Step 2: Run frontend validation**

Run from `crates/noema-core/web`:

```bash
bun run gen:types
bun test src/routes.test.ts src/memoryGraph.test.ts src/memoryGraphLayout.test.ts
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 3: Inspect changed file sizes**

Run:

```bash
wc -l crates/noema-core/web/src/App.tsx crates/noema-core/src/store/claims.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/src/graphql/types.rs
```

Expected: inspect any file over 750 lines. `claims.rs`, `schema.rs`, and `types.rs` are already near or above the project threshold, so include a short final note if this feature pushes them further and whether a split is warranted next.

- [ ] **Step 4: Update durable context if direction changed during implementation**

If implementation materially differs from the approved design, update `docs/context/current.md` with the final shape. If implementation follows the plan, no context edit is required.

- [ ] **Step 5: Run ship checklist**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected: no unstaged implementation changes before the final commit. Report any remaining untracked or unstaged files.

- [ ] **Step 6: Final commit for validation/context cleanup**

If Task 8 changed docs or generated files, commit them:

```bash
git add docs/context/current.md crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts
git commit -m "chore: validate memory graph page"
```

If there are no changes after validation, do not create an empty commit.
