#[derive(Debug)]
pub(super) struct MemoryArticleTestProvider {
    pub(super) text: String,
}

impl noema_providers::ModelProvider for MemoryArticleTestProvider {
    async fn generate(
        &self,
        _request: noema_providers::GenerateRequest,
    ) -> Result<noema_providers::GenerateResponse, noema_providers::ProviderError> {
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

}

pub(super) async fn test_memory_article_runtime(
    store: noema_store::NoemaStore,
    text: &str,
) -> noema_runtime::RuntimeHandle {
    let provider = noema_providers::erase_model_provider(MemoryArticleTestProvider {
        text: text.to_string(),
    });
    crate::test_support::spawn_runtime_with_provider(provider, store)
        .await
        .expect("runtime")
}
