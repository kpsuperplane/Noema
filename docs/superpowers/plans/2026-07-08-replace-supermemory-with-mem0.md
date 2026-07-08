# Replace Supermemory With Mem0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Noema's local Supermemory integration with a local Mem0-backed memory service that keeps chat response generation unblocked, uses Noema's configured memory model for extraction, avoids local graph database dependencies, and exposes memory search/settings through Noema Core.

**Architecture:** Do not run Mem0's official Docker REST stack, because current Mem0 self-host docs describe a FastAPI + Postgres/pgvector + Neo4j deployment. Instead, add a Noema-owned private Python sidecar that embeds Mem0 OSS `AsyncMemory`, stores vectors in local Chroma under `~/.noema/mem0/data`, uses FastEmbed for local embeddings, and calls Noema's existing OpenAI-compatible model proxy for extraction. Rust talks only to a small stable Noema sidecar API, so Mem0 API drift is isolated to the sidecar.

**Tech Stack:** Rust, Tokio, reqwest, Python FastAPI sidecar, Mem0 OSS `AsyncMemory`, Chroma local persistence, FastEmbed local ONNX embeddings, existing Noema model proxy, GraphQL, React/Astryx/StyleX.

## Global Constraints

- Work on `main` unless explicitly redirected.
- Preserve unrelated dirty worktree changes.
- Clean pre-V1 reset: rewrite schema/docs directly; do not build compatibility migrations unless explicitly requested.
- Do not use Docker/Compose for local development or runtime.
- Do not add or require a local graph database.
- Do not expose the Mem0 sidecar port outside Noema; bind a runtime-selected loopback port and keep it in memory.
- Memory ingest must run asynchronously and never block normal chat provider generation.
- Do not submit assistant responses, tool results, reasoning, notices, or empty user text as memory source.
- Keep memory ownership scoped to `human:local` for the current product slice.
- Follow existing Settings UI model-selection patterns.
- Run Rust unit tests only; do not run smoke or fixture tests unless explicitly requested.

---

## File Structure

- Create `crates/noema-core/mem0-sidecar/pyproject.toml`: pinned Python sidecar dependencies.
- Create `crates/noema-core/mem0-sidecar/noema_mem0_sidecar/app.py`: private FastAPI API wrapping Mem0 `AsyncMemory`.
- Create `crates/noema-core/mem0-sidecar/noema_mem0_sidecar/config.py`: sidecar environment parsing and Mem0 config construction.
- Create `crates/noema-core/src/mem0/client.rs`: Rust HTTP client for the Noema Mem0 sidecar API.
- Create `crates/noema-core/src/mem0/lifecycle.rs`: managed sidecar process startup/shutdown.
- Create `crates/noema-core/src/mem0/mod.rs`: module exports.
- Modify `crates/noema-core/src/supermemory/model_proxy.rs`: move or rename to a neutral memory model proxy in `crates/noema-core/src/memory_model_proxy.rs`.
- Modify `crates/noema-core/src/runtime_host.rs`: start Mem0 sidecar and memory model proxy instead of Supermemory lifecycle.
- Modify `crates/noema-core/src/daemon/runtime/actor.rs`: expose `Mem0Client` through the runtime actor.
- Modify `crates/noema-core/src/daemon/runtime/turn.rs`: enqueue Mem0 add-memory request after persisted user item.
- Modify `crates/noema-core/src/daemon/memory/tool.rs`: search Mem0 instead of Supermemory.
- Modify `crates/noema-core/src/graphql/memory.rs`: status/settings/graph/list backed by Mem0.
- Modify `crates/noema-core/src/graphql/runtime_state.rs` and `crates/noema-core/src/graphql/schema.rs`: rename Supermemory runtime state to memory service state.
- Modify `crates/noema-core/src/store/schema.rs`: rename auxiliary task id from `supermemory_extraction` to `memory_extraction`.
- Modify `crates/noema-core/src/store/memory_service.rs` and tests: keep GraphQL-facing mode shape but remove Supermemory names.
- Modify `crates/noema-core/src/paths.rs`: replace Supermemory paths with Mem0 paths.
- Modify `crates/noema-core/src/bin/noema_dev.rs`: remove Supermemory binary installation and ensure sidecar Python environment instead.
- Modify `crates/noema-core/web/src/components/settings/MemorySettingsPaneContent.tsx`: replace Supermemory copy with Mem0-neutral memory copy.
- Modify `crates/noema-core/web/src/components/settings/memorySettingsModel.ts`: replace Supermemory status labels.
- Modify `crates/noema-core/web/src/pages/MemoryPage.tsx`: remove `@supermemory/memory-graph` and render a Noema-native memory list grouped by provenance.
- Modify `crates/noema-core/web/src/graphql/operations.ts`: adjust memory graph/list operation if schema changes.
- Modify `crates/noema-core/web/package.json`: remove `@supermemory/memory-graph` dependency after the native memory page lands.
- Modify `docs/project.md` and `docs/context/current.md`: update durable memory direction.
- Delete `crates/noema-core/src/supermemory/*` after replacement.
- Delete `crates/noema-core/supermemory/supermemory-server` bundle artifacts after replacement.

