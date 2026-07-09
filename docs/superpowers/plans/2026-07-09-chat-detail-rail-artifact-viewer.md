# Chat Detail Rail Artifact Viewer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a generic chat-owned read-only detail rail that can render local Markdown artifact versions and offer download from the rail header.

**Architecture:** `ChatSurface` owns disposable detail state. Transcript artifact rows request a detail target, and `ChatSurface` renders a generic `ChatDetailRail` that delegates artifact targets to an `ArtifactDetailPanel`. The backend exposes a version-oriented GraphQL query that safely reads local Markdown artifact bytes through the existing artifact path validation helpers.

**Tech Stack:** Rust, async-graphql, SQLite store, cap-std artifact file reads, React 19, Apollo Client, Astryx `Item` and `Markdown`, StyleX, lucide-react.

## Global Constraints

- The first slice has no URL routing, query params, deep links, back/forward integration, or restoration after refresh.
- Detail state is owned by `ChatSurface`, not `AppRoot` or the global shell.
- The rail is generic from day one; the first target is `{ type: "artifact"; version: string }`.
- Download is secondary and belongs in the rail header, not as the primary artifact card affordance.
- Markdown is the only rendered format in this slice; unsupported local files show a read-only unsupported state with download.
- External artifacts keep external opening behavior for now and do not route through the local detail rail.
- No Markdown editing, title editing, comments, diffing, save workflow, or version picker.
- Do not write browser/UI tests unless explicitly requested; validate frontend with `bun run lint`.
- Preserve existing artifact path safety and symlink protections.

---

## File Structure

- Modify `crates/noema-core/src/graphql/artifacts.rs`
  - Add `GraphqlArtifactVersionDetail` and `GraphqlArtifactVersionPreviewKind`.
  - Add `artifact_version_detail(state, artifact_version_id)` resolver helper.
  - Reuse `crate::artifacts::read_validated_local_artifact_file` for local bytes.
- Modify `crates/noema-core/src/graphql/schema.rs`
  - Add query field `artifactVersionDetail(artifactVersionId: String!): ArtifactVersionDetail`.
  - Add focused GraphQL tests for Markdown content and unsupported local file behavior.
- Modify `crates/noema-core/web/src/graphql/operations.ts`
  - Add `ArtifactVersionDetail` query document.
- Create `crates/noema-core/web/src/components/chatDetail/chatDetailTypes.ts`
  - Define the generic `ChatDetailTarget` union and helpers.
- Create `crates/noema-core/web/src/components/chatDetail/ChatDetailRail.tsx`
  - Render generic rail frame, close action, and target dispatch.
- Create `crates/noema-core/web/src/components/chatDetail/ArtifactDetailPanel.tsx`
  - Fetch artifact version detail, render Markdown or unsupported/error state, expose download action in header area.
- Modify `crates/noema-core/web/src/components/ChatSurface.tsx`
  - Own `detailTarget`, render chat plus optional rail, pass `onOpenDetail` down.
- Modify `crates/noema-core/web/src/components/transcript/Transcript.tsx`
  - Accept and forward `onOpenDetail`.
- Modify `crates/noema-core/web/src/components/transcript/ArtifactReferenceCard.tsx`
  - Use `onOpenDetail` for local artifacts with `artifact_version_id`.
  - Keep external URL behavior for external artifacts.
  - Remove primary "Download" action label from local cards; local card action becomes "Open".
- Modify generated files with `bun run lint`
  - `crates/noema-core/web/src/generated/schema.graphql`
  - `crates/noema-core/web/src/generated/graphql.ts`

---

### Task 1: Backend Artifact Version Detail Query

**Files:**
- Modify: `crates/noema-core/src/graphql/artifacts.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`

**Interfaces:**
- Consumes:
  - `GraphqlState::store()`
  - `GraphqlState::paths()`
  - `NoemaStore::get_artifact_version(&str)`
  - `NoemaStore::get_artifact(&str)`
  - `crate::artifacts::read_validated_local_artifact_file(paths, artifact, version)`
  - `crate::artifact_download_url(&str)`
