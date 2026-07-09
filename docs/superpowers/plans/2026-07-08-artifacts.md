# Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build governed, versioned artifacts so Noema can persist local files and external links for users to view, download, and iterate on.

**Architecture:** Add artifact metadata/version tables to SQLite, keep local bytes in owner-owned filesystem paths, and expose artifacts through thin GraphQL resolvers plus a read-only local download route. Artifacts use governed owner semantics independent of storage kind, while transcript items only reference artifacts for chat visibility.

**Tech Stack:** Rust, rusqlite, async-graphql, tokio local HTTP server, ring digest, React, Apollo GraphQL codegen, StyleX.

## Global Constraints

- Work on `main`; preserve unrelated dirty worktree changes.
- Do not add migrations or compatibility layers for pre-V1 SQLite data.
- Do not use file names, English text, URL prefixes, or direct phrase matching as authority for semantic artifact intent.
- Run unit tests only; do not run smoke tests or fixture tests.
- Do not write UI/frontend tests unless explicitly requested.
- Do not modify `CARGO_BUILD_RUSTC_WRAPPER` or interfere with `sccache`.
- Local artifact bytes live under `${NOEMA_HOME}/conversations/<conversation_id>/artifacts/<artifact_id>/versions/<version_index>/<filename>` in the first slice.
- External URL artifacts store durable links only; Noema does not mirror external content.
- Transcript rows reference artifact ids and version ids; artifact tables remain canonical.

---

## File Structure

- `crates/noema-core/src/store/schema.rs`: add `artifacts` and `artifact_versions` tables, indexes, and `artifact_reference` conversation item kind.
- `crates/noema-core/src/store/artifacts.rs`: new artifact records, enums, validation, and repository commands.
- `crates/noema-core/src/store/error.rs`: add artifact-specific store errors.
- `crates/noema-core/src/store.rs`: export the new store module.
- `crates/noema-core/src/lib.rs`: re-export artifact types used by GraphQL and runtime code.
- `crates/noema-core/src/paths.rs`: add conversation artifact path helpers and safe file-name validation.
- `crates/noema-core/src/artifacts.rs`: new local artifact file writer and content hash helper.
- `crates/noema-core/src/graphql/artifacts.rs`: new GraphQL artifact objects, queries, and external-link mutation.
- `crates/noema-core/src/graphql/schema.rs`: wire artifact queries/mutations and tests.
- `crates/noema-core/src/conversation/status.rs`: add `ArtifactReference` item kind.
- `crates/noema-core/src/daemon/protocol.rs`: add `TurnTranscriptItem::ArtifactReference`.
- `crates/noema-core/src/daemon/web/replay.rs`: replay artifact reference rows by resolving artifact metadata.
- `crates/noema-core/src/graphql/chat.rs`: expose `ArtifactReference` in the transcript union.
- `crates/noema-core/src/daemon/web/mod.rs`: add `GET /artifacts/<artifact_version_id>/download`.
- `crates/noema-core/web/src/graphql/operations.ts`: request artifact fields in artifact queries and transcript fragments.
- `crates/noema-core/web/src/shared/types.ts`: add artifact transcript item and entry types.
- `crates/noema-core/web/src/transcript/events.ts`: map GraphQL `ArtifactReference` to transcript entries.
- `crates/noema-core/web/src/components/transcript/ArtifactReferenceCard.tsx`: new compact artifact row.
- `crates/noema-core/web/src/components/transcript/Transcript.tsx`: render artifact entries through the new card.
- Generated files under `crates/noema-core/web/src/generated/`: refresh GraphQL schema and TypeScript types with `bun run gen:types`.
- `docs/context/current.md`: summarize the settled artifact ownership direction.

---

### Task 1: SQLite Schema And Artifact Domain Types

