use std::{collections::HashMap, sync::Arc};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
};

#[tokio::test]
async fn supermemory_search_posts_v4_search() {
    let server = FakeSupermemoryServer::start(
        "/v4/search",
        serde_json::json!({
            "results": [{
                "id": "mem_1",
                "memory": "Kevin prefers concise plans",
                "metadata": {"source": "test"},
                "updatedAt": "2026-07-07T12:00:00.000Z",
                "similarity": 0.91
            }],
            "timing": 2,
            "total": 1
        }),
    )
    .await;
    let client = crate::SupermemoryClient::new(server.base_url(), Some("sm_test".to_string()));

    let response = client
        .search_memories(crate::SupermemorySearchRequest {
            query: "preferences".to_string(),
            container_tag: "human:local".to_string(),
            limit: 5,
        })
        .await
        .expect("search");

    assert_eq!(response.results[0].id, "mem_1");
    assert_eq!(
        server.last_authorization().await.as_deref(),
        Some("Bearer sm_test")
    );
    let request_body = server.last_body_json().await;
    assert_eq!(request_body["q"], "preferences");
    assert_eq!(request_body["containerTag"], "human:local");
    assert_eq!(request_body["limit"], 5);
    assert_eq!(request_body["searchMode"], "memories");
    assert_eq!(request_body["include"]["documents"], false);
}

#[tokio::test]
async fn supermemory_ingest_posts_v4_conversations_camel_case_shape() {
    let server =
        FakeSupermemoryServer::start("/v4/conversations", serde_json::json!({"ok": true})).await;
    let client = crate::SupermemoryClient::new(server.base_url(), None);

    client
        .ingest_conversation(crate::SupermemoryConversationIngestRequest::new(
            "conversation:local",
            "conversation_local",
            vec![
                crate::supermemory::SupermemoryConversationMessage {
                    role: "user".to_string(),
                    content: "Kevin likes local-first tools".to_string(),
                },
                crate::supermemory::SupermemoryConversationMessage {
                    role: "assistant".to_string(),
                    content: "Noted".to_string(),
                },
            ],
        ))
        .await
        .expect("ingest");

    let request_body = server.last_body_json().await;
    assert_eq!(request_body["conversationId"], "conversation:local");
    assert_eq!(request_body["containerTag"], "conversation_local");
    assert_eq!(
        request_body["containerTags"],
        serde_json::json!(["conversation_local"])
    );
    assert_eq!(request_body["messages"][0]["role"], "user");
    assert_eq!(
        request_body["messages"][0]["content"],
        "Kevin likes local-first tools"
    );
    assert!(request_body.get("conversation_id").is_none());
    assert!(request_body.get("container_tag").is_none());
    assert!(request_body.get("payload").is_none());
}

#[tokio::test]
async fn supermemory_graph_documents_posts_v3_documents_documents() {
    let server = FakeSupermemoryServer::start(
        "/v3/documents/documents",
        serde_json::json!({
            "documents": [{
                "id": "doc_1",
                "customId": "human-profile",
                "title": "Human profile",
                "content": "Kevin likes local-first tools",
                "summary": "Preference summary",
                "url": null,
                "source": "noema",
                "type": "note",
                "status": "done",
                "metadata": {"scope": "human"},
                "createdAt": "2026-07-08T00:00:00.000Z",
                "updatedAt": "2026-07-08T00:01:00.000Z",
                "memoryEntries": [{
                    "id": "mem_1",
                    "documentId": "doc_1",
                    "content": "Kevin prefers local-first tools",
                    "summary": "Local-first preference",
                    "title": "Preference",
                    "type": "fact",
                    "metadata": {"confidence": 0.9},
                    "createdAt": "2026-07-08T00:00:30.000Z",
                    "updatedAt": "2026-07-08T00:01:00.000Z",
                    "spaceContainerTag": "human:local",
                    "relation": "extends",
                    "isLatest": true,
                    "spaceId": "human:local"
                }]
            }],
            "pagination": {"page": 1, "limit": 25, "hasMore": true, "total": 42}
        }),
    )
    .await;
    let client = crate::SupermemoryClient::new(server.base_url(), Some("sm_test".to_string()));

    let response = client
        .list_memory_graph_documents(crate::SupermemoryGraphDocumentsRequest {
            container_tag: "human:local".to_string(),
            page: 1,
            limit: 25,
        })
        .await
        .expect("graph documents");

    assert_eq!(response.documents[0].id, "doc_1");
    assert_eq!(response.documents[0].memory_entries[0].id, "mem_1");
    assert_eq!(response.pagination.has_more, Some(true));
    assert_eq!(
        server.last_authorization().await.as_deref(),
        Some("Bearer sm_test")
    );
    let request_body = server.last_body_json().await;
    assert_eq!(request_body["containerTag"], "human:local");
    assert_eq!(
        request_body["containerTags"],
        serde_json::json!(["human:local"])
    );
    assert_eq!(request_body["page"], 1);
    assert_eq!(request_body["limit"], 25);
    assert_eq!(request_body["sort"], "createdAt");
    assert_eq!(request_body["order"], "desc");
}