---

### Task 1: Mem0 Sidecar Contract Spike

**Files:**
- Create: `crates/noema-core/mem0-sidecar/pyproject.toml`
- Create: `crates/noema-core/mem0-sidecar/noema_mem0_sidecar/__init__.py`
- Create: `crates/noema-core/mem0-sidecar/noema_mem0_sidecar/config.py`
- Create: `crates/noema-core/mem0-sidecar/noema_mem0_sidecar/app.py`
- Create: `crates/noema-core/mem0-sidecar/tests/test_app.py`

**Interfaces:**
- Consumes: environment variables provided by Rust lifecycle:
  - `NOEMA_MEM0_DATA_DIR`
  - `NOEMA_MEM0_PORT`
  - `NOEMA_MEMORY_OPENAI_BASE_URL`
  - `NOEMA_MEMORY_OPENAI_API_KEY`
  - `NOEMA_MEMORY_MODEL`
- Produces: private HTTP API:
  - `GET /health`
  - `POST /v1/memories/add`
  - `POST /v1/memories/search`
  - `GET /v1/memories`
  - `GET /v1/memories/{memory_id}/history`

- [ ] **Step 1: Add the failing sidecar tests**

```python
# crates/noema-core/mem0-sidecar/tests/test_app.py
from fastapi.testclient import TestClient

from noema_mem0_sidecar.app import create_app


class FakeMemory:
    def __init__(self):
        self.add_calls = []

    async def add(self, **kwargs):
        self.add_calls.append(kwargs)
        return {"results": [{"id": "mem_1", "memory": "The user likes planes."}]}

    async def search(self, **kwargs):
        return {"results": [{"id": "mem_1", "memory": "The user likes planes.", "score": 0.91}]}

    async def get_all(self, **kwargs):
        return {"results": [{"id": "mem_1", "memory": "The user likes planes."}]}

    async def history(self, memory_id):
        return {"memory_id": memory_id, "history": []}


def test_health_reports_ready():
    client = TestClient(create_app(memory=FakeMemory()))

    response = client.get("/health")

    assert response.status_code == 200
    assert response.json() == {"status": "ready"}


def test_add_submits_user_message_with_noema_metadata():
    memory = FakeMemory()
    client = TestClient(create_app(memory=memory))

    response = client.post(
        "/v1/memories/add",
        json={
            "messages": [{"role": "user", "content": "I love planes."}],
            "user_id": "human:local",
            "agent_id": "agent:local",
            "run_id": "conv:1",
            "metadata": {"noemaConversationId": "conv:1", "userItemId": "item:1"},
        },
    )

    assert response.status_code == 200
    assert response.json()["results"][0]["id"] == "mem_1"
    assert memory.add_calls == [
        {
            "messages": [{"role": "user", "content": "I love planes."}],
            "user_id": "human:local",
            "agent_id": "agent:local",
            "run_id": "conv:1",
            "metadata": {"noemaConversationId": "conv:1", "userItemId": "item:1"},
        }
    ]
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd crates/noema-core/mem0-sidecar
uv run pytest tests/test_app.py -q
```

