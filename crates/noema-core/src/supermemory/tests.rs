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