- Produces:
  - GraphQL type `ArtifactVersionPreviewKind` with values `MARKDOWN`, `UNSUPPORTED`, `EXTERNAL`
  - GraphQL type `ArtifactVersionDetail`
  - Query `artifactVersionDetail(artifactVersionId: String!): ArtifactVersionDetail`

- [ ] **Step 1: Add failing GraphQL test for Markdown detail**

Add this test inside `#[cfg(test)] mod tests` in `crates/noema-core/src/graphql/schema.rs`, near existing artifact query tests:

```rust
#[tokio::test]
async fn artifact_version_detail_reads_markdown_content() {
    let home = tempfile::TempDir::new().expect("home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let artifact = crate::create_conversation_local_file_artifact(
        &store,
        &paths,
        crate::NewConversationLocalFileArtifact {
            conversation_id: conversation.conversation_id,
            title: "Session report".to_string(),
            description: Some("Local markdown artifact".to_string()),
            artifact_kind: "document".to_string(),
            filename: "report.md".to_string(),
            bytes: b"# Report\n\nA useful note.\n".to_vec(),
            media_type: Some("text/markdown".to_string()),
            created_by_actor_id: "agent:primary".to_string(),
            source: crate::ArtifactSource::default(),
            metadata: serde_json::json!({}),
        },
    )
    .await
    .expect("artifact");
    let schema = build_schema(GraphqlState::for_tests_with_store_and_paths(store, paths));

    let response = schema
        .execute(async_graphql::Request::new(format!(
            r#"
            {{
              artifactVersionDetail(artifactVersionId: "{}") {{
                artifactVersionId
                artifactId
                title
                artifactKind
                storageKind
                mediaType
                previewKind
                markdown
                downloadUrl
                externalUrl
              }}
            }}
            "#,
            artifact.current_version.artifact_version_id
        )))
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    let detail = data["artifactVersionDetail"]
        .as_object()
        .expect("detail object");
    assert_eq!(
        detail["artifactVersionId"],
        artifact.current_version.artifact_version_id
    );
    assert_eq!(detail["artifactId"], artifact.artifact.artifact_id);
    assert_eq!(detail["title"], "Session report");
    assert_eq!(detail["artifactKind"], "document");
    assert_eq!(detail["storageKind"], "LOCAL_FILE");
    assert_eq!(detail["mediaType"], "text/markdown");
    assert_eq!(detail["previewKind"], "MARKDOWN");
    assert_eq!(detail["markdown"], "# Report\n\nA useful note.\n");
    assert_eq!(
        detail["downloadUrl"],
        crate::artifact_download_url(&artifact.current_version.artifact_version_id)
    );
    assert!(detail["externalUrl"].is_null());
}
```

- [ ] **Step 2: Add failing GraphQL test for unsupported local detail**

Add this second test near the Markdown detail test:

```rust
#[tokio::test]
async fn artifact_version_detail_marks_non_markdown_local_file_unsupported() {
    let home = tempfile::TempDir::new().expect("home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let artifact = crate::create_conversation_local_file_artifact(
        &store,
        &paths,
        crate::NewConversationLocalFileArtifact {
            conversation_id: conversation.conversation_id,
            title: "Data export".to_string(),
            description: None,
            artifact_kind: "table".to_string(),
            filename: "data.csv".to_string(),
            bytes: b"a,b\n1,2\n".to_vec(),
            media_type: Some("text/csv".to_string()),
            created_by_actor_id: "agent:primary".to_string(),
            source: crate::ArtifactSource::default(),
            metadata: serde_json::json!({}),
        },
    )
    .await
    .expect("artifact");
    let schema = build_schema(GraphqlState::for_tests_with_store_and_paths(store, paths));

    let response = schema
        .execute(async_graphql::Request::new(format!(
            r#"
            {{
              artifactVersionDetail(artifactVersionId: "{}") {{
                previewKind
                markdown
                downloadUrl
                mediaType
              }}
            }}
            "#,
            artifact.current_version.artifact_version_id
        )))
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    let detail = data["artifactVersionDetail"]
        .as_object()
        .expect("detail object");
    assert_eq!(detail["previewKind"], "UNSUPPORTED");
    assert!(detail["markdown"].is_null());
    assert_eq!(detail["mediaType"], "text/csv");
    assert_eq!(
        detail["downloadUrl"],
        crate::artifact_download_url(&artifact.current_version.artifact_version_id)
    );
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run:

```bash
cargo test -p noema-core graphql::schema::tests::artifact_version_detail -- --nocapture
```

Expected: FAIL because `artifactVersionDetail` does not exist in the schema.

- [ ] **Step 4: Add GraphQL types and resolver helper**

In `crates/noema-core/src/graphql/artifacts.rs`, extend the imports:

```rust
use async_graphql::{Enum, InputObject, Result, SimpleObject};
```

Then add these types after `GraphqlArtifactVersion`:

```rust
/// Preview renderer selected for an artifact version detail panel.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ArtifactVersionPreviewKind")]
pub enum GraphqlArtifactVersionPreviewKind {
    /// Local Markdown bytes are available as UTF-8 text.
    Markdown,
    /// The version exists but this first slice cannot render it inline.
    Unsupported,
    /// The version points at an external URL.
    External,
}

