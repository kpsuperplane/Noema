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
            TurnStreamEvent::ConversationItem { .. }
            | TurnStreamEvent::AssistantTextDelta { .. }
            | TurnStreamEvent::AgentStatusChanged { .. } => None,
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
        TurnTranscriptItem::UserText { .. }
        | TurnTranscriptItem::MultipleChoicePrompt { .. }
        | TurnTranscriptItem::MultipleChoiceSelection { .. }
        | TurnTranscriptItem::Activity { .. }
        | TurnTranscriptItem::A2uiCard { .. }
        | TurnTranscriptItem::ErrorNotice { .. }
        | TurnTranscriptItem::ArtifactReference { .. }
        | TurnTranscriptItem::TaskReference { .. } => None,
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
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::test_support::test_store_for_paths(&paths).await;
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("provider account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate provider account");
    let configured_default = noema_providers::ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6-luna",
        None,
        Some("test_configured_default".to_string()),
    );
    let ready_selection = crate::test_support::ready_provider_selection(configured_default.clone());
    store
        .initialize_missing_provider_selections(ready_selection.selection(), Some(&ready_selection))
        .await
        .expect("initialize provider selections");
    let provider_registry = crate::test_support::ready_test_provider_registry();
    store
        .ensure_default_task_model_pool_settings_with_readiness("codex", provider_registry.as_ref())
        .await
        .expect("task model pool");
    std::mem::forget(home);
    let handle = RuntimeHandle::spawn_with_provider(Arc::new(provider), store.clone())
        .await
        .expect("runtime");
    (handle, store)
}

async fn test_runtime_handle_with_mnemosyne(
    provider: FakeCodexProvider,
    response: serde_json::Value,
) -> (RuntimeHandle, noema_store::NoemaStore, FakeMemoryServer) {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::test_support::test_store_for_paths(&paths).await;
    let server = FakeMemoryServer::start(response, 32).await;
    store
        .save_memory_service_settings(noema_memory::SaveMemoryServiceSettings {
            mode: noema_memory::MemoryServiceMode::External,
            base_url: Some(server.base_url()),
            port: None,
            provider_account_id: None,
            provider_kind: None,
            model_profile: None,
            reasoning_effort: None,
        })
        .await
        .expect("save memory settings");
    std::mem::forget(home);
    let memory_operations =
        crate::test_support::mnemosyne_operations_for_base_url(server.base_url());
    let handle = RuntimeHandle::spawn_with_provider_and_memory(
        Arc::new(provider),
        store.clone(),
        Some(memory_operations),
    )
    .await
    .expect("runtime");
    (handle, store, server)
}

async fn test_runtime_handle_with_private_memory(
    provider: FakeCodexProvider,
    response: serde_json::Value,
) -> (RuntimeHandle, noema_store::NoemaStore, FakeMemoryServer) {
    let store = crate::test_support::test_store().await;
    let server = FakeMemoryServer::start(response, 32).await;
    let memory_operations =
        crate::test_support::mnemosyne_operations_for_base_url(server.base_url());
    let handle = RuntimeHandle::spawn_with_provider_and_memory(
        Arc::new(provider),
        store.clone(),
        Some(memory_operations),
    )
    .await
    .expect("runtime");
    (handle, store, server)
}

async fn spawn_runtime_with_memory_provider(
    provider: noema_providers::ProviderHandle,
    response: serde_json::Value,
) -> (RuntimeHandle, noema_store::NoemaStore, FakeMemoryServer) {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::test_support::test_store_for_paths(&paths).await;
    store.ensure_default_actors().await.expect("actors");
    let server = FakeMemoryServer::start(response, 32).await;
    store
        .save_memory_service_settings(noema_memory::SaveMemoryServiceSettings {
            mode: noema_memory::MemoryServiceMode::External,
            base_url: Some(server.base_url()),
            port: None,
            provider_account_id: None,
            provider_kind: None,
            model_profile: None,
            reasoning_effort: None,
        })
        .await
        .expect("save memory settings");
    std::mem::forget(home);
    let memory_operations =
        crate::test_support::mnemosyne_operations_for_base_url(server.base_url());
    let handle = RuntimeHandle::spawn_with_provider_and_memory(
        provider,
        store.clone(),
        Some(memory_operations),
    )
    .await
    .expect("runtime");
    (handle, store, server)
}

async fn test_runtime_handle_with_search_provider(
    provider: noema_providers::ProviderHandle,
    search_provider: WebSearchBackendHandle,
) -> (RuntimeHandle, noema_store::NoemaStore) {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::test_support::test_store_for_paths(&paths).await;
    std::mem::forget(home);
    let handle = RuntimeHandle::spawn_with_provider_and_search_provider(
        provider,
        store.clone(),
        search_provider,
    )
    .await
    .expect("runtime");
    (handle, store)
}

struct FakeMemoryServer {
    base_url: String,
    state: Arc<AsyncMutex<FakeMemoryState>>,
    request_received: Arc<Notify>,
}

#[derive(Default)]
struct FakeMemoryState {
    bodies: Vec<serde_json::Value>,
    paths: Vec<String>,
}

impl FakeMemoryServer {
    async fn start(response: serde_json::Value, max_requests: usize) -> Self {
        Self::start_with_add_delay(response, max_requests, None).await
    }

