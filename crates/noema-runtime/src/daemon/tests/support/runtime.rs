async fn collect_turn(
    handle: &RuntimeHandle,
    conversation_id: String,
    input: String,
) -> Result<Vec<TurnTranscriptItem>, RuntimeError> {
    let (result, events) = collect_turn_events(handle, conversation_id, input).await;
    result?;
    Ok(transcript_items_from_events(events))
}

async fn collect_turn_events(
    handle: &RuntimeHandle,
    conversation_id: String,
    input: String,
) -> (Result<(), RuntimeError>, Vec<TurnStreamEvent>) {
    let (item_tx, mut item_rx) = mpsc::unbounded_channel();
    let result = handle.turn(conversation_id, input, item_tx).await;
    let mut events = Vec::new();
    while let Ok(event) = item_rx.try_recv() {
        events.push(event);
    }
    (result, events)
}

fn transcript_items_from_events(events: Vec<TurnStreamEvent>) -> Vec<TurnTranscriptItem> {
    events
        .into_iter()
        .filter_map(|event| match event {
            TurnStreamEvent::ConversationItem { item, .. }
                if !matches!(item.as_ref(), TurnTranscriptItem::UserText { .. }) =>
            {
                Some(*item)
            }
            _ => None,
        })
        .collect()
}

fn assistant_text_item_event_index(
    events: &[TurnStreamEvent],
    expected_text: &str,
) -> Option<usize> {
    events.iter().position(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem { item, .. }
                if matches!(
                    item.as_ref(),
                    TurnTranscriptItem::AssistantText { text } if text == expected_text
                )
        )
    })
}
fn assistant_text(items: &[TurnTranscriptItem]) -> &str {
    let Some(text) = items.iter().find_map(|item| match item {
        TurnTranscriptItem::AssistantText { text } => Some(text.as_str()),
        _ => None,
    }) else {
        panic!("expected assistant text item, got {items:?}");
    };
    text
}

async fn test_runtime_handle(provider: FakeCodexProvider) -> RuntimeHandle {
    test_runtime_handle_with_store(provider).await.0
}

async fn authenticate_provider_account(store: &noema_store::NoemaStore, account_id: &str) {
    store
        .update_provider_account_status(
            account_id,
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate provider account");
}

async fn test_runtime_handle_with_store(
    provider: FakeCodexProvider,
) -> (RuntimeHandle, noema_store::NoemaStore) {
    let (handle, store, _system_errors) =
        test_runtime_handle_with_store_and_system_errors(provider).await;
    (handle, store)
}

async fn test_runtime_handle_with_store_and_system_errors(
    provider: FakeCodexProvider,
) -> (
    RuntimeHandle,
    noema_store::NoemaStore,
    noema_home::SystemErrorLogger,
) {
    let store = crate::test_support::test_store().await;
    let system_errors = crate::test_support::system_error_logger();
    let handle = RuntimeHandle::spawn_with_provider_map_and_memory(
        "codex".to_string(),
        HashMap::from([(
            "codex".to_string(),
            Arc::new(provider) as noema_providers::ProviderHandle,
        )]),
        store.clone(),
        crate::test_support::artifact_operations(&store).expect("artifact operations"),
        system_errors.clone(),
        None,
        crate::daemon::RuntimeEventRegistry::default(),
    )
    .await
    .expect("runtime");
    (handle, store, system_errors)
}

async fn test_runtime_handle_with_task_delegation(
    provider: FakeCodexProvider,
) -> (RuntimeHandle, noema_store::NoemaStore) {
    let store = crate::test_support::test_store().await;
    crate::test_support::initialize_codex_provider_selections(&store).await;
    let provider_registry = crate::test_support::ready_test_provider_registry();
    store
        .ensure_default_task_model_pool_settings_with_readiness("codex", provider_registry.as_ref())
        .await
        .expect("task model pool");
    let handle = RuntimeHandle::spawn_with_provider(Arc::new(provider), store.clone())
        .await
        .expect("runtime");
    (handle, store)
}

async fn test_runtime_handle_with_mnemosyne(
    provider: FakeCodexProvider,
    memory: Arc<RecordingMemoryOperations>,
) -> (
    RuntimeHandle,
    noema_store::NoemaStore,
    Arc<RecordingMemoryOperations>,
) {
    spawn_runtime_with_memory_provider(Arc::new(provider), memory).await
}

async fn spawn_runtime_with_memory_provider(
    provider: noema_providers::ProviderHandle,
    memory: Arc<RecordingMemoryOperations>,
) -> (
    RuntimeHandle,
    noema_store::NoemaStore,
    Arc<RecordingMemoryOperations>,
) {
    let store = crate::test_support::test_store().await;
    let handle = RuntimeHandle::spawn_with_provider_and_memory(
        provider,
        store.clone(),
        Some(memory.clone()),
    )
    .await
    .expect("runtime");
    (handle, store, memory)
}

#[derive(Debug)]
struct RecordingMemoryOperations {
    search_response: SearchMemoriesResponse,
    search_requests: AsyncMutex<Vec<SearchMemoriesRequest>>,
    add_requests: AsyncMutex<Vec<AddMemoryRequest>>,
    add_started: Notify,
    block_add: bool,
}

impl Default for RecordingMemoryOperations {
    fn default() -> Self {
        Self::returning(SearchMemoriesResponse::default())
    }
}

impl RecordingMemoryOperations {
    fn returning(search_response: SearchMemoriesResponse) -> Self {
        Self {
            search_response,
            search_requests: AsyncMutex::new(Vec::new()),
            add_requests: AsyncMutex::new(Vec::new()),
            add_started: Notify::new(),
            block_add: false,
        }
    }

    fn blocking_add() -> Self {
        Self {
            block_add: true,
            ..Self::default()
        }
    }

    async fn add_requests(&self) -> Vec<AddMemoryRequest> {
        self.add_requests.lock().await.clone()
    }

    async fn search_requests(&self) -> Vec<SearchMemoriesRequest> {
        self.search_requests.lock().await.clone()
    }

    async fn wait_for_add(&self) {
        while self.add_requests.lock().await.is_empty() {
            self.add_started.notified().await;
        }
    }
}

impl MemoryOperations for RecordingMemoryOperations {
    fn check_readiness(&self) -> MemoryOperationFuture<'_, MemoryServiceReadiness> {
        Box::pin(async { Ok(MemoryServiceReadiness { ready: true }) })
    }

    fn add_memory(&self, request: AddMemoryRequest) -> MemoryOperationFuture<'_, ()> {
        Box::pin(async move {
            self.add_requests.lock().await.push(request);
            self.add_started.notify_waiters();
            if self.block_add {
                std::future::pending().await
            }
            Ok(())
        })
    }

    fn search_memories(
        &self,
        request: SearchMemoriesRequest,
    ) -> MemoryOperationFuture<'_, SearchMemoriesResponse> {
        let response = self.search_response.clone();
        Box::pin(async move {
            self.search_requests.lock().await.push(request);
            Ok(response)
        })
    }

    fn list_memories(
        &self,
        _request: ListMemoriesRequest,
    ) -> MemoryOperationFuture<'_, ListMemoriesResponse> {
        Box::pin(async { Ok(ListMemoriesResponse::default()) })
    }
}