/// Artifact version detail payload for the chat detail rail.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ArtifactVersionDetail")]
pub struct GraphqlArtifactVersionDetail {
    /// Stable artifact version id.
    pub artifact_version_id: String,
    /// Parent artifact id.
    pub artifact_id: String,
    /// Display title inherited from version title or artifact title.
    pub title: String,
    /// Product-defined artifact kind label.
    pub artifact_kind: String,
    /// Durable storage family shared by the artifact.
    pub storage_kind: GraphqlArtifactStorageKind,
    /// Optional media type for the version payload.
    pub media_type: Option<String>,
    /// Preview renderer selected by the server.
    pub preview_kind: GraphqlArtifactVersionPreviewKind,
    /// Markdown content when previewKind is MARKDOWN.
    pub markdown: Option<String>,
    /// Local download route when the version is stored in Noema.
    pub download_url: Option<String>,
    /// External durable URL when the version is externally hosted.
    pub external_url: Option<String>,
}
```

Add this resolver helper after `artifact(...)`:

```rust
pub async fn artifact_version_detail(
    state: &GraphqlState,
    artifact_version_id: String,
) -> Result<Option<GraphqlArtifactVersionDetail>> {
    let store = state.store()?;
    let Some(version) = store
        .get_artifact_version(&artifact_version_id)
        .await
        .map_err(graphql_error)?
    else {
        return Ok(None);
    };

    let Some(artifact) = store
        .get_artifact(&version.artifact_id)
        .await
        .map_err(graphql_error)?
    else {
        return Ok(None);
    };

    let title = version
        .title
        .clone()
        .unwrap_or_else(|| artifact.artifact.title.clone());
    let media_type = version.media_type.clone();

    match &version.storage {
        crate::ArtifactVersionStorage::ExternalUrl { url } => Ok(Some(
            GraphqlArtifactVersionDetail {
                artifact_version_id: version.artifact_version_id,
                artifact_id: version.artifact_id,
                title,
                artifact_kind: artifact.artifact.artifact_kind,
                storage_kind: artifact.artifact.storage_kind.into(),
                media_type,
                preview_kind: GraphqlArtifactVersionPreviewKind::External,
                markdown: None,
                download_url: None,
                external_url: Some(url.clone()),
            },
        )),
        crate::ArtifactVersionStorage::LocalFile { .. } => {
            let download_url = Some(crate::artifact_download_url(&version.artifact_version_id));
            if !is_markdown_media_type(media_type.as_deref()) {
                return Ok(Some(GraphqlArtifactVersionDetail {
                    artifact_version_id: version.artifact_version_id,
                    artifact_id: version.artifact_id,
                    title,
                    artifact_kind: artifact.artifact.artifact_kind,
                    storage_kind: artifact.artifact.storage_kind.into(),
                    media_type,
                    preview_kind: GraphqlArtifactVersionPreviewKind::Unsupported,
                    markdown: None,
                    download_url,
                    external_url: None,
                }));
            }

            let (_, bytes) = crate::artifacts::read_validated_local_artifact_file(
                state.paths()?,
                &artifact.artifact,
                &version,
            )
            .map_err(graphql_error)?;
            let markdown = String::from_utf8(bytes).map_err(|error| {
                graphql_error(format!("artifact Markdown content is not valid UTF-8: {error}"))
            })?;

            Ok(Some(GraphqlArtifactVersionDetail {
                artifact_version_id: version.artifact_version_id,
                artifact_id: version.artifact_id,
                title,
                artifact_kind: artifact.artifact.artifact_kind,
                storage_kind: artifact.artifact.storage_kind.into(),
                media_type,
                preview_kind: GraphqlArtifactVersionPreviewKind::Markdown,
                markdown: Some(markdown),
                download_url,
                external_url: None,
            }))
        }
    }
}

