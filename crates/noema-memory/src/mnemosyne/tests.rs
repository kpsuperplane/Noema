use std::sync::Arc;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
};

use crate::{
    MemoryOperationError,
    model::{AddMemoryRequest, MemoryMessage, SearchMemoriesRequest},
};

#[tokio::test]
async fn mnemosyne_client_preserves_add_search_and_connection_contracts() {
    let server =
        FakeMnemosyneServer::start("POST", "/v1/memories/add", serde_json::json!({})).await;
    let client =
        super::client::MnemosyneClient::new(server.base_url(), Some("mnemosyne_test".to_string()));

    client
        .add_memory(AddMemoryRequest {
            messages: vec![MemoryMessage {
                role: "user".to_string(),
                content: "I love planes.".to_string(),
            }],
            user_id: "human:local".to_string(),
            agent_id: Some("agent:local".to_string()),
            run_id: Some("conv:local".to_string()),
            metadata: serde_json::json!({"userItemId": "item:1"}),
        })
        .await
        .expect("add memory");

    assert_eq!(
        server.last_authorization().await.as_deref(),
        Some("Bearer mnemosyne_test")
    );
    let request_body = server.last_body_json().await;
    assert_eq!(request_body["user_id"], "human:local");
    assert_eq!(request_body["agent_id"], "agent:local");
    assert_eq!(request_body["run_id"], "conv:local");
    assert_eq!(
        request_body["messages"],
        serde_json::json!([{"role": "user", "content": "I love planes."}])
    );
    let connection = super::MnemosyneConnection::new(
        "http://127.0.0.1:12345/".to_string(),
        Some("secret".to_string()),
    );
    assert_eq!(connection.base_url, "http://127.0.0.1:12345");
    assert_eq!(connection.api_key.as_deref(), Some("secret"));

    let server = FakeMnemosyneServer::start(
        "POST",
        "/v1/memories/search",
        serde_json::json!({
            "results": [{
                "id": "mem_1",
                "memory": "Kevin likes planes.",
                "score": 0.91,
                "metadata": {"sourceKind": "user_message"},
                "updated_at": "2026-07-08T12:00:00Z"
            }]
        }),
    )
    .await;
    let client = super::client::MnemosyneClient::new(server.base_url(), None);

    let response = client
        .search_memories(SearchMemoriesRequest {
            query: "planes".to_string(),
            user_id: "human:local".to_string(),
            agent_id: None,
            run_id: None,
            limit: 8,
        })
        .await
        .expect("search");

    assert_eq!(response.results[0].id, "mem_1");
    assert_eq!(response.results[0].score, Some(0.91));
    let request_body = server.last_body_json().await;
    assert_eq!(request_body["query"], "planes");
    assert_eq!(request_body["user_id"], "human:local");
    assert_eq!(request_body["limit"], 8);

    let server = FakeMnemosyneServer::health().await;
    super::client::MnemosyneClient::new(server.base_url(), None)
        .check_readiness()
        .await
        .expect("health readiness");
}

#[tokio::test]
async fn mnemosyne_client_times_out_when_server_never_responds() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buffer = [0_u8; 1024];
        let _ = stream.read(&mut buffer).await.expect("read request");
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    });
    let client = super::client::MnemosyneClient::new_with_request_timeout(
        base_url,
        None,
        std::time::Duration::from_millis(100),
    );

    let error = client
        .search_memories(SearchMemoriesRequest {
            query: "preferences".to_string(),
            user_id: "human:local".to_string(),
            agent_id: None,
            run_id: None,
            limit: 5,
        })
        .await
        .expect_err("request should time out");

    assert!(matches!(
        MemoryOperationError::from(error),
        MemoryOperationError::TimedOut
    ));
}

pub(super) struct FakeMnemosyneServer {
    base_url: String,
    state: Arc<Mutex<FakeMnemosyneState>>,
}

#[derive(Default)]
struct FakeMnemosyneState {
    authorization: Option<String>,
    body: Vec<u8>,
}

impl FakeMnemosyneServer {
    pub(super) async fn health() -> Self {
        Self::start("GET", "/health", serde_json::json!({"status": "ready"})).await
    }

    async fn start(
        expected_method: &'static str,
        expected_path: &'static str,
        response: serde_json::Value,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
        let state = Arc::new(Mutex::new(FakeMnemosyneState::default()));
        let server_state = Arc::clone(&state);

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let request = crate::model_proxy::protocol::read_http_request(&mut stream)
                .await
                .expect("request");
            assert_eq!(request.method, expected_method);
            assert_eq!(request.path, expected_path);

            *server_state.lock().await = FakeMnemosyneState {
                authorization: request.header("authorization").map(str::to_string),
                body: request.body,
            };

            let response =
                crate::model_proxy::protocol::json_response(http::StatusCode::OK, response);
            stream
                .write_all(&response.to_bytes())
                .await
                .expect("write response");
        });

        Self { base_url, state }
    }

    pub(super) fn base_url(&self) -> String {
        self.base_url.clone()
    }

    async fn last_authorization(&self) -> Option<String> {
        self.state.lock().await.authorization.clone()
    }

    async fn last_body_json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.state.lock().await.body).expect("request body JSON")
    }
}
