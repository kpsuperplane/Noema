package store

const schemaV11SQL = `
CREATE TABLE artifacts (
    artifact_id TEXT PRIMARY KEY
        CHECK (length(artifact_id) = 41 AND substr(artifact_id, 1, 9) = 'artifact:'),
    owner_object_type TEXT NOT NULL CHECK (owner_object_type IN ('conversation', 'task')),
    owner_object_id TEXT NOT NULL CHECK (length(trim(owner_object_id)) > 0),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    description TEXT,
    artifact_kind TEXT NOT NULL CHECK (length(trim(artifact_kind)) > 0),
    storage_kind TEXT NOT NULL CHECK (storage_kind IN ('local_file', 'external_url')),
    current_version_id TEXT,
    created_by_actor_id TEXT NOT NULL CHECK (length(trim(created_by_actor_id)) > 0),
    source_conversation_id TEXT,
    source_turn_id TEXT,
    source_item_id TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
    deleted_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE artifact_versions (
    artifact_version_id TEXT PRIMARY KEY
        CHECK (length(artifact_version_id) = 49 AND substr(artifact_version_id, 1, 17) = 'artifact_version:'),
    artifact_id TEXT NOT NULL REFERENCES artifacts(artifact_id) ON DELETE CASCADE,
    version_index INTEGER NOT NULL CHECK (version_index > 0),
    title TEXT,
    local_relative_path TEXT,
    external_url TEXT,
    media_type TEXT,
    byte_size INTEGER CHECK (byte_size IS NULL OR byte_size >= 0),
    content_sha256 TEXT CHECK (
        content_sha256 IS NULL OR (length(content_sha256) = 64 AND content_sha256 = lower(content_sha256))
    ),
    created_by_actor_id TEXT NOT NULL CHECK (length(trim(created_by_actor_id)) > 0),
    source_conversation_id TEXT,
    source_turn_id TEXT,
    source_item_id TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (artifact_id, version_index),
    CHECK ((local_relative_path IS NOT NULL) <> (external_url IS NOT NULL))
) STRICT;

CREATE INDEX artifacts_owner_current
ON artifacts(owner_object_type, owner_object_id, updated_at_ms DESC, artifact_id DESC)
WHERE deleted_at_ms IS NULL;

CREATE INDEX artifact_versions_artifact
ON artifact_versions(artifact_id, version_index);
`