fn is_markdown_media_type(media_type: Option<&str>) -> bool {
    media_type
        .map(|value| {
            let normalized = value
                .split(';')
                .next()
                .unwrap_or(value)
                .trim()
                .to_ascii_lowercase();
            normalized == "text/markdown" || normalized == "text/x-markdown"
        })
        .unwrap_or(false)
}
```

- [ ] **Step 5: Add query field**

In `crates/noema-core/src/graphql/schema.rs`, add this query method after `artifact(...)`:

```rust
    /// Load one artifact version detail payload for the chat detail rail.
    async fn artifact_version_detail(
        &self,
        ctx: &Context<'_>,
        artifact_version_id: String,
    ) -> Result<Option<artifacts::GraphqlArtifactVersionDetail>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        artifacts::artifact_version_detail(state, artifact_version_id).await
    }
```

- [ ] **Step 6: Run focused backend tests**

Run:

```bash
cargo test -p noema-core graphql::schema::tests::artifact_version_detail -- --nocapture
```

Expected: PASS; both new artifact-version detail tests pass.

- [ ] **Step 7: Commit backend query**

```bash
git add crates/noema-core/src/graphql/artifacts.rs crates/noema-core/src/graphql/schema.rs
git commit -m "Add artifact version detail query"
```

---

### Task 2: Frontend GraphQL Operation And Detail Types

**Files:**
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Create: `crates/noema-core/web/src/components/chatDetail/chatDetailTypes.ts`
- Generated by validation: `crates/noema-core/web/src/generated/schema.graphql`
- Generated by validation: `crates/noema-core/web/src/generated/graphql.ts`

**Interfaces:**
- Consumes:
  - Query field `artifactVersionDetail(artifactVersionId: String!)`
- Produces:
  - `ArtifactVersionDetailDocument`
  - `ArtifactVersionDetailQuery`
  - `ArtifactVersionDetailQueryVariables`
  - `type ChatDetailTarget = { type: "artifact"; version: string }`
  - `artifactDetailTarget(version: string): ChatDetailTarget | null`

- [ ] **Step 1: Add GraphQL operation**

Append this query to `crates/noema-core/web/src/graphql/operations.ts` near the other artifact operations:

```ts
export const ARTIFACT_VERSION_DETAIL_QUERY = gql`
  query ArtifactVersionDetail($artifactVersionId: String!) {
    artifactVersionDetail(artifactVersionId: $artifactVersionId) {
      artifactVersionId
      artifactId
      title
      artifactKind
      storageKind
      mediaType
      previewKind
      markdown
      downloadUrl
      externalUrl
    }
  }
`;
```

- [ ] **Step 2: Create detail target type helper**

Create `crates/noema-core/web/src/components/chatDetail/chatDetailTypes.ts`:

```ts
export type ChatDetailTarget = {
  type: "artifact";
  version: string;
};

export function artifactDetailTarget(version: string | null | undefined): ChatDetailTarget | null {
  const trimmed = version?.trim();
  return trimmed ? { type: "artifact", version: trimmed } : null;
}
```

- [ ] **Step 3: Generate frontend schema and operation types**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: PASS and updates to `src/generated/schema.graphql` and `src/generated/graphql.ts`.

- [ ] **Step 4: Verify generated symbols exist**

Run:

```bash
rg "ArtifactVersionDetail(Document|Query|PreviewKind)" src/generated/graphql.ts
```

Expected: output includes `ArtifactVersionDetailDocument`, `ArtifactVersionDetailQuery`, and the enum/type names generated from the backend schema.

- [ ] **Step 5: Commit frontend operation/types**

```bash
git add \
  crates/noema-core/web/src/graphql/operations.ts \
  crates/noema-core/web/src/components/chatDetail/chatDetailTypes.ts \
  crates/noema-core/web/src/generated/schema.graphql \
  crates/noema-core/web/src/generated/graphql.ts