**Files:**
- Modify: `crates/noema-core/src/store/schema.rs`
- Create: `crates/noema-core/src/store/artifacts.rs`
- Modify: `crates/noema-core/src/store/error.rs`
- Modify: `crates/noema-core/src/store.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Test: `crates/noema-core/src/store/tests.rs`

**Interfaces:**
- Produces: `ArtifactOwnerRef`, `ArtifactStorageKind`, `ArtifactVersionStorage`, `ArtifactSource`, `NewArtifact`, `NewArtifactVersion`, `ArtifactRecord`, `ArtifactVersionRecord`.
- Produces: schema tables `artifacts`, `artifact_versions`.
- Consumes: existing `NoemaStore`, `ActorRef`, JSON helpers, and `allocate_id`.

- [ ] **Step 1: Write failing schema test**

Add this test to `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn sqlite_schema_creates_artifact_tables() {
    let store = test_store().await;

    let tables = store
        .with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('artifacts', 'artifact_versions') ORDER BY name",
            )?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("table lookup");

    assert_eq!(tables, vec!["artifact_versions", "artifacts"]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p noema-core store::tests::sqlite_schema_creates_artifact_tables --lib`

Expected: FAIL because the artifact tables do not exist.

- [ ] **Step 3: Add schema tables and indexes**

In `STORE_SCHEMA_SQL`, add `artifact_reference` to the `conversation_items.kind` check and add:

```sql
CREATE TABLE IF NOT EXISTS artifacts (
  artifact_id TEXT PRIMARY KEY NOT NULL,
  owner_object_type TEXT NOT NULL CHECK (owner_object_type IN ('human', 'agent', 'conversation', 'workspace', 'project', 'task', 'tool')),
  owner_object_id TEXT NOT NULL,
  title TEXT NOT NULL CHECK (title <> ''),
  description TEXT,
  artifact_kind TEXT NOT NULL CHECK (artifact_kind <> ''),
  storage_kind TEXT NOT NULL CHECK (storage_kind IN ('local_file', 'external_url')),
  current_version_id TEXT,
  created_by_actor_id TEXT NOT NULL,
  source_conversation_id TEXT,
  source_turn_id TEXT,
  source_item_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  deleted_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS artifact_versions (
  artifact_version_id TEXT PRIMARY KEY NOT NULL,
  artifact_id TEXT NOT NULL,
  version_index INTEGER NOT NULL CHECK (version_index >= 1),
  title TEXT,
  local_relative_path TEXT,
  external_url TEXT,
  media_type TEXT,
  byte_size INTEGER CHECK (byte_size IS NULL OR byte_size >= 0),
  content_sha256 TEXT,
  created_by_actor_id TEXT NOT NULL,
  source_conversation_id TEXT,
  source_turn_id TEXT,
  source_item_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(artifact_id, version_index),
  CHECK (
    (local_relative_path IS NOT NULL AND external_url IS NULL)
    OR (local_relative_path IS NULL AND external_url IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS artifacts_owner
ON artifacts(owner_object_type, owner_object_id, deleted_at, updated_at);

CREATE INDEX IF NOT EXISTS artifact_versions_artifact
ON artifact_versions(artifact_id, version_index);
```

- [ ] **Step 4: Add artifact module domain types**

Create `crates/noema-core/src/store/artifacts.rs` with these public types and parsers:

```rust
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactOwnerRef {
    pub object_type: String,
    pub object_id: String,
}

impl ArtifactOwnerRef {
    #[must_use]
    pub fn conversation(conversation_id: impl Into<String>) -> Self {
        Self {
            object_type: "conversation".to_string(),
            object_id: conversation_id.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactStorageKind {
    LocalFile,
    ExternalUrl,
}

impl ArtifactStorageKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalFile => "local_file",
            Self::ExternalUrl => "external_url",
        }
    }

    pub fn parse(value: &str) -> Result<Self, crate::StoreError> {
        match value {
            "local_file" => Ok(Self::LocalFile),
            "external_url" => Ok(Self::ExternalUrl),
            _ => Err(crate::StoreError::InvalidEnum {
                kind: "artifact_storage_kind",
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactVersionStorage {
    LocalFile { relative_path: String },
    ExternalUrl { url: String },
}

impl ArtifactVersionStorage {
    #[must_use]
    pub fn storage_kind(&self) -> ArtifactStorageKind {
        match self {
            Self::LocalFile { .. } => ArtifactStorageKind::LocalFile,
            Self::ExternalUrl { .. } => ArtifactStorageKind::ExternalUrl,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactSource {
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewArtifact {
    pub artifact_id: Option<String>,
    pub owner: ArtifactOwnerRef,
    pub title: String,
    pub description: Option<String>,
    pub artifact_kind: String,
    pub storage_kind: ArtifactStorageKind,
    pub created_by_actor_id: String,
    pub source: ArtifactSource,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewArtifactVersion {
    pub artifact_version_id: Option<String>,
    pub title: Option<String>,
    pub storage: ArtifactVersionStorage,
    pub media_type: Option<String>,
    pub byte_size: Option<i64>,
    pub content_sha256: Option<String>,
    pub created_by_actor_id: String,
    pub source: ArtifactSource,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactRecord {
    pub artifact_id: String,
    pub owner: ArtifactOwnerRef,
    pub title: String,
    pub description: Option<String>,
    pub artifact_kind: String,
    pub storage_kind: ArtifactStorageKind,
    pub current_version_id: Option<String>,
    pub created_by_actor_id: String,
    pub source: ArtifactSource,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactVersionRecord {
    pub artifact_version_id: String,
    pub artifact_id: String,
    pub version_index: i64,
    pub title: Option<String>,
    pub storage: ArtifactVersionStorage,
    pub media_type: Option<String>,
    pub byte_size: Option<i64>,
    pub content_sha256: Option<String>,
    pub created_by_actor_id: String,
    pub source: ArtifactSource,
    pub metadata: Value,
    pub created_at: String,
}
```

- [ ] **Step 5: Wire exports and errors**

In `crates/noema-core/src/store.rs`, add `mod artifacts;` and export the new types.

In `crates/noema-core/src/lib.rs`, re-export the artifact records/input types used outside the store.

In `StoreError`, add:

```rust
#[error("artifact not found: {artifact_id}")]
ArtifactNotFound { artifact_id: String },
#[error("artifact title cannot be empty")]
ArtifactTitleEmpty,
#[error("artifact kind cannot be empty")]
ArtifactKindEmpty,
#[error("unsupported artifact owner: {owner_object_type}:{owner_object_id}")]
UnsupportedArtifactOwner {
    owner_object_type: String,
    owner_object_id: String,
},
#[error("artifact version storage kind does not match artifact storage kind")]
ArtifactStorageKindMismatch,
```

- [ ] **Step 6: Run schema test**

Run: `cargo test -p noema-core store::tests::sqlite_schema_creates_artifact_tables --lib`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/store/schema.rs crates/noema-core/src/store/artifacts.rs crates/noema-core/src/store/error.rs crates/noema-core/src/store.rs crates/noema-core/src/lib.rs crates/noema-core/src/store/tests.rs
git commit -m "feat: add artifact store schema"
```

---

### Task 2: Artifact Store Commands

**Files:**
- Modify: `crates/noema-core/src/store/artifacts.rs`
- Test: `crates/noema-core/src/store/tests.rs`

**Interfaces:**
- Consumes: types from Task 1.
- Produces: `NoemaStore::create_artifact_with_initial_version`, `append_artifact_version`, `get_artifact`, `get_artifact_version`, `list_artifacts_for_owner`, `list_artifact_versions`, `new_artifact_id`, `new_artifact_version_id`.

- [ ] **Step 1: Write failing store tests**

Add tests named:

```rust
#[tokio::test]
async fn artifact_external_url_initial_version_round_trips() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");

    let artifact = store
        .create_artifact_with_initial_version(
            crate::NewArtifact {
                artifact_id: None,
                owner: crate::ArtifactOwnerRef::conversation(&conversation.conversation_id),
                title: "Sprint brief".to_string(),
                description: Some("Planning notes".to_string()),
                artifact_kind: "document".to_string(),
                storage_kind: crate::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource {
                    conversation_id: Some(conversation.conversation_id.clone()),
                    turn_id: None,
                    item_id: None,
                },
                metadata: serde_json::json!({"provider": "notion"}),
            },
            crate::NewArtifactVersion {
                artifact_version_id: None,
                title: Some("Initial".to_string()),
                storage: crate::ArtifactVersionStorage::ExternalUrl {
                    url: "https://notion.so/noema-brief".to_string(),
                },
                media_type: Some("text/html".to_string()),
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource {
                    conversation_id: Some(conversation.conversation_id.clone()),
                    turn_id: None,
                    item_id: None,
                },
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("artifact");

    assert_eq!(artifact.artifact.title, "Sprint brief");
    assert_eq!(artifact.versions.len(), 1);
    assert_eq!(artifact.current_version.artifact_id, artifact.artifact.artifact_id);
    assert_eq!(artifact.current_version.version_index, 1);
}
```

```rust
#[tokio::test]
async fn append_artifact_version_updates_current_version() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let created = seed_external_artifact(&store, &conversation.conversation_id).await;

    let second = store
        .append_artifact_version(
            &created.artifact.artifact_id,
            crate::NewArtifactVersion {
                artifact_version_id: None,
                title: Some("Revision".to_string()),
                storage: crate::ArtifactVersionStorage::ExternalUrl {
                    url: "https://notion.so/noema-brief-v2".to_string(),
                },
                media_type: Some("text/html".to_string()),
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource::default(),
                metadata: serde_json::json!({"revision": 2}),
            },
        )
        .await
        .expect("append version");

    assert_eq!(second.version_index, 2);
    let loaded = store
        .get_artifact(&created.artifact.artifact_id)
        .await
        .expect("load artifact")
        .expect("artifact exists");
    assert_eq!(
        loaded.artifact.current_version_id.as_deref(),
        Some(second.artifact_version_id.as_str())
    );
    assert_eq!(loaded.versions.len(), 2);
}
```

Add a private `seed_external_artifact` helper in the test module using the same inputs.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p noema-core artifact_external_url_initial_version_round_trips append_artifact_version_updates_current_version --lib`

Expected: FAIL because the store methods are missing.

- [ ] **Step 3: Implement command return type and row mapping**

Add:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactWithVersions {
    pub artifact: ArtifactRecord,
    pub current_version: ArtifactVersionRecord,
    pub versions: Vec<ArtifactVersionRecord>,
}
```

Implement row mappers that parse `storage_kind`, JSON metadata, and version storage. Reject invalid rows with `StoreError::InvalidEnum` or `StoreError::InvariantViolation`.

- [ ] **Step 4: Implement create and append commands**

Implement `create_artifact_with_initial_version` on `NoemaStore`:

- trim and reject empty title;
- trim and reject empty artifact kind;
- require `owner.object_type == "conversation"` for creation in this slice;
- call `require_conversation(&owner.object_id)`;
- verify initial version storage kind matches artifact storage kind;
- allocate ids with prefixes `artifact` and `artifact_version` when input ids are `None`;
- insert artifact and version in one transaction;
- set `current_version_id` to the initial version id.

Implement `append_artifact_version`:

- load artifact by id;
- reject storage kind mismatch;
- compute `COALESCE(MAX(version_index), 0) + 1`;
- insert immutable version;
- update artifact `current_version_id` and `updated_at`.

- [ ] **Step 5: Implement read commands**

Implement:

```rust
pub async fn get_artifact(&self, artifact_id: &str) -> Result<Option<ArtifactWithVersions>, StoreError>
pub async fn get_artifact_version(&self, artifact_version_id: &str) -> Result<Option<ArtifactVersionRecord>, StoreError>
pub async fn list_artifacts_for_owner(&self, owner: ArtifactOwnerRef, limit: i64) -> Result<Vec<ArtifactWithVersions>, StoreError>
pub async fn list_artifact_versions(&self, artifact_id: &str) -> Result<Vec<ArtifactVersionRecord>, StoreError>
pub fn new_artifact_id(&self) -> String
pub fn new_artifact_version_id(&self) -> String
```

Clamp list limit to `1..=100`, exclude soft-deleted artifacts, and order artifacts by newest `updated_at`.

- [ ] **Step 6: Run store tests**

Run: `cargo test -p noema-core artifact_external_url_initial_version_round_trips append_artifact_version_updates_current_version --lib`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/store/artifacts.rs crates/noema-core/src/store/tests.rs
git commit -m "feat: persist artifact versions"
```

---

### Task 3: Local File Artifacts And Download Route

**Files:**
- Modify: `crates/noema-core/src/paths.rs`
- Create: `crates/noema-core/src/artifacts.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `crates/noema-core/src/daemon/web/http.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Test: `crates/noema-core/src/paths.rs`
- Test: `crates/noema-core/src/store/tests.rs`
- Test: `crates/noema-core/src/daemon/web/mod.rs`

**Interfaces:**
- Consumes: store artifact commands from Task 2.
- Produces: `create_conversation_local_file_artifact`, `artifact_download_url`, and web download route.

- [ ] **Step 1: Write failing path and local file tests**

In `paths.rs` tests, add:

```rust
#[test]
fn conversation_artifact_version_dir_lives_under_conversation_artifacts() {
    let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

    assert_eq!(
        paths.conversation_artifact_version_dir("conversation:abc", "artifact:def", 2),
        PathBuf::from("/tmp/noema/conversations/conversation_abc/artifacts/artifact_def/versions/2")
    );
}

#[test]
fn safe_artifact_filename_rejects_path_traversal() {
    assert!(safe_artifact_filename("report.md").is_ok());
    assert!(safe_artifact_filename("../report.md").is_err());
    assert!(safe_artifact_filename("nested/report.md").is_err());
    assert!(safe_artifact_filename("").is_err());
}
```

In `store/tests.rs`, add `conversation_local_file_artifact_writes_bytes_and_metadata` using `tempfile`, `NoemaPaths`, and `create_conversation_local_file_artifact`.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p noema-core conversation_artifact_version_dir_lives_under_conversation_artifacts safe_artifact_filename_rejects_path_traversal conversation_local_file_artifact_writes_bytes_and_metadata --lib`

Expected: FAIL because helpers and writer are missing.

- [ ] **Step 3: Add path helpers**

In `NoemaPaths`, add:

```rust
#[must_use]
pub fn conversations_dir(&self) -> PathBuf {
    self.root.join("conversations")
}

#[must_use]
pub fn conversation_dir(&self, conversation_id: &str) -> PathBuf {
    self.conversations_dir().join(sanitize_path_segment(conversation_id))
}

#[must_use]
pub fn conversation_artifacts_dir(&self, conversation_id: &str) -> PathBuf {
    self.conversation_dir(conversation_id).join("artifacts")
}

#[must_use]
pub fn conversation_artifact_version_dir(
    &self,
    conversation_id: &str,
    artifact_id: &str,
    version_index: i64,
) -> PathBuf {
    self.conversation_artifacts_dir(conversation_id)
        .join(sanitize_path_segment(artifact_id))
        .join("versions")
        .join(version_index.to_string())
}
```

Add `pub(crate) fn safe_artifact_filename(value: &str) -> Result<&str, NoemaPathError>` and extend `NoemaPathError` with `UnsafeArtifactFilename`.

- [ ] **Step 4: Implement local file writer**

Create `crates/noema-core/src/artifacts.rs` with:

```rust
pub struct NewConversationLocalFileArtifact {
    pub conversation_id: String,
    pub title: String,
    pub description: Option<String>,
    pub artifact_kind: String,
    pub filename: String,
    pub bytes: Vec<u8>,
    pub media_type: Option<String>,
    pub created_by_actor_id: String,
    pub source: crate::ArtifactSource,
    pub metadata: serde_json::Value,
}
```

Implement `create_conversation_local_file_artifact(store, paths, input)`:

- preallocate `artifact_id` and `artifact_version_id` with `store.new_artifact_id()` and `store.new_artifact_version_id()`;
- compute the local relative path from those ids before writing bytes;
- write bytes to the conversation artifact version directory;
- compute SHA-256 using `ring::digest::digest(&ring::digest::SHA256, &bytes)`;
- call `create_artifact_with_initial_version` with the preallocated ids after a successful write;
- remove the just-written file on metadata failure.

The final public function signature should be:

```rust
pub async fn create_conversation_local_file_artifact(
    store: &crate::NoemaStore,
    paths: &crate::NoemaPaths,
    input: NewConversationLocalFileArtifact,
) -> Result<crate::ArtifactWithVersions, ArtifactWriteError>
```

- [ ] **Step 5: Add download URL and route**

In GraphQL mapping later, local versions use:

```rust
pub fn artifact_download_url(artifact_version_id: &str) -> String {
    format!("/artifacts/{artifact_version_id}/download")
}
```

In `daemon/web/http.rs`, add `write_binary_response` with optional `Content-Disposition`.

In `daemon/web/mod.rs`, add route detection for `GET /artifacts/<artifact_version_id>/download`, load the version and artifact, reject missing/deleted/external versions, resolve `local_relative_path` under `state.graphql_state().paths()?`, read bytes with `tokio::fs::read`, and return `200 OK`.

- [ ] **Step 6: Run local artifact tests**

Run: `cargo test -p noema-core conversation_artifact_version_dir_lives_under_conversation_artifacts safe_artifact_filename_rejects_path_traversal conversation_local_file_artifact_writes_bytes_and_metadata --lib`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/paths.rs crates/noema-core/src/artifacts.rs crates/noema-core/src/lib.rs crates/noema-core/src/daemon/web/http.rs crates/noema-core/src/daemon/web/mod.rs crates/noema-core/src/store/artifacts.rs crates/noema-core/src/store/tests.rs
git commit -m "feat: write local artifact files"
```

---

### Task 4: GraphQL Artifact API

**Files:**
- Create: `crates/noema-core/src/graphql/artifacts.rs`
- Modify: `crates/noema-core/src/graphql.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Test: `crates/noema-core/src/graphql/schema.rs`

**Interfaces:**
- Consumes: store commands and `artifact_download_url`.
- Produces: `artifacts`, `artifact`, and `createConversationExternalArtifact` GraphQL fields.

- [ ] **Step 1: Write failing GraphQL tests**

Add tests:

```rust
#[tokio::test]
async fn create_conversation_external_artifact_mutation_round_trips() {
    let store = crate::store::tests::test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let schema = build_schema(GraphqlState::for_tests_with_store(store));

    let response = schema.execute(async_graphql::Request::new(format!(
        r#"
        mutation {{
          createConversationExternalArtifact(input: {{
            conversationId: "{}"
            title: "Noema notes"
            artifactKind: "document"
            externalUrl: "https://notion.so/noema-notes"
            mediaType: "text/html"
          }}) {{
            artifactId
            ownerObjectType
            ownerObjectId
            title
            storageKind
            currentVersion {{
              versionIndex
              externalUrl
              downloadUrl
            }}
          }}
        }}
        "#,
        conversation.conversation_id
    ))).await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    let artifact = &data["createConversationExternalArtifact"];
    assert_eq!(artifact["ownerObjectType"], "conversation");
    assert_eq!(artifact["ownerObjectId"], conversation.conversation_id);
    assert_eq!(artifact["storageKind"], "EXTERNAL_URL");
    assert_eq!(artifact["currentVersion"]["versionIndex"], 1);
    assert_eq!(artifact["currentVersion"]["downloadUrl"], serde_json::Value::Null);
}
```

```rust
#[tokio::test]
async fn create_conversation_external_artifact_rejects_non_http_url() {
    let store = crate::store::tests::test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let schema = build_schema(GraphqlState::for_tests_with_store(store));

    let response = schema.execute(async_graphql::Request::new(format!(
        r#"
        mutation {{
          createConversationExternalArtifact(input: {{
            conversationId: "{}"
            title: "Bad"
            artifactKind: "document"
            externalUrl: "file:///tmp/secret.txt"
          }}) {{
            artifactId
          }}
        }}
        "#,
        conversation.conversation_id
    ))).await;

    assert!(!response.errors.is_empty());
    assert!(response.errors[0].message.contains("HTTP"));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p noema-core create_conversation_external_artifact_mutation_round_trips create_conversation_external_artifact_rejects_non_http_url --lib`

Expected: FAIL because GraphQL fields are missing.

- [ ] **Step 3: Implement GraphQL artifacts module**

Create `graphql/artifacts.rs` with:

- `GraphqlArtifactStorageKind` enum mapping to/from `ArtifactStorageKind`;
- `GraphqlArtifact`, `GraphqlArtifactVersion`;
- `GraphqlCreateConversationExternalArtifactInput`;
- `artifacts(state, owner_object_type, owner_object_id, limit)`;
- `artifact(state, artifact_id)`;
- `create_conversation_external_artifact(state, input)`.

URL validation should use `url::Url::parse` and accept only `http` or `https`.

- [ ] **Step 4: Wire schema fields**

In `graphql.rs`, add `mod artifacts;`.

In `QueryRoot`, add:

```rust
async fn artifacts(
    &self,
    ctx: &Context<'_>,
    owner_object_type: String,
    owner_object_id: String,
    limit: Option<i32>,
) -> Result<Vec<artifacts::GraphqlArtifact>>
```

and:

```rust
async fn artifact(&self, ctx: &Context<'_>, artifact_id: String) -> Result<Option<artifacts::GraphqlArtifact>>
```

In `MutationRoot`, add `create_conversation_external_artifact`.

- [ ] **Step 5: Add frontend operations**

In `operations.ts`, add:

```ts
export const ArtifactsDocument = gql`
  query Artifacts($ownerObjectType: String!, $ownerObjectId: String!, $limit: Int) {
    artifacts(ownerObjectType: $ownerObjectType, ownerObjectId: $ownerObjectId, limit: $limit) {
      artifactId
      ownerObjectType
      ownerObjectId
      title
      description
      artifactKind
      storageKind
      currentVersion {
        artifactVersionId
        versionIndex
        externalUrl
        downloadUrl
        mediaType
      }
    }
  }
`;
```

Add `CreateConversationExternalArtifactDocument` with the same fields returned by the mutation test.

- [ ] **Step 6: Run GraphQL tests**

Run: `cargo test -p noema-core create_conversation_external_artifact_mutation_round_trips create_conversation_external_artifact_rejects_non_http_url --lib`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/graphql/artifacts.rs crates/noema-core/src/graphql.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/web/src/graphql/operations.ts
git commit -m "feat: expose artifact graphql api"
```

---

### Task 5: Typed Artifact Transcript References

**Files:**
- Modify: `crates/noema-core/src/conversation/status.rs`
- Modify: `crates/noema-core/src/daemon/protocol.rs`
- Modify: `crates/noema-core/src/daemon/web/replay.rs`
- Modify: `crates/noema-core/src/graphql/chat.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Test: `crates/noema-core/src/daemon/web/replay.rs`
- Test: `crates/noema-core/src/graphql/schema.rs`

**Interfaces:**
- Consumes: artifact store reads from Task 2.
- Produces: `TurnTranscriptItem::ArtifactReference` and `GraphqlArtifactReference`.

- [ ] **Step 1: Write failing replay test**

Add a test that creates an external artifact, appends a conversation item with:

```rust
kind: ConversationItemKind::ArtifactReference,
payload_json: serde_json::json!({
    "artifact_id": artifact.artifact.artifact_id,
    "artifact_version_id": artifact.current_version.artifact_version_id
})
```

Then assert `web_conversation_item_from_record` returns `TurnTranscriptItem::ArtifactReference` with title, kind, storage kind, and external URL.

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p noema-core daemon::web::replay::tests::conversation_replay_maps_artifact_reference --lib`

Expected: FAIL because the item kind and transcript variant do not exist.

- [ ] **Step 3: Add item kind and protocol variant**

Add `ArtifactReference` to `ConversationItemKind`, with storage string `artifact_reference`.

Add to `TurnTranscriptItem`:

```rust
ArtifactReference {
    artifact_id: String,
    artifact_version_id: Option<String>,
    title: String,
    artifact_kind: String,
    storage_kind: String,
    external_url: Option<String>,
    download_url: Option<String>,
    media_type: Option<String>,
}
```

- [ ] **Step 4: Resolve artifact references during replay**

In `replay.rs`, parse payload:

```rust
#[derive(Debug, Deserialize)]
struct ReplayArtifactReferencePayload {
    artifact_id: String,
    #[serde(default)]
    artifact_version_id: Option<String>,
}
```

Make replay resolution async if needed, or add a store-assisted mapping function at the caller layer. Prefer keeping `web_conversation_item_from_record` synchronous by storing display fields in the payload when the reference item is created:

```json
{
  "artifact_id": "...",
  "artifact_version_id": "...",
  "title": "Noema notes",
  "artifact_kind": "document",
  "storage_kind": "external_url",
  "external_url": "https://notion.so/noema-notes",
  "download_url": null,
  "media_type": "text/html"
}
```

This preserves replay shape without adding async store access to the replay mapper. Artifact tables remain canonical for artifact reads; transcript references are display snapshots.

- [ ] **Step 5: Add GraphQL union member**

In `graphql/chat.rs`, add `GraphqlArtifactReference` and include it in `GraphqlTranscriptItem`.

Map `TurnTranscriptItem::ArtifactReference` to the GraphQL type.

- [ ] **Step 6: Run replay and GraphQL tests**

Run: `cargo test -p noema-core conversation_replay_maps_artifact_reference --lib`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/conversation/status.rs crates/noema-core/src/daemon/protocol.rs crates/noema-core/src/daemon/web/replay.rs crates/noema-core/src/graphql/chat.rs crates/noema-core/src/store/schema.rs
git commit -m "feat: add artifact transcript references"
```

---

### Task 6: Frontend Artifact Reference Rendering And Codegen

**Files:**
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Modify: `crates/noema-core/web/src/shared/types.ts`
- Modify: `crates/noema-core/web/src/transcript/events.ts`
- Create: `crates/noema-core/web/src/components/transcript/ArtifactReferenceCard.tsx`
- Modify: `crates/noema-core/web/src/components/transcript/Transcript.tsx`
- Modify: generated GraphQL files under `crates/noema-core/web/src/generated/`

**Interfaces:**
- Consumes: `ArtifactReference` GraphQL union member from Task 5.
- Produces: rendered transcript artifact rows with external or download links.

- [ ] **Step 1: Add GraphQL fragment fields**

In both transcript item selections in `operations.ts`, add:

```graphql
... on ArtifactReference {
  artifactId
  artifactVersionId
  title
  artifactKind
  storageKind
  externalUrl
  downloadUrl
  mediaType
}
```

- [ ] **Step 2: Update shared types and event mapper**

Add `artifact_reference` to `TurnTranscriptItem` and an `artifact` entry to `TranscriptEntry`.

In `transcriptItemFromGraphql`, map `ArtifactReference`.

In `entryFromConversationItem`, return:

```ts
if (transcriptItem.kind === "artifact_reference") {
  return { id: itemId, itemId, cursor, turnId, type: "artifact", item: transcriptItem };
}
```

- [ ] **Step 3: Add artifact card component**

Create `ArtifactReferenceCard.tsx`:

```tsx
import type { TurnTranscriptItem } from "@/shared/types";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";

export function ArtifactReferenceCard({
  item
}: {
  item: Extract<TurnTranscriptItem, { kind: "artifact_reference" }>;
}) {
  const href = item.download_url ?? item.external_url ?? undefined;
  const description = [item.artifact_kind, item.media_type].filter(Boolean).join(" · ");
  return (
    <TranscriptAttachmentCard
      title={href ? <a href={href}>{item.title}</a> : item.title}
      description={description || undefined}
      meta={item.storage_kind === "local_file" ? "file" : "link"}
      icon={item.storage_kind === "local_file" ? "FILE" : "URL"}
      tone="info"
    />
  );
}
```

- [ ] **Step 4: Render artifact entries**

In `Transcript.tsx`, import `ArtifactReferenceCard` and add:

```tsx
if (entry.type === "artifact") {
  return (
    <TranscriptRow lane="assistant" showAvatar={showAvatar}>
      <ArtifactReferenceCard item={entry.item} />
    </TranscriptRow>
  );
}
```

- [ ] **Step 5: Regenerate types**

Run from `crates/noema-core/web`:

```bash
bun run gen:types
```

Expected: generated schema and TypeScript types include `Artifact`, `ArtifactVersion`, and `ArtifactReference`.

- [ ] **Step 6: Run frontend checks**

Run from `crates/noema-core/web`:

```bash
bun run lint
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/shared/types.ts crates/noema-core/web/src/transcript/events.ts crates/noema-core/web/src/components/transcript/ArtifactReferenceCard.tsx crates/noema-core/web/src/components/transcript/Transcript.tsx crates/noema-core/web/src/generated
git commit -m "feat: render artifact references"
```

---

### Task 7: Context Update And Final Validation

**Files:**
- Modify: `docs/context/current.md`

**Interfaces:**
- Consumes: completed artifact implementation.
- Produces: durable project context and final verification.

- [ ] **Step 1: Update durable context**

Add this bullet under settled decisions:

```markdown
- Governed artifacts are durable, versioned outputs owned by the same concrete
  contexts as memory governance. SQLite owns artifact metadata and immutable
  version records; local bytes live under the owner filesystem area and
  external resources are represented as URL versions. The first product slice
  creates conversation-owned artifacts, exposes artifact reads through GraphQL,
  serves local file versions through read-only download URLs, and renders typed
  artifact references in the transcript.
```

- [ ] **Step 2: Run backend unit checks**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: PASS.

- [ ] **Step 3: Run frontend checks**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 4: Ship checklist**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected: no whitespace errors. Report unrelated unstaged files, especially the pre-existing `crates/noema-core/web/src/pages/memoryPageStyles.ts` if still dirty.

- [ ] **Step 5: Commit final context if needed**

```bash
git add docs/context/current.md
git commit -m "docs: record artifact implementation context"
```

Skip this commit only if the context update was already included in a prior artifact implementation commit.

---

## Self-Review

- Spec coverage: the plan covers governed ownership, SQLite metadata, local and external storage, immutable versions, local file hashing, download URLs, GraphQL reads/mutation, typed transcript references, frontend rendering, context update, and validation.
- Placeholder scan: every task names concrete files, commands, expected results, and implementation details.
- Type consistency: artifact ids use `artifact_id` in Rust/SQL and `artifactId` in GraphQL/TypeScript; version ids use `artifact_version_id` and `artifactVersionId`; storage kind values are `local_file`/`external_url` in storage and `LOCAL_FILE`/`EXTERNAL_URL` in GraphQL.