Expected: FAIL because `pyproject.toml` and `noema_mem0_sidecar.app` do not exist.

- [ ] **Step 3: Add the sidecar implementation**

```toml
# crates/noema-core/mem0-sidecar/pyproject.toml
[project]
name = "noema-mem0-sidecar"
version = "0.1.0"
requires-python = ">=3.11"
dependencies = [
  "fastapi>=0.115,<1",
  "uvicorn[standard]>=0.30,<1",
  "mem0ai>=2,<3",
  "chromadb>=0.5,<1",
  "fastembed>=0.4,<1",
  "pydantic>=2,<3",
]

[dependency-groups]
dev = ["pytest>=8,<9", "httpx>=0.27,<1"]
```

```python
# crates/noema-core/mem0-sidecar/noema_mem0_sidecar/config.py
from __future__ import annotations

import os
from pathlib import Path

from mem0 import AsyncMemory
from mem0.configs.base import MemoryConfig


def build_memory_from_env() -> AsyncMemory:
    data_dir = Path(os.environ["NOEMA_MEM0_DATA_DIR"])
    data_dir.mkdir(parents=True, exist_ok=True)
    base_url = os.environ["NOEMA_MEMORY_OPENAI_BASE_URL"].rstrip("/")
    api_key = os.environ["NOEMA_MEMORY_OPENAI_API_KEY"]
    model = os.environ["NOEMA_MEMORY_MODEL"]

    config = MemoryConfig(
        llm={
            "provider": "openai",
            "config": {
                "api_key": api_key,
                "openai_base_url": base_url,
                "model": model,
            },
        },
        embedder={
            "provider": "fastembed",
            "config": {
                "model": "BAAI/bge-small-en-v1.5",
            },
        },
        vector_store={
            "provider": "chroma",
            "config": {
                "collection_name": "noema_human_local",
                "path": str(data_dir / "chroma"),
            },
        },
    )
    return AsyncMemory(config=config)
```

```python
# crates/noema-core/mem0-sidecar/noema_mem0_sidecar/app.py
from __future__ import annotations

from typing import Any, Literal

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field

from .config import build_memory_from_env


class Message(BaseModel):
    role: Literal["user"]
    content: str = Field(min_length=1)


class AddMemoryRequest(BaseModel):
    messages: list[Message]
    user_id: str
    agent_id: str | None = None
    run_id: str | None = None
    metadata: dict[str, Any] = Field(default_factory=dict)


class SearchMemoryRequest(BaseModel):
    query: str = Field(min_length=1)
    user_id: str
    agent_id: str | None = None
    run_id: str | None = None
    limit: int = Field(default=8, ge=1, le=100)


def create_app(memory: Any | None = None) -> FastAPI:
    app = FastAPI()
    app.state.memory = memory or build_memory_from_env()

    @app.get("/health")
    async def health() -> dict[str, str]:
        return {"status": "ready"}

    @app.post("/v1/memories/add")
    async def add_memory(request: AddMemoryRequest) -> Any:
        try:
            return await app.state.memory.add(
                messages=[message.model_dump() for message in request.messages],
                user_id=request.user_id,
                agent_id=request.agent_id,
                run_id=request.run_id,
                metadata=request.metadata,
            )
        except Exception as exc:
            raise HTTPException(status_code=500, detail=str(exc)) from exc

    @app.post("/v1/memories/search")
    async def search_memory(request: SearchMemoryRequest) -> Any:
        filters: dict[str, Any] = {"user_id": request.user_id}
        if request.agent_id:
            filters["agent_id"] = request.agent_id
        if request.run_id:
            filters["run_id"] = request.run_id
        try:
            return await app.state.memory.search(
                query=request.query,
                filters=filters,
                top_k=request.limit,
            )
        except Exception as exc:
            raise HTTPException(status_code=500, detail=str(exc)) from exc

    @app.get("/v1/memories")
    async def list_memories(user_id: str, limit: int = 100) -> Any:
        try:
            return await app.state.memory.get_all(filters={"user_id": user_id}, limit=limit)
        except Exception as exc:
            raise HTTPException(status_code=500, detail=str(exc)) from exc

    @app.get("/v1/memories/{memory_id}/history")
    async def history(memory_id: str) -> Any:
        try:
            return await app.state.memory.history(memory_id=memory_id)
        except Exception as exc:
            raise HTTPException(status_code=500, detail=str(exc)) from exc

    return app


app = create_app()
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cd crates/noema-core/mem0-sidecar
uv run pytest tests/test_app.py -q
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/mem0-sidecar
git commit -m "feat: add mem0 sidecar contract"
```