git commit -m "Add artifact detail frontend query"
```

---

### Task 3: Generic Chat Detail Rail And Artifact Panel

**Files:**
- Create: `crates/noema-core/web/src/components/chatDetail/ChatDetailRail.tsx`
- Create: `crates/noema-core/web/src/components/chatDetail/ArtifactDetailPanel.tsx`

**Interfaces:**
- Consumes:
  - `ChatDetailTarget` from `chatDetailTypes.ts`
  - `ArtifactVersionDetailDocument` and generated types
  - Astryx `Markdown`
- Produces:
  - `ChatDetailRail({ target, onClose })`
  - Read-only artifact detail UI with `Download` action in the header area

- [ ] **Step 1: Create artifact panel**

Create `crates/noema-core/web/src/components/chatDetail/ArtifactDetailPanel.tsx`:

```tsx
import React from "react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { Download } from "lucide-react";
import {
  ArtifactVersionDetailDocument,
  ArtifactVersionPreviewKind,
  type ArtifactVersionDetailQuery
} from "@/generated/graphql";

type MarkdownXStyle = MarkdownProps["xstyle"];
export type ArtifactDetail = NonNullable<ArtifactVersionDetailQuery["artifactVersionDetail"]>;

export function ArtifactDetailPanel({
  version,
  onDetailChange
}: {
  version: string;
  onDetailChange: (detail: ArtifactDetail | null) => void;
}) {
  const { data, error, loading } = useQuery(ArtifactVersionDetailDocument, {
    fetchPolicy: "cache-and-network",
    variables: { artifactVersionId: version }
  });
  const detail = data?.artifactVersionDetail ?? null;

  React.useEffect(() => {
    onDetailChange(detail);
  }, [detail, onDetailChange]);

  if (loading && !detail) {
    return <div role="status" {...stylex.props(styles.status)}>Loading artifact...</div>;
  }

  if (error) {
    return <ArtifactUnavailable message={`Error loading artifact: ${error.message}`} />;
  }

  if (!detail) {
    return <ArtifactUnavailable message="Artifact unavailable" />;
  }

  return (
    <div {...stylex.props(styles.root)}>
      <ArtifactMeta detail={detail} />
      {detail.previewKind === ArtifactVersionPreviewKind.Markdown && detail.markdown ? (
        <Markdown
          autolink="gfm"
          contentWidth="100%"
          density="comfortable"
          headingLevelStart={1}
          xstyle={markdownXStyle(styles.markdown)}
        >
          {detail.markdown}
        </Markdown>
      ) : (
        <ArtifactUnavailable message="Preview unavailable" />
      )}
    </div>
  );
}

export function ArtifactDownloadAction({ detail }: { detail: ArtifactDetail | null }) {
  if (!detail?.downloadUrl) {
    return null;
  }

  return (
    <Button
      href={detail.downloadUrl}
      icon={<Download aria-hidden="true" size={15} />}
      label="Download"
      size="sm"
      variant="secondary"
    />
  );
}

function ArtifactMeta({ detail }: { detail: ArtifactDetail }) {
  const parts = [detail.artifactKind, detail.mediaType].filter(Boolean);
  return parts.length > 0 ? <div {...stylex.props(styles.meta)}>{parts.join(" · ")}</div> : null;
}

function ArtifactUnavailable({ message }: { message: string }) {
  return (
    <div role="status" {...stylex.props(styles.empty)}>
      <p>{message}</p>
    </div>
  );
}

function markdownXStyle(...xstyle: unknown[]): MarkdownXStyle {
  return xstyle as unknown as MarkdownXStyle;
}