    async fn start_with_add_delay(
        response: serde_json::Value,
        max_requests: usize,
        conversation_delay: Option<Duration>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
        let state = Arc::new(AsyncMutex::new(FakeMemoryState::default()));
        let server_state = Arc::clone(&state);
        let request_received = Arc::new(Notify::new());
        let server_request_received = Arc::clone(&request_received);

        tokio::spawn(async move {
            for _ in 0..max_requests {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let mut buffer = vec![0_u8; 8192];
                let read = stream.read(&mut buffer).await.expect("read");
                let request = String::from_utf8_lossy(&buffer[..read]);
                let Some((head, body)) = request.split_once("\r\n\r\n") else {
                    continue;
                };
                let mut lines = head.lines();
                let request_line = lines.next().expect("request line");
                let mut request_parts = request_line.split_whitespace();
                let method = request_parts.next().expect("request method");
                let path = request_parts.next().expect("request path").to_string();
                assert!(
                    matches!(
                        (method, path.as_str()),
                        ("POST", "/v1/memories/search" | "/v1/memories/add")
                    ) || matches!(
                        (method, path.as_str()),
                        ("GET", path) if path.starts_with("/v1/memories?")
                    )
                );

                let mut headers = HashMap::new();
                for line in lines {
                    if let Some((name, value)) = line.split_once(':') {
                        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
                    }
                }
                let content_length = headers
                    .get("content-length")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                let mut body_bytes = body.as_bytes().to_vec();
                while body_bytes.len() < content_length {
                    let read = stream.read(&mut buffer).await.expect("read body");
                    if read == 0 {
                        break;
                    }
                    body_bytes.extend_from_slice(&buffer[..read]);
                }
                let body_json = if body_bytes.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::from_slice(&body_bytes).expect("request body JSON")
                };
                {
                    let mut state = server_state.lock().await;
                    state.paths.push(path.clone());
                    state.bodies.push(body_json);
                }
                server_request_received.notify_one();

                let response_body = if path == "/v1/memories/add" {
                    if let Some(delay) = conversation_delay {
                        tokio::time::sleep(delay).await;
                    }
                    b"{}".to_vec()
                } else {
                    serde_json::to_vec(&response).expect("response JSON")
                };
                let response_head = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    response_body.len()
                );
                stream
                    .write_all(response_head.as_bytes())
                    .await
                    .expect("write head");
                stream.write_all(&response_body).await.expect("write body");
            }
        });

        Self {
            base_url,
            state,
            request_received,
        }
    }

    fn base_url(&self) -> String {
        self.base_url.clone()
    }

    async fn request_bodies(&self) -> Vec<serde_json::Value> {
        self.state.lock().await.bodies.clone()
    }

    async fn request_paths(&self) -> Vec<String> {
        self.state.lock().await.paths.clone()
    }

    async fn wait_for_request(&self) {
        self.request_received.notified().await;
    }
}

async fn wait_for_memory_observation_requests(server: &FakeMemoryServer, minimum_count: usize) {
    for _ in 0..50 {
        let bodies = server.request_bodies().await;
        let observations = bodies
            .iter()
            .filter(|body| body["metadata"]["sourceKind"] == "user_message")
            .count();
        if observations >= minimum_count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for {minimum_count} memory observation requests");
}

async fn test_runtime_handle_with_search_and_fetch_providers(
    provider: noema_providers::ProviderHandle,
    web_fetch_provider: WebFetchBackendHandle,
) -> (RuntimeHandle, noema_store::NoemaStore) {
    let search_provider =
        static_web_search_backend(noema_capabilities::web::search::SearchResponse {
            provider: "duckduckgo_public".to_string(),
            provider_contract: "best_effort_public".to_string(),
            query: String::new(),
            summary: "Found 0 web results".to_string(),
            results: Vec::new(),
        });
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::test_support::test_store_for_paths(&paths).await;
    std::mem::forget(home);
    let handle = RuntimeHandle::spawn_with_provider_and_search_fetch_providers(
        provider,
        store.clone(),
        search_provider,
        web_fetch_provider,
    )
    .await
    .expect("runtime");
    (handle, store)
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
    let author = match kind {
        ConversationItemKind::UserText => {
            ActorRef::human("human:local").expect("static local human actor id must be valid")
        }
        ConversationItemKind::MultipleChoiceSelection => {
            ActorRef::human("human:local").expect("static local human actor id must be valid")
        }
        ConversationItemKind::AssistantText => {
            ActorRef::agent("agent:primary").expect("static primary agent id must be valid")
        }
        ConversationItemKind::Activity
        | ConversationItemKind::A2uiCard
        | ConversationItemKind::MultipleChoicePrompt
        | ConversationItemKind::ToolCall
        | ConversationItemKind::ToolResult
        | ConversationItemKind::Reasoning
        | ConversationItemKind::ModelContextUpdate
        | ConversationItemKind::ApprovalRequest
        | ConversationItemKind::ApprovalResult
        | ConversationItemKind::ArtifactReference
        | ConversationItemKind::TaskReference
        | ConversationItemKind::ErrorNotice => {
            ActorRef::agent("agent:primary").expect("static primary agent id must be valid")
        }
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
