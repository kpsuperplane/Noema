use std::{collections::HashMap, sync::Arc};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
};

#[tokio::test]
async fn mnemosyne_client_add_posts_v1_memories_add() {
    let server =
        FakeMnemosyneServer::start("POST", "/v1/memories/add", serde_json::json!({})).await;
    let client = crate::MnemosyneClient::new(server.base_url(), Some("mnemosyne_test".to_string()));

    client
        .add_memory(crate::MnemosyneAddMemoryRequest {
            messages: vec![crate::MnemosyneMessage {
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
}

#[tokio::test]
async fn mnemosyne_client_search_posts_v1_memories_search() {
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
    let client = crate::MnemosyneClient::new(server.base_url(), None);

    let response = client
        .search_memories(crate::MnemosyneSearchRequest {
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
    let client = crate::MnemosyneClient::new_with_request_timeout(
        base_url,
        None,
        std::time::Duration::from_millis(100),
    );

    let error = client
        .search_memories(crate::MnemosyneSearchRequest {
            query: "preferences".to_string(),
            user_id: "human:local".to_string(),
            agent_id: None,
            run_id: None,
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

struct FakeMnemosyneServer {
    base_url: String,
    state: Arc<Mutex<FakeMnemosyneState>>,
}

#[derive(Default)]
struct FakeMnemosyneState {
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl FakeMnemosyneServer {
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
            let mut buffer = vec![0_u8; 8192];
            let read = stream.read(&mut buffer).await.expect("read");
            let request = String::from_utf8_lossy(&buffer[..read]);
            let (head, body) = request.split_once("\r\n\r\n").expect("request head");
            let mut lines = head.lines();
            let request_line = lines.next().expect("request line");
            assert_eq!(
                request_line,
                format!("{expected_method} {expected_path} HTTP/1.1")
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

            *server_state.lock().await = FakeMnemosyneState {
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