---

### Task 2: Rust Mem0 Client Boundary

**Files:**
- Create: `crates/noema-core/src/mem0/mod.rs`
- Create: `crates/noema-core/src/mem0/client.rs`
- Create: `crates/noema-core/src/mem0/tests.rs`
- Modify: `crates/noema-core/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 HTTP contract.
- Produces:
  - `Mem0Client::new(base_url: String, api_key: Option<String>) -> Self`
  - `Mem0Client::add_memory(request: Mem0AddMemoryRequest) -> Result<(), Mem0ClientError>`
  - `Mem0Client::search_memories(request: Mem0SearchRequest) -> Result<Mem0SearchResponse, Mem0ClientError>`
  - `Mem0Client::list_memories(request: Mem0ListMemoriesRequest) -> Result<Mem0ListMemoriesResponse, Mem0ClientError>`

- [ ] **Step 1: Write failing client tests**

Add tests mirroring existing `crates/noema-core/src/supermemory/tests.rs`, but expect:

```rust
assert_eq!(request_line, "POST /v1/memories/add HTTP/1.1");
assert_eq!(request_body["user_id"], "human:local");
assert_eq!(request_body["run_id"], "conv:local");
assert_eq!(request_body["messages"], serde_json::json!([
    {"role": "user", "content": "I love planes."}
]));
```

And search:

```rust
assert_eq!(request_line, "POST /v1/memories/search HTTP/1.1");
assert_eq!(request_body["query"], "planes");
assert_eq!(request_body["user_id"], "human:local");
assert_eq!(request_body["limit"], 8);
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p noema-core mem0_client_ --no-fail-fast
```

Expected: FAIL because module and types are missing.

- [ ] **Step 3: Implement client types**

Use serde rename rules matching the sidecar API:

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Mem0AddMemoryRequest {
    pub messages: Vec<Mem0Message>,
    pub user_id: String,
    pub agent_id: Option<String>,
    pub run_id: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mem0Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mem0SearchRequest {
    pub query: String,
    pub user_id: String,
    pub agent_id: Option<String>,
    pub run_id: Option<String>,
    pub limit: u16,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Mem0SearchResponse {
    #[serde(default)]
    pub results: Vec<Mem0Memory>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Mem0Memory {
    pub id: String,
    #[serde(default)]
    pub memory: Option<String>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p noema-core mem0_client_ --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/mem0 crates/noema-core/src/lib.rs
git commit -m "feat: add mem0 client boundary"
```

---

### Task 3: Managed Mem0 Lifecycle

**Files:**
- Create: `crates/noema-core/src/mem0/lifecycle.rs`
- Modify: `crates/noema-core/src/mem0/mod.rs`
- Modify: `crates/noema-core/src/paths.rs`
- Modify: `crates/noema-core/src/runtime_host.rs`
- Modify: `crates/noema-core/src/graphql/runtime_state.rs`
- Modify: `crates/noema-core/src/bin/noema_dev.rs`

**Interfaces:**
- Consumes: Task 1 sidecar app and Task 2 connection/client types.
- Produces:
  - `Mem0Lifecycle::start(paths, settings, model_proxy, system_errors) -> Result<Self, Mem0LifecycleError>`
  - `Mem0Lifecycle::connection(&self) -> Option<&Mem0Connection>`
  - `Mem0Lifecycle::shutdown(self) -> impl Future<Output = ()>`

- [ ] **Step 1: Write failing lifecycle tests**