const styles = stylex.create({
  root: {
    minWidth: 0,
    display: "grid",
    gap: 16
  },
  status: {
    color: "var(--noema-text-secondary)",
    fontSize: 13
  },
  meta: {
    color: "var(--noema-text-muted)",
    fontSize: 12
  },
  markdown: {
    color: "var(--noema-text-primary)",
    fontSize: 14,
    lineHeight: 1.55
  },
  empty: {
    display: "grid",
    gap: 12,
    alignContent: "start",
    color: "var(--noema-text-secondary)",
    fontSize: 13
  }
});
```

- [ ] **Step 2: Create generic rail**

Create `crates/noema-core/web/src/components/chatDetail/ChatDetailRail.tsx`:

```tsx
import React from "react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { X } from "lucide-react";
import {
  ArtifactDetailPanel,
  ArtifactDownloadAction,
  type ArtifactDetail
} from "./ArtifactDetailPanel";
import type { ChatDetailTarget } from "./chatDetailTypes";

export function ChatDetailRail({
  target,
  onClose
}: {
  target: ChatDetailTarget;
  onClose: () => void;
}) {
  const closeButtonRef = React.useRef<HTMLButtonElement>(null);
  const [title, setTitle] = React.useState("Artifact");
  const [artifactDetail, setArtifactDetail] = React.useState<ArtifactDetail | null>(null);

  React.useEffect(() => {
    closeButtonRef.current?.focus();
  }, [target]);

  React.useEffect(() => {
    setTitle(target.type === "artifact" ? "Artifact" : "Details");
    setArtifactDetail(null);
  }, [target]);

  const updateArtifactDetail = React.useCallback((detail: ArtifactDetail | null) => {
    setArtifactDetail(detail);
    if (detail?.title) {
      setTitle(detail.title);
    }
  }, []);

  return (
    <aside data-slot="chat-detail-rail" aria-label="Chat detail" {...stylex.props(styles.rail)}>
      <header {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.titleBlock)}>
          <div {...stylex.props(styles.kicker)}>Detail</div>
          <h2 {...stylex.props(styles.title)}>{title}</h2>
        </div>
        {target.type === "artifact" ? <ArtifactDownloadAction detail={artifactDetail} /> : null}
        <Button
          ref={closeButtonRef}
          type="button"
          variant="ghost"
          size="sm"
          label="Close detail"
          icon={<X aria-hidden="true" size={16} />}
          isIconOnly
          onClick={onClose}
        />
      </header>
      <div {...stylex.props(styles.body)}>
        {target.type === "artifact" ? (
          <ArtifactDetailPanel version={target.version} onDetailChange={updateArtifactDetail} />
        ) : null}
      </div>
    </aside>
  );
}

const styles = stylex.create({
  rail: {
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)",
    minWidth: 0,
    height: "100%",
    borderLeftWidth: 1,
    borderLeftStyle: "solid",
    borderLeftColor: "var(--noema-border-subtle)",
    backgroundColor: "var(--noema-surface-card)"
  },
  header: {
    minWidth: 0,
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto auto",
    alignItems: "start",
    gap: 12,
    paddingBlock: 14,
    paddingInline: 16,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--noema-border-subtle)"
  },
  titleBlock: {
    minWidth: 0,
    display: "grid",
    gap: 2
  },
  kicker: {
    color: "var(--noema-text-muted)",
    fontSize: 11,
    fontWeight: 600,
    textTransform: "uppercase"
  },
  title: {
    margin: 0,
    color: "var(--noema-text-primary)",
    fontSize: 15,
    fontWeight: 650,
    lineHeight: 1.25,
    overflowWrap: "anywhere"
  },
  body: {
    minHeight: 0,
    overflow: "auto",
    padding: 16
  }
});
```

- [ ] **Step 3: Run frontend lint and fix type/style issues**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: PASS. If Astryx `Button` does not accept the close button ref in this project version, focus the rail title instead by adding `tabIndex={-1}` and a `ref` to the `<h2>`.

- [ ] **Step 4: Commit rail components**

```bash
git add \
  crates/noema-core/web/src/components/chatDetail/ChatDetailRail.tsx \
  crates/noema-core/web/src/components/chatDetail/ArtifactDetailPanel.tsx