#[tokio::test]
async fn supermemory_graph_documents_decodes_memory_graph_package_shape() {
    let server = FakeSupermemoryServer::start(
        "/v3/documents/documents",
        serde_json::json!({
            "documents": [{
                "id": "doc_1",
                "title": "Human profile",
                "summary": "Preference summary",
                "documentType": "note",
                "status": "done",
                "createdAt": "2026-07-08T00:00:00.000Z",
                "updatedAt": "2026-07-08T00:01:00.000Z",
                "memories": [{
                    "id": "mem_1",
                    "documentId": "doc_1",
                    "memory": "Kevin prefers local-first tools",
                    "createdAt": "2026-07-08T00:00:30.000Z",
                    "updatedAt": "2026-07-08T00:01:00.000Z",
                    "spaceContainerTag": "human:local",
                    "parentMemoryId": "mem_root",
                    "rootMemoryId": "mem_root",
                    "memoryRelations": {"mem_root": "extends"}
                }]
            }],
            "pagination": {"currentPage": 2, "limit": 25, "totalItems": 42}
        }),
    )
    .await;
    let client = crate::SupermemoryClient::new(server.base_url(), None);

    let response = client
        .list_memory_graph_documents(crate::SupermemoryGraphDocumentsRequest {
            container_tag: "human:local".to_string(),
            page: 2,
            limit: 25,
        })
        .await
        .expect("graph documents");

    assert_eq!(response.documents[0].r#type.as_deref(), Some("note"));
    assert_eq!(
        response.documents[0].memory_entries[0].content.as_deref(),
        Some("Kevin prefers local-first tools")
    );
    assert_eq!(
        response.documents[0].memory_entries[0]
            .parent_memory_id
            .as_deref(),
        Some("mem_root")
    );
    assert_eq!(
        response.documents[0].memory_entries[0]
            .root_memory_id
            .as_deref(),
        Some("mem_root")
    );
    assert_eq!(
        response.documents[0].memory_entries[0].memory_relations,
        Some(serde_json::json!({"mem_root": "extends"}))
    );
    assert_eq!(response.pagination.page, Some(2));
    assert_eq!(response.pagination.total, Some(42));
}

#[tokio::test]
async fn supermemory_client_times_out_when_server_never_responds() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buffer = [0_u8; 1024];
        let _ = stream.read(&mut buffer).await.expect("read request");
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    });
    let client = crate::SupermemoryClient::new(base_url, None);

    let error = client
        .search_memories(crate::SupermemorySearchRequest {
            query: "preferences".to_string(),
            container_tag: "human:local".to_string(),
            limit: 5,
        })
        .await
        .expect_err("request should time out");

    assert_eq!(error.sanitized_code(), "timeout");
    assert_eq!(
        error.sanitized_message(),
        "memory service request timed out"
    );
}

struct FakeSupermemoryServer {
    base_url: String,
    state: Arc<Mutex<FakeSupermemoryState>>,
}

#[derive(Default)]
struct FakeSupermemoryState {
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl FakeSupermemoryServer {
    async fn start(expected_path: &'static str, response: serde_json::Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
        let state = Arc::new(Mutex::new(FakeSupermemoryState::default()));
        let server_state = Arc::clone(&state);

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buffer = vec![0_u8; 8192];
            let read = stream.read(&mut buffer).await.expect("read");
            let request = String::from_utf8_lossy(&buffer[..read]);
            let (head, body) = request.split_once("\r\n\r\n").expect("request head");
            let mut lines = head.lines();
            let request_line = lines.next().expect("request line");
            assert_eq!(request_line, format!("POST {expected_path} HTTP/1.1"));

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

            *server_state.lock().await = FakeSupermemoryState {
                headers,
                body: body_bytes,
            };

            let response_body = serde_json::to_vec(&response).expect("response JSON");
            let response_head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
                response_body.len()
            );
            stream
                .write_all(response_head.as_bytes())
                .await
                .expect("write head");
            stream.write_all(&response_body).await.expect("write body");
        });

        Self { base_url, state }
    }

    fn base_url(&self) -> String {
        self.base_url.clone()
    }

    async fn last_authorization(&self) -> Option<String> {
        self.state
            .lock()
            .await
            .headers
            .get("authorization")
            .cloned()
    }

    async fn last_body_json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.state.lock().await.body).expect("request body JSON")
    }
}