Add tests equivalent to existing `SupermemoryLifecycle` tests:

```rust
#[tokio::test]
async fn managed_mem0_lifecycle_exports_private_sidecar_env() {
    let home = tempfile::tempdir().expect("temp");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let binary = write_sleeping_mem0_sidecar(home.path());
    let proxy = fake_memory_model_proxy().await;

    let lifecycle = crate::mem0::Mem0Lifecycle::start_with_command_for_test(
        &paths,
        binary,
        proxy,
    )
    .await
    .expect("lifecycle");

    assert!(lifecycle.connection().unwrap().base_url.starts_with("http://127.0.0.1:"));
    lifecycle.shutdown().await;
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p noema-core mem0_lifecycle --no-fail-fast
```

Expected: FAIL because lifecycle does not exist.

- [ ] **Step 3: Implement lifecycle**

Implementation requirements:

- Create `paths.mem0_dir()`, `paths.mem0_data_dir()`, and `paths.mem0_runtime_dir()`.
- Start sidecar with:

```rust
command
    .env("NOEMA_MEM0_DATA_DIR", paths.mem0_data_dir())
    .env("NOEMA_MEM0_PORT", port.to_string())
    .env("NOEMA_MEMORY_OPENAI_BASE_URL", proxy.openai_base_url())
    .env("NOEMA_MEMORY_OPENAI_API_KEY", proxy.api_key())
    .env("NOEMA_MEMORY_MODEL", proxy.model_profile())
    .stdin(std::process::Stdio::null())
    .kill_on_drop(true);
```

- Use `python -m uvicorn noema_mem0_sidecar.app:app --host 127.0.0.1 --port <port>` for development.
- Allow `NOEMA_MEM0_SIDECAR_COMMAND` as an override for tests and future packaging.
- Keep the port runtime-only.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p noema-core mem0_lifecycle --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/mem0 crates/noema-core/src/paths.rs crates/noema-core/src/runtime_host.rs crates/noema-core/src/graphql/runtime_state.rs crates/noema-core/src/bin/noema_dev.rs
git commit -m "feat: manage local mem0 sidecar"
```

---

### Task 4: Runtime Memory Observation Ingest

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/actor.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

**Interfaces:**
- Consumes: `Mem0Client`.
- Produces: asynchronous `Mem0AddMemoryRequest` submission after persisted `UserText` item.

- [ ] **Step 1: Update failing runtime tests**

Replace Supermemory request assertions with Mem0 assertions:

```rust
assert_eq!(body["user_id"], "human:local");
assert_eq!(body["agent_id"], "agent:local");
assert_eq!(body["run_id"], conversation_id);
assert_eq!(body["metadata"]["noemaConversationId"], conversation_id);
assert_eq!(body["metadata"]["turnId"].as_str().unwrap().starts_with("turn:"), true);
assert_eq!(body["metadata"]["userItemId"].as_str().unwrap().starts_with("item:"), true);
assert_eq!(
    body["messages"],
    serde_json::json!([{"role": "user", "content": "remember this turn"}])
);
```

Keep these tests:

- memory submits after user item persistence even if provider later fails
- slow memory ingest does not delay provider response
- two user messages in one conversation produce two add calls
- each payload includes only the current user text
- assistant responses are never submitted after provider completion

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p noema-core user_message_submits_memory_observation provider_failure_after_user_message_still_submits_memory_observation slow_supermemory_ingest_does_not_delay_provider_response memory_observation_uses_distinct_source_ids_and_only_current_user_text --no-fail-fast
```

Expected: FAIL because runtime still sends Supermemory `/v4/conversations` payloads.

- [ ] **Step 3: Implement Mem0 observation builder**

Replace `build_memory_observation_ingest_request` with:

```rust
fn build_memory_observation_add_request(
    conversation_id: &str,
    turn_id: &str,
    user_item_id: &str,
    user_text: &str,
) -> Option<crate::mem0::Mem0AddMemoryRequest> {
    if user_text.trim().is_empty() {
        return None;
    }

    Some(crate::mem0::Mem0AddMemoryRequest {
        messages: vec![crate::mem0::Mem0Message {
            role: "user".to_string(),
            content: user_text.to_string(),
        }],
        user_id: HUMAN_MEMORY_SCOPE_ID.to_string(),
        agent_id: Some("agent:local".to_string()),
        run_id: Some(conversation_id.to_string()),
        metadata: json!({
            "noemaConversationId": conversation_id,
            "turnId": turn_id,
            "userItemId": user_item_id,
            "sourceKind": "user_message",
        }),
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run the same focused tests. Rename `slow_supermemory_ingest_does_not_delay_provider_response` to `slow_memory_ingest_does_not_delay_provider_response`.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/daemon/runtime crates/noema-core/src/daemon/tests.rs
git commit -m "feat: submit user memory observations to mem0"
```

---

### Task 5: Search Memory Tool Backed By Mem0

**Files:**
- Modify: `crates/noema-core/src/daemon/memory/tool.rs`
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

**Interfaces:**
- Consumes: `Mem0Client::search_memories`.
- Produces: existing model-visible `search_memory` tool result shape, with raw Mem0 metadata sanitized away.

- [ ] **Step 1: Update failing search tests**

Change fake memory server search path to `/v1/memories/search` and response shape:

```json
{
  "results": [
    {
      "id": "mem_plane",
      "memory": "Kevin likes planes.",
      "score": 0.91,
      "metadata": {"sourceKind": "user_message"},
      "updated_at": "2026-07-08T12:00:00Z"
    }
  ]
}
```

- [ ] **Step 2: Run focused search tests**

Run:

```bash
cargo test -p noema-core search_memory_ --no-fail-fast
```

Expected: FAIL because search still calls Supermemory `/v4/search`.

- [ ] **Step 3: Implement Mem0 search adapter**

Map Mem0 fields:

- `memory` -> sanitized result text
- `score` -> result confidence/similarity
- `updated_at` -> result timestamp
- never expose raw `metadata`

- [ ] **Step 4: Run focused search tests**

Run:

```bash
cargo test -p noema-core search_memory_ --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/daemon/memory crates/noema-core/src/daemon/runtime/local_tools.rs crates/noema-core/src/daemon/tests.rs
git commit -m "feat: route memory search through mem0"
```

---

### Task 6: GraphQL Settings And Memory Listing

**Files:**
- Modify: `crates/noema-core/src/graphql/memory.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/src/graphql/runtime_state.rs`
- Modify: `crates/noema-core/src/store/memory_service.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/tests.rs`

**Interfaces:**
- Consumes: `Mem0Client::list_memories`.
- Produces GraphQL-compatible memory page. Keep `MemorySettings` and `MemoryServiceMode` names; remove Supermemory wording from descriptions and errors.

- [ ] **Step 1: Update failing GraphQL tests**

Expected status labels:

```rust
assert_eq!(status.last_error_code.as_deref(), Some("mem0_unavailable"));
```

Expected memory list synthetic document:

```rust
assert_eq!(document.id, "mem0:human:local");
assert_eq!(document.title.as_deref(), Some("Human memory"));
assert_eq!(document.memory_entries[0].content.as_deref(), Some("Kevin likes planes."));
```

- [ ] **Step 2: Run GraphQL memory tests**

Run:

```bash
cargo test -p noema-core memory_settings_ memory_graph_ save_memory_service_settings_ --no-fail-fast
```

Expected: FAIL on Supermemory wording, paths, and response shapes.

- [ ] **Step 3: Implement GraphQL adapter**

Keep `memoryGraph` as the frontend query for now, but synthesize one document per provenance group:

- `metadata.noemaConversationId` present: `document.id = "conversation:<id>"`
- otherwise: `document.id = "mem0:human:local"`
- one `MemoryGraphMemoryEntry` per Mem0 memory
- `parentMemoryId`, `rootMemoryId`, `memoryRelations`, and `spaceId` become `None` until Mem0 graph/entity data is explicitly modeled

- [ ] **Step 4: Rename auxiliary task id**

In `crates/noema-core/src/store/schema.rs`, change:

```sql
CHECK (task_id IN ('web_fetch_summarizer', 'tool_progress_audit', 'supermemory_extraction'))
```

to:

```sql
CHECK (task_id IN ('web_fetch_summarizer', 'tool_progress_audit', 'memory_extraction'))
```

- [ ] **Step 5: Run focused tests**

Run the same GraphQL/store tests.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/graphql crates/noema-core/src/store
git commit -m "feat: expose mem0 memory through graphql"
```

---

### Task 7: Replace Settings And Memory UI Copy

**Files:**
- Modify: `crates/noema-core/web/src/components/settings/MemorySettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/components/settings/memorySettingsModel.ts`
- Modify: `crates/noema-core/web/src/pages/SettingsPage.tsx`
- Modify: `crates/noema-core/web/src/generated/graphql.ts`
- Modify: `crates/noema-core/web/src/generated/schema.graphql`

**Interfaces:**
- Consumes: Task 6 GraphQL schema.
- Produces: same settings UI behavior with Mem0-neutral copy.

- [ ] **Step 1: Update UI copy**

Replace:

```tsx
Supermemory
```

with:

```tsx
Mem0
```

Replace:

```tsx
ariaLabel="Model settings for Supermemory extraction"
```

with:

```tsx
ariaLabel="Model settings for memory extraction"
```

Replace `memoryStatusLabel` strings:

```ts
return "Managed Mem0 is ready.";
return "Managed Mem0 is unavailable.";
return "Mem0 is ready.";
```

- [ ] **Step 2: Regenerate frontend GraphQL types**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: `src/generated/graphql.ts` and `src/generated/schema.graphql` update.

- [ ] **Step 3: Run TypeScript check for touched surface**

Run:

```bash
cd crates/noema-core/web
bun run typecheck
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/noema-core/web/src/components/settings crates/noema-core/web/src/pages/SettingsPage.tsx crates/noema-core/web/src/generated
git commit -m "feat: update memory settings for mem0"
```

---

### Task 8: Replace Supermemory Graph UI With Native Memory Page

**Files:**
- Modify: `crates/noema-core/web/src/pages/MemoryPage.tsx`
- Modify: `crates/noema-core/web/package.json`
- Modify: `crates/noema-core/web/bun.lock`

**Interfaces:**
- Consumes: `MemoryGraph` GraphQL query from Task 6.
- Produces: native list grouped by synthetic document/provenance.

- [ ] **Step 1: Replace graph component import**

Remove:

```tsx
import { MemoryGraph as SupermemoryGraph } from "@supermemory/memory-graph";
```

Render:

```tsx
<div data-slot="memory-list">
  {documents.map((document) => (
    <section key={document.id}>
      <h2>{document.title ?? "Memory source"}</h2>
      <ul>
        {document.memoryEntries.map((entry) => (
          <li key={entry.id}>{entry.content ?? entry.summary ?? entry.title}</li>
        ))}
      </ul>
    </section>
  ))}