git commit -m "Add chat detail rail components"
```

---

### Task 4: Wire Chat-Owned Detail State Through Transcript

**Files:**
- Modify: `crates/noema-core/web/src/components/ChatSurface.tsx`
- Modify: `crates/noema-core/web/src/components/transcript/Transcript.tsx`
- Modify: `crates/noema-core/web/src/components/transcript/ArtifactReferenceCard.tsx`

**Interfaces:**
- Consumes:
  - `ChatDetailTarget`
  - `artifactDetailTarget(version)`
  - `ChatDetailRail`
- Produces:
  - Chat-owned detail rail state
  - Artifact card local click opens detail rail when possible

- [ ] **Step 1: Update `ArtifactReferenceCard` props and click behavior**

In `crates/noema-core/web/src/components/transcript/ArtifactReferenceCard.tsx`, import:

```ts
import { artifactDetailTarget, type ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
```

Change props:

```ts
export function ArtifactReferenceCard({
  item,
  onOpenDetail
}: {
  item: ArtifactReferenceItem;
  onOpenDetail?: (target: ChatDetailTarget) => void;
}) {
```

Add this after `const link = ...`:

```ts
  const detailTarget =
    item.storage_kind === "local_file" ? artifactDetailTarget(item.artifact_version_id) : null;
  const opensDetail = Boolean(detailTarget && onOpenDetail);
```

Change action label:

```ts
  const actionLabel = opensDetail ? "Open" : link ? (link.external ? "Open" : "Download") : "Unavailable";
```

Change the `<Item>` interaction props:

```tsx
      href={opensDetail ? undefined : link?.href}
      onClick={
        opensDetail && detailTarget
          ? () => {
              onOpenDetail?.(detailTarget);
            }
          : undefined
      }
```

Keep external links using `href`, `target`, and `rel`. Keep local fallback download only when there is no version target.

- [ ] **Step 2: Update `Transcript` callback prop**

In `crates/noema-core/web/src/components/transcript/Transcript.tsx`, import:

```ts
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
```

Add to `Transcript` props:

```ts
  onOpenDetail?: (target: ChatDetailTarget) => void;
```

Pass it to artifact cards:

```tsx
        <ArtifactReferenceCard item={entry.item} onOpenDetail={onOpenDetail} />
```

- [ ] **Step 3: Update `ChatSurface` layout and state**

In `crates/noema-core/web/src/components/ChatSurface.tsx`, import:

```ts
import { ChatDetailRail } from "./chatDetail/ChatDetailRail";
import type { ChatDetailTarget } from "./chatDetail/chatDetailTypes";
```

Add state inside `ChatSurface`:

```ts
  const [detailTarget, setDetailTarget] = React.useState<ChatDetailTarget | null>(null);
```

Replace the existing `return` block with this structure, preserving the existing
transcript/composer props and adding `onOpenDetail={setDetailTarget}`:

```tsx
  return (
    <section
      data-slot="chat-surface"
      data-detail-open={detailTarget ? "true" : undefined}
      {...stylex.props(styles.root)}
      style={rootStyle}
      aria-label="Noema chat"
    >
      <div data-slot="chat-main-pane" {...stylex.props(styles.mainPane)}>
        <div {...stylex.props(styles.contentLayer)}>
          {loadingInitialTranscript || transcript.length === 0 ? (
            <TranscriptLoadingSkeleton />
          ) : (
            <Transcript
              entries={transcript}
              loadingOlderTranscript={loadingOlderTranscript}
              hasMoreTranscriptBefore={hasMoreTranscriptBefore}
              olderTranscriptPageError={olderTranscriptPageError}
              pending={pending}
              agentStatus={agentStatus}
              awaitingAssistantTurn={awaitingAssistantTurn}
              expandedActivities={expandedActivities}
              sentMessageScrollRequest={sentMessageScrollRequest}
              onToggleActivity={onToggleActivity}
              onSubmitMultipleChoiceSelection={onSubmitMultipleChoiceSelection}
              onLoadOlderTranscript={onLoadOlderTranscript}
              onOpenDetail={setDetailTarget}
            />
          )}
        </div>

        <div ref={composerDockRef} data-slot="chat-composer-dock" {...stylex.props(styles.composerDock)}>
          <div aria-hidden="true" data-slot="chat-composer-scrim" {...stylex.props(styles.composerScrim)} />
          <div {...stylex.props(styles.composerLayer)}>
            <Composer
              ref={composerRef}
              value={draft}
              ready={ready}
              pending={pending}
              placeholder={composerPlaceholder({ ready, agentName })}
              onChange={onDraftChange}
              onSubmit={onSubmit}
            />
          </div>
        </div>
      </div>

      {detailTarget ? (
        <ChatDetailRail target={detailTarget} onClose={() => setDetailTarget(null)} />
      ) : null}
    </section>
  );
```

Update styles:

```ts
  root: {
    "--chat-column-width": {
      default: "min(860px, calc(100% - 48px))",
      "@media (max-width: 760px)": "calc(100% - 40px)"
    },
    display: "grid",
    gridTemplateColumns: {
      default: "minmax(0, 1fr)",
      "@media (min-width: 980px)": "minmax(0, 1fr) minmax(320px, 380px)"
    },
    minHeight: 0,
    height: "calc(100% + var(--shell-deck-header-height, 44px))",
    marginTop: "calc(var(--shell-deck-header-height, 44px) * -1)",
    width: "100%",
    overflow: "hidden"
  },
  mainPane: {
    display: "grid",
    gridTemplateRows: "minmax(0, 1fr)",
    minWidth: 0,
    minHeight: 0,
    overflow: "hidden"
  }
```

For the first implementation, narrow screens may stack the rail after the main pane inside the grid. Do not add URL state.

- [ ] **Step 4: Run frontend lint**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: PASS.

- [ ] **Step 5: Commit wiring**

```bash
git add \
  crates/noema-core/web/src/components/ChatSurface.tsx \
  crates/noema-core/web/src/components/transcript/Transcript.tsx \
  crates/noema-core/web/src/components/transcript/ArtifactReferenceCard.tsx
git commit -m "Open artifacts in chat detail rail"
```

---

### Task 5: Integration Validation And Cleanup

**Files:**
- Inspect: all files changed in Tasks 1-4
- Optional modify: `docs/context/current.md` only if this becomes durable project context beyond the committed spec

**Interfaces:**
- Consumes:
  - Completed backend query
  - Generated frontend GraphQL types
  - Chat-owned detail rail wiring
- Produces:
  - Validated implementation ready for review

- [ ] **Step 1: Run full required validation**

Run from repo root:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all commands PASS.

- [ ] **Step 2: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: PASS.

- [ ] **Step 3: Run ship checklist**

Run from repo root:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected before final commit: only intentional files are modified or staged; `git diff --check` prints no output.

- [ ] **Step 4: Commit any final cleanup**

If validation required fixes after the prior task commits, commit the cleanup with the exact fixed paths:

```bash
git add crates/noema-core/src/graphql/artifacts.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/components/chatDetail/chatDetailTypes.ts crates/noema-core/web/src/components/chatDetail/ChatDetailRail.tsx crates/noema-core/web/src/components/chatDetail/ArtifactDetailPanel.tsx crates/noema-core/web/src/components/ChatSurface.tsx crates/noema-core/web/src/components/transcript/Transcript.tsx crates/noema-core/web/src/components/transcript/ArtifactReferenceCard.tsx crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts
git commit -m "Polish chat detail artifact viewer"
```

If no fixes are needed, do not create an empty commit.

- [ ] **Step 5: Report final state**

Include in the final summary:

- backend query added for artifact version detail
- generic chat-owned detail rail added
- Markdown artifacts open in the rail
- download action moved into rail
- unsupported local files show preview unavailable with download
- validation commands and results
- remaining unstaged/untracked files, if any

---

## Self-Review

- Spec coverage: The plan covers generic rail ownership, no URL state, Markdown-only rendering, unsupported states, download in rail header, external artifact behavior, backend data flow, accessibility focus basics, and validation.
- Placeholder scan: The plan contains no TBD/TODO/fill-in placeholders. The only conditional guidance is tied to concrete installed-package type checks or validation fixes.
- Type consistency: The detail target is consistently `{ type: "artifact"; version: string }`; GraphQL names are consistently `artifactVersionDetail`, `ArtifactVersionDetail`, and `ArtifactVersionPreviewKind`.
