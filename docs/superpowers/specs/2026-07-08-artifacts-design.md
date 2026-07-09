# Artifacts Design

## Goal

Add durable, iterable artifacts to Noema so agents can create files or register
external resources for the user to view, download, and revise over time.

Artifacts should share Noema's governed ownership semantics with memory:
ownership belongs to concrete contexts such as humans, agents, conversations,
workspaces, projects, tasks, or tools. The first product slice exposes
conversation-created artifacts, but the underlying model must not make
conversation ownership special.

## Context

Noema already treats durable files as filesystem-owned object data and SQLite as
the canonical structured store. The project layout reserves artifact directories
under humans, agents, conversations, and projects. The memory system has also
settled the important ownership principle: facts belong to a governed context,
while the persistence subsystem that stores the content is an implementation
detail.

Artifacts follow the same rule. The artifact owner answers "whose context and
policy governs this output?" The artifact storage kind answers "where are the
bytes or external resource?" A local Markdown report and a Notion document link
can therefore share the same owner, provenance, versioning, and UI contract.

## Non-Goals

- Do not build a full artifact gallery for every owner type in the first slice.
- Do not add browser uploads yet.
- Do not add project, task, workspace, or agent-run creation workflows yet.
- Do not add migrations or compatibility layers for pre-V1 SQLite data.
- Do not use file names, English text, or URL prefixes as the authority for
  artifact semantics.
- Do not mirror external resource contents into Noema unless a future explicit
  import/snapshot workflow asks for that.

## Ownership Model

Artifacts are governed objects. Each artifact has:

```text
owner_object_type
owner_object_id
```

These fields use the same concrete object vocabulary as conversations and
memory governance:

```text
human
agent
conversation
workspace
project
task
tool
```

The first implementation only creates artifacts whose owner is a conversation,
but store and GraphQL reads should accept any valid owner object. This keeps the
schema aligned with future project, task, human, and agent-run surfaces without
building those workflows now.

Artifacts also record source provenance separately from ownership:

```text
source_conversation_id
source_turn_id
source_item_id
created_by_actor_id
```

A project-owned artifact may have been produced during a conversation. A
conversation-owned artifact may later be promoted or copied into a project.
Ownership and provenance must stay separate so policy and history do not get
blurred.

## Storage Model

Artifacts have two initial storage kinds:

```text
local_file
external_url
```

`local_file` versions store bytes under the Noema home directory. `external_url`
versions store a durable link to a resource created or maintained elsewhere,
such as a Notion document.

SQLite owns artifact metadata and version records. The filesystem or external
service owns the content bytes/resource. `system/` indexes and previews remain
derived and rebuildable.

## SQLite Schema

Add `artifacts`:

```sql
CREATE TABLE artifacts (
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
```

Add `artifact_versions`:

```sql
CREATE TABLE artifact_versions (
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
```

The implementation should add indexes for owner listing and version listing:

```sql
CREATE INDEX artifacts_owner
ON artifacts(owner_object_type, owner_object_id, deleted_at, updated_at);

CREATE INDEX artifact_versions_artifact
ON artifact_versions(artifact_id, version_index);
```

Because Noema is pre-V1, the schema may be rewritten directly instead of adding
migrations.

## Versioning Rules

An artifact is the durable identity the user and agent iterate on. An artifact
version is one immutable revision of that identity.

Rules:

- `version_index` starts at `1` for each artifact.
- Creating a new version never mutates an old version.
- `artifacts.current_version_id` points at the latest accepted version.
- Local version bytes are content-address checked with `content_sha256` when
  Noema writes the bytes.
- External URL versions are not content-hashed in the first slice because Noema
  does not own their content.
- A future snapshot/import workflow may create a separate local-file version
  from an external resource.

## Filesystem Layout

Noema should expose path helpers that resolve artifact roots by owner object.

Initial conversation-owned layout:

```text
${NOEMA_HOME}/conversations/<conversation_id>/artifacts/<artifact_id>/versions/<version_index>/<filename>
```

Future layouts should use the same owner resolver:

```text
${NOEMA_HOME}/humans/<human_id>/artifacts/<artifact_id>/versions/<version_index>/<filename>
${NOEMA_HOME}/agents/<agent_id>/runs/<run_id>/artifacts/<artifact_id>/versions/<version_index>/<filename>
${NOEMA_HOME}/workspaces/<workspace_id>/projects/<project_id>/artifacts/<artifact_id>/versions/<version_index>/<filename>
```

Only the conversation path needs to be created in the first implementation.
Other owner paths can return an unsupported-owner-path error until their object
records and path metadata exist.

Local paths stored in SQLite are relative to the Noema home root. Resolvers must
construct absolute paths from `NoemaPaths` and must reject path traversal when
creating local artifact file names.

## Store API

Add focused store records and commands. The artifact store should use an
artifact-specific owner reference so it can match the existing
`owner_object_type` vocabulary in conversation rows even while the core
`ObjectRef` type remains narrower.

```rust
pub struct ArtifactOwnerRef {
    pub object_type: String,
    pub object_id: String,
}

pub struct NewArtifact {
    pub owner: ArtifactOwnerRef,
    pub title: String,
    pub description: Option<String>,
    pub artifact_kind: String,
    pub storage_kind: ArtifactStorageKind,
    pub created_by: ActorRef,
    pub source: ArtifactSource,
    pub metadata: serde_json::Value,
}

pub struct NewArtifactVersion {
    pub artifact_id: String,
    pub title: Option<String>,
    pub storage: ArtifactVersionStorage,
    pub media_type: Option<String>,
    pub byte_size: Option<i64>,
    pub content_sha256: Option<String>,
    pub created_by: ActorRef,
    pub source: ArtifactSource,
    pub metadata: serde_json::Value,
}
```

