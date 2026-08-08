mod queries;
mod rows;

use super::{NoemaStore, StoreError};

pub(super) use rows::{
    ARTIFACT_SELECT, ARTIFACT_VERSION_SELECT, artifact_from_row, artifact_row, artifact_row_at,
    artifact_version_from_row, artifact_version_row, artifact_version_row_at,
};

#[derive(Debug)]
pub(super) struct ArtifactRow {
    artifact_id: String,
    owner_object_type: String,
    owner_object_id: String,
    title: String,
    description: Option<String>,
    artifact_kind: String,
    storage_kind: String,
    current_version_id: Option<String>,
    created_by_actor_id: String,
    source_conversation_id: Option<String>,
    source_turn_id: Option<String>,
    source_item_id: Option<String>,
    metadata_json: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug)]
pub(super) struct ArtifactVersionRow {
    artifact_version_id: String,
    artifact_id: String,
    version_index: i64,
    title: Option<String>,
    local_relative_path: Option<String>,
    external_url: Option<String>,
    media_type: Option<String>,
    byte_size: Option<i64>,
    content_sha256: Option<String>,
    created_by_actor_id: String,
    source_conversation_id: Option<String>,
    source_turn_id: Option<String>,
    source_item_id: Option<String>,
    metadata_json: String,
    created_at: String,
}
