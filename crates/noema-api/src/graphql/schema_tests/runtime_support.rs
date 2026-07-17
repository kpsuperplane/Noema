#[derive(Debug)]
pub(super) struct AutofillTestProvider {
    pub(super) text: String,
    pub(super) tool_classification_model: Option<String>,
    pub(super) requests: Arc<Mutex<Vec<noema_providers::GenerateRequest>>>,
}

impl noema_providers::ModelProvider for AutofillTestProvider {
    async fn generate(
        &self,
        request: noema_providers::GenerateRequest,
    ) -> Result<noema_providers::GenerateResponse, noema_providers::ProviderError> {
        self.requests.lock().expect("requests").push(request);
        Ok(noema_providers::GenerateResponse {
            responses: vec![noema_providers::GenerateResponseItem::Text {
                phase: None,
                text: self.text.clone(),
            }],
            tool_calls: Vec::new(),
            reasoning_items: Vec::new(),
            response_status: noema_providers::GenerateResponseStatus::Final,
            provider: "test".to_string(),
            model: "test-autofill".to_string(),
            response_id: None,
            usage: None,
        })
    }

    fn default_tool_classification_model(&self) -> Option<String> {
        self.tool_classification_model.clone()
    }
}

pub(super) async fn test_autofill_runtime(
    store: noema_store::NoemaStore,
    text: &str,
) -> noema_runtime::RuntimeHandle {
    let (_runtime, runtime) =
        test_autofill_runtime_with_requests(store, text, Some("test-tool-classifier")).await;
    runtime
}

pub(super) async fn test_autofill_runtime_with_requests(
    store: noema_store::NoemaStore,
    text: &str,
    tool_classification_model: Option<&str>,
) -> (
    Arc<Mutex<Vec<noema_providers::GenerateRequest>>>,
    noema_runtime::RuntimeHandle,
) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = noema_providers::erase_model_provider(AutofillTestProvider {
        text: text.to_string(),
        tool_classification_model: tool_classification_model.map(str::to_string),
        requests: requests.clone(),
    });
    let runtime = crate::test_support::spawn_runtime_with_provider(provider, store)
        .await
        .expect("runtime");
    (requests, runtime)
}