Commands:

```rust
create_artifact_with_initial_version(new_artifact, initial_version)
append_artifact_version(new_version)
get_artifact(artifact_id)
list_artifacts_for_owner(owner, limit)
list_artifact_versions(artifact_id)
```

The first implementation should validate conversation owners against the
`conversations` table. Other allowed owner types may be listed before their
creation workflows exist, but creation should return an unsupported-owner error
unless the owner can be verified.

`create_artifact_with_initial_version` should create the artifact row and first
version in one SQLite transaction, then set `current_version_id`.

The first implementation may write local bytes outside the store module, but it
must insert metadata only after bytes are durably written. If metadata insertion
fails after a local file write, the caller should remove the just-written file
best effort and return the store error.

## GraphQL Contract

Expose artifacts as typed objects separate from transcript items:

```graphql
enum ArtifactStorageKind {
  LOCAL_FILE
  EXTERNAL_URL
}

type Artifact {
  artifactId: String!
  ownerObjectType: String!
  ownerObjectId: String!
  title: String!
  description: String
  artifactKind: String!
  storageKind: ArtifactStorageKind!
  currentVersion: ArtifactVersion
  versions: [ArtifactVersion!]!
  metadata: JSON!
}

type ArtifactVersion {
  artifactVersionId: String!
  artifactId: String!
  versionIndex: Int!
  title: String
  localRelativePath: String
  externalUrl: String
  downloadUrl: String
  mediaType: String
  byteSize: Int
  contentSha256: String
  metadata: JSON!
  createdAt: String!
}
```

Initial queries:

```graphql
artifacts(ownerObjectType: String!, ownerObjectId: String!, limit: Int): [Artifact!]!
artifact(artifactId: String!): Artifact
```

Initial mutation for external links:

```graphql
input CreateConversationExternalArtifactInput {
  conversationId: String!
  title: String!
  description: String
  artifactKind: String!
  externalUrl: String!
  mediaType: String
  sourceTurnId: String
  sourceItemId: String
  metadata: JSON
}
```

The mutation creates an artifact owned by the conversation and an initial
external URL version. It should validate the conversation exists, title is
non-empty, artifact kind is non-empty, and URL is HTTP or HTTPS.

Local-file creation is initially a backend command path, not a browser upload
mutation. This gives agents and internal tools a safe persistence primitive
without adding user upload policy and streaming concerns.

Local file versions expose a read-only `downloadUrl`:

```text
/artifacts/<artifact_version_id>/download
```

The route resolves the version through SQLite, verifies the version belongs to a
non-deleted artifact, resolves the stored relative path under `NoemaPaths`, and
streams the file bytes with `Content-Type` from `media_type` or
`application/octet-stream`. External URL versions return `null` for
`downloadUrl`; clients should use `externalUrl`.

## Transcript Integration

Transcript items should reference artifacts; they should not be the artifact
store.

Add a typed transcript union member for artifact references:

```graphql
type ArtifactReference {
  artifactId: String!
  artifactVersionId: String
  title: String!
  artifactKind: String!
  storageKind: ArtifactStorageKind!
  externalUrl: String
  mediaType: String
}
```

The durable conversation item can store `artifact_id` and optional
`artifact_version_id` in `payload_json`. Replay resolves display fields from the
artifact tables. If the artifact has been deleted, replay should render a small
missing-artifact notice rather than failing the whole transcript page.

The first implementation should use the typed `ArtifactReference` transcript
item rather than an `A2uiCard` schema. Artifacts are a product primitive, not
arbitrary card data.

## Agent-Facing Behavior

Agents should create artifacts through explicit Noema commands/tools, not by
inventing filesystem paths in assistant text.

The initial internal command should support:

- create local text/file artifact for a conversation;
- create external URL artifact for a conversation;
- append a new version to an existing artifact;
- return artifact id, version id, title, storage kind, and display/open target.

Future model-visible tool design can expose this through a governed native tool
such as `artifact.create` or through structured runtime actions. The first slice
does not need to make it model-visible if no runtime path is ready to call it.

## Error Handling

Local artifact writes fail before metadata insertion when path validation,
directory creation, byte write, or hashing fails.

External artifact creation fails before metadata insertion when URL validation
fails.

Store commands reject:

- missing owners for owner types the store can verify in the first slice;
- empty titles;
- empty artifact kinds;
- invalid storage kind strings;
- local versions without a local path;
- external versions without a URL;
- mixed local path and URL on one version;
- appending a version whose storage kind conflicts with the artifact's storage
  kind.

GraphQL resolvers should return ordinary GraphQL errors and must not append
transcript references when artifact creation fails.

## Testing

Backend tests should cover:

- schema creates artifact tables and indexes;
- create artifact with external URL initial version;
- create local file artifact metadata after writing bytes into a temporary
  Noema home;
- append version increments `version_index` and updates `current_version_id`;
- list artifacts by owner excludes soft-deleted rows;
- GraphQL creates a conversation external artifact and reads it back;
- invalid external URLs are rejected;
- path traversal in local filenames is rejected.

Frontend tests are not required for this slice unless a later implementation
adds substantial artifact UI behavior beyond a simple transcript row.

## Open Follow-Ups

- Project, task, workspace, human, and agent-run artifact creation surfaces.
- Browser upload routes with permission checks.
- Artifact promotion or copy between owners.
- External resource snapshot/import versions.
- Derived previews, search indexing, and vector indexing.
- A governed model-visible artifact tool contract.
