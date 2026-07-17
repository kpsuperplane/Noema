use super::*;

pub(super) async fn resolve_memory_service(state: &GraphqlState) -> Result<MemoryServiceSnapshot> {
    if let Some(access) = state.memory_service_access() {
        return access.resolve().await.map_err(graphql_error);
    }

    let settings = state
        .memory_repository()?
        .memory_service_settings()
        .await
        .map_err(graphql_error)?;
    Ok(MemoryServiceSnapshot::new(settings, None))
}

pub(super) fn memory_graph_error_status(
    error: &MemoryOperationError,
) -> Result<GraphqlMemoryServiceStatus> {
    let status = match error.code() {
        "auth_error" => GraphqlMemoryServiceStatusKind::AuthError,
        _ => GraphqlMemoryServiceStatusKind::Unavailable,
    };
    Ok(GraphqlMemoryServiceStatus {
        status,
        checked_at: Some(now_rfc3339()?),
        last_error_code: Some(error.code().to_string()),
        last_error_message: Some(error.to_string()),
    })
}

pub(super) async fn mnemosyne_memories_to_graph_documents(
    state: &GraphqlState,
    memories: Vec<MemoryRecord>,
) -> Result<Vec<GraphqlMemoryGraphDocument>> {
    let mut groups = BTreeMap::<String, Vec<MemoryRecord>>::new();
    for memory in memories {
        let document_id = memory
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("noemaConversationId"))
            .and_then(serde_json::Value::as_str)
            .map(|conversation_id| format!("conversation:{conversation_id}"))
            .unwrap_or_else(|| format!("mnemosyne:{HUMAN_MEMORY_SCOPE_ID}"));
        groups.entry(document_id).or_default().push(memory);
    }

    let mut documents = Vec::new();
    for (document_id, memories) in groups {
        let title = if document_id == format!("mnemosyne:{HUMAN_MEMORY_SCOPE_ID}") {
            "Human memory".to_string()
        } else {
            "Conversation memory".to_string()
        };
        let created_at = memories
            .iter()
            .filter_map(|memory| memory.created_at.as_deref())
            .min()
            .unwrap_or("")
            .to_string();
        let updated_at = memories
            .iter()
            .filter_map(|memory| memory.updated_at.as_deref())
            .max()
            .unwrap_or("")
            .to_string();
        let mut memory_entries = Vec::new();
        for memory in memories {
            let source = memory_source_from_metadata(state, memory.metadata.as_ref()).await;
            let metadata = memory.metadata.map(Json);
            let citation_key = memory_citation_key(&memory.id);
            memory_entries.push(GraphqlMemoryGraphMemoryEntry {
                id: memory.id,
                citation_key,
                document_id: document_id.clone(),
                content: memory.memory,
                summary: None,
                title: None,
                r#type: Some("memory".to_string()),
                source,
                metadata,
                created_at: memory.created_at.unwrap_or_default(),
                updated_at: memory.updated_at.unwrap_or_default(),
                space_container_tag: Some(HUMAN_MEMORY_SCOPE_ID.to_string()),
                relation: None,
                parent_memory_id: None,
                root_memory_id: None,
                memory_relations: None,
                is_latest: None,
                space_id: None,
            });
        }

        documents.push(GraphqlMemoryGraphDocument {
            id: document_id,
            custom_id: None,
            title: Some(title),
            content: None,
            summary: None,
            url: None,
            source: Some("mnemosyne".to_string()),
            r#type: Some("memory_group".to_string()),
            status: "ready".to_string(),
            metadata: None,
            created_at,
            updated_at,
            memory_entries,
        });
    }
    Ok(documents)
}

pub(super) async fn memory_source_from_metadata(
    state: &GraphqlState,
    metadata: Option<&serde_json::Value>,
) -> Option<GraphqlMemoryGraphMemorySource> {
    let metadata = metadata?;
    let kind = metadata_string(metadata, &["sourceKind", "source_kind"]);
    let conversation_id = metadata_string(metadata, &["noemaConversationId", "conversation_id"]);
    let turn_id = metadata_string(metadata, &["turnId", "turn_id"]);
    let item_id = metadata_string(metadata, &["userItemId", "source_item_id", "item_id"]);
    let source_observation =
        metadata_string(metadata, &["sourceObservation", "source_observation"]);

    let item = match (state.optional_store(), item_id.as_deref()) {
        (Some(store), Some(item_id)) => store
            .get_visible_conversation_item(item_id)
            .await
            .ok()
            .flatten(),
        _ => None,
    };
    let message_text = item
        .as_ref()
        .and_then(|item| item.content_text.clone())
        .filter(|text| !text.trim().is_empty())
        .or(source_observation);

    if kind.is_none()
        && conversation_id.is_none()
        && turn_id.is_none()
        && item_id.is_none()
        && message_text.is_none()
    {
        return None;
    }

    Some(GraphqlMemoryGraphMemorySource {
        kind,
        conversation_id: item
            .as_ref()
            .map(|item| item.conversation_id.clone())
            .or(conversation_id),
        turn_id: item
            .as_ref()
            .and_then(|item| item.turn_id.clone())
            .or(turn_id),
        item_id: item.as_ref().map(|item| item.item_id.clone()).or(item_id),
        message_text,
    })
}

pub(super) fn metadata_string(metadata: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| metadata.get(*key).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}