async fn append_test_text_item(store: &noema_store::NoemaStore, conversation_id: &str, text: &str) {
    append_test_text_item_with_kind(store, conversation_id, ConversationItemKind::UserText, text)
        .await;
}

async fn append_test_text_item_with_kind(
    store: &noema_store::NoemaStore,
    conversation_id: &str,
    kind: ConversationItemKind,
    text: &str,
) {
    let author = if matches!(
        kind,
        ConversationItemKind::UserText | ConversationItemKind::MultipleChoiceSelection
    ) {
        ActorRef::human("human:local").expect("valid static human id")
    } else {
        ActorRef::agent("agent:primary").expect("valid static agent id")
    };
    store
        .append_conversation_item(noema_conversations::NewConversationItem {
            conversation_id: conversation_id.to_string(),
            turn_id: None,
            parent_item_id: None,
            kind,
            status: ConversationItemStatus::Completed,
            author,
            content_text: Some(text.to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("append item");
}

async fn append_test_multiple_choice_prompt(
    store: &noema_store::NoemaStore,
    conversation_id: &str,
    selection_mode: MultipleChoiceSelectionMode,
) -> String {
    let record = store
        .append_conversation_item(noema_conversations::NewConversationItem {
            conversation_id: conversation_id.to_string(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::MultipleChoicePrompt,
            status: ConversationItemStatus::Completed,
            author: ActorRef::agent("agent:primary")
                .expect("static primary agent id must be valid"),
            content_text: Some("Pick a direction".to_string()),
            payload_json: json!({
                "prompt": "Pick a direction",
                "selection_mode": selection_mode,
                "options": [
                    {"id": "ship", "label": "Ship it"},
                    {"id": "polish", "label": "Polish first"}
                ],
            }),
            metadata: json!({}),
        })
        .await
        .expect("append multiple-choice prompt");
    record.item_id
}

async fn wait_for_context_summary_count(
    store: &noema_store::NoemaStore,
    conversation_id: &str,
    minimum_count: usize,
) {
    for _ in 0..50 {
        let summaries = store
            .list_context_summaries_for_conversation(conversation_id)
            .await
            .expect("summaries");
        if summaries.len() >= minimum_count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for {minimum_count} context summaries");
}