</div>
```

- [ ] **Step 2: Remove dependency**

Run:

```bash
cd crates/noema-core/web
bun remove @supermemory/memory-graph
```

- [ ] **Step 3: Run frontend check**

Run:

```bash
cd crates/noema-core/web
bun run typecheck
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/noema-core/web/src/pages/MemoryPage.tsx crates/noema-core/web/package.json crates/noema-core/web/bun.lock
git commit -m "feat: replace supermemory graph with native memory page"
```

---

### Task 9: Remove Supermemory Lifecycle, Client, Bundle, And Names

**Files:**
- Delete: `crates/noema-core/src/supermemory/binary.rs`
- Delete: `crates/noema-core/src/supermemory/client.rs`
- Delete: `crates/noema-core/src/supermemory/config.rs`
- Delete: `crates/noema-core/src/supermemory/endpoint.rs`
- Delete: `crates/noema-core/src/supermemory/lifecycle.rs`
- Delete: `crates/noema-core/src/supermemory/tests.rs`
- Delete: `crates/noema-core/src/supermemory.rs`
- Delete: `crates/noema-core/supermemory/supermemory-server`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `crates/noema-core/src/runtime_host.rs`
- Modify: `crates/noema-core/src/bin/noema_dev.rs`
- Modify: all remaining Rust/Web/docs references returned by `rg -n "Supermemory|supermemory"`

**Interfaces:**
- Consumes: all previous tasks.
- Produces: repository with no Supermemory code dependency.

- [ ] **Step 1: Search references**

Run:

```bash
rg -n "Supermemory|supermemory" crates docs
```

Expected: only historical plan/spec references remain before deletion.

- [ ] **Step 2: Delete Supermemory module and bundle**

Use `apply_patch` or `git rm` for the files listed above.

- [ ] **Step 3: Fix exports and imports**

Update `crates/noema-core/src/lib.rs` to export `mem0` types instead of `supermemory` types.

- [ ] **Step 4: Run compile check**

Run:

```bash
cargo check -p noema-core
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates/noema-core
git commit -m "refactor: remove supermemory integration"
```

---

### Task 10: Documentation And Final Validation

**Files:**
- Modify: `docs/project.md`
- Modify: `docs/context/current.md`
- Modify: `docs/superpowers/plans/2026-07-08-replace-supermemory-with-mem0.md` if implementation discoveries require plan notes.

**Interfaces:**
- Consumes: completed implementation.
- Produces: durable project direction and validated working tree.

- [ ] **Step 1: Update docs**

Replace Supermemory source-of-truth wording with:

```markdown
Local Mem0 owns durable memory extraction, retrieval indexes, and memory CRUD.
Noema owns memory service lifecycle, provenance, UI, model routing, and policy.
```

Also document:

```markdown
Noema uses a private Mem0 sidecar through a Noema-owned local HTTP contract. The sidecar embeds Mem0 OSS, Chroma local persistence, FastEmbed local embeddings, and Noema's private OpenAI-compatible memory model proxy.
```

- [ ] **Step 2: Run format check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 3: Run Rust check**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 4: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 5: Run unit tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: PASS.

- [ ] **Step 6: Run frontend typecheck**

Run:

```bash
cd crates/noema-core/web
bun run typecheck
```

Expected: PASS.

- [ ] **Step 7: Inspect final diff**

Run:

```bash
git status --short --branch
git diff --check
git diff --stat
```

Expected: no whitespace errors; only Mem0 replacement files changed.

- [ ] **Step 8: Commit docs and final cleanup**

```bash
git add docs/project.md docs/context/current.md docs/superpowers/plans/2026-07-08-replace-supermemory-with-mem0.md
git commit -m "docs: record mem0 memory architecture"
```

---

## Risk Gates

- **Gate 1:** If Task 1 cannot initialize Mem0 with Chroma + FastEmbed + Noema's OpenAI-compatible proxy without OpenAI credentials, stop and revisit. Do not fall back to Mem0's Docker REST stack because it reintroduces Postgres and Neo4j.
- **Gate 2:** If Mem0 add calls require full conversation replay to extract useful memories, stop and revisit. The target architecture is one persisted user observation per turn.
- **Gate 3:** If Mem0's local SDK does not expose enough memory metadata/history for Noema's `/memory` page, keep the native memory list and defer graph visualization instead of inventing fake graph semantics.
- **Gate 4:** If Python sidecar packaging makes `cargo dev` materially slower or flaky, keep it as an explicit external mode for the first slice and do not remove Supermemory until a private managed sidecar is reliable.

## Self-Review

- Spec coverage: plan covers lifecycle, local-only storage, model routing, async ingest, search tool, settings UI, `/memory`, docs, and Supermemory removal.
- Placeholder scan: no implementation task contains `TBD` or `TODO`; risk gates explicitly define stop conditions.
- Type consistency: Rust client names use `Mem0*`; GraphQL keeps generic `Memory*`; sidecar contract uses `/v1/memories/*` Noema-owned paths.
- Known architectural caveat: Mem0 official self-host REST stack conflicts with Noema's no-graph-DB direction, so this plan intentionally embeds Mem0 OSS behind a Noema sidecar instead.
