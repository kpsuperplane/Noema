use thiserror::Error;

// V1 keeps the provider contract as a native async trait and does not expose
// `dyn ModelProvider`, so the public future-bound tradeoff is intentional.
#[allow(async_fn_in_trait)]
pub trait ModelProvider: Send + Sync {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenerateRequest {
    pub model: Option<String>,
    pub input: GenerateInput,
    pub instructions: Option<String>,
    pub options: GenerateOptions,
}

impl GenerateRequest {
    pub fn text(input: impl Into<String>) -> Self {
        Self {
            model: None,
            input: GenerateInput::Text(input.into()),
            instructions: None,
            options: GenerateOptions::default(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenerateInput {
    Text(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GenerateOptions {
    pub max_output_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenerateResponse {
    pub text: String,
    pub provider: String,
    pub model: String,
    pub response_id: Option<String>,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("missing credentials for {provider}: {credential}")]
    MissingCredentials {
        provider: String,
        credential: String,
    },

    #[error("invalid request: {message}")]
    InvalidRequest { message: String },

    #[error("HTTP request failed: {source}")]
    HttpFailure {
        #[from]
        source: reqwest::Error,
    },

    #[error("provider API error ({status}): {message}")]
    ApiError {
        status: u16,
        message: String,
        request_id: Option<String>,
    },

    #[error("provider rate limit: {message}")]
    RateLimit {
        message: String,
        request_id: Option<String>,
    },

    #[error("provider authentication failed: {message}")]
    AuthenticationFailure {
        message: String,
        request_id: Option<String>,
    },

    #[error("malformed provider response: {message}")]
    MalformedResponse { message: String },

    #[error("{provider} provider protocol error: {message}")]
    ProtocolError { provider: String, message: String },

    #[error("{provider} provider timed out during {operation} after {seconds} seconds")]
    Timeout {
        provider: String,
        operation: String,
        seconds: u64,
    },

    #[error("unsupported provider feature: {feature}")]
    UnsupportedFeature { feature: String },

    #[error("{provider} provider is unavailable: {message}")]
    ProviderUnavailable { provider: String, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoProvider;

    impl ModelProvider for EchoProvider {
        async fn generate(
            &self,
            request: GenerateRequest,
        ) -> Result<GenerateResponse, ProviderError> {
            let GenerateInput::Text(text) = request.input;

            Ok(GenerateResponse {
                text,
                provider: "mock".to_string(),
                model: request.model.unwrap_or_else(|| "mock-model".to_string()),
                response_id: Some("mock-response".to_string()),
                usage: Some(TokenUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                    total_tokens: 2,
                }),
            })
        }
    }

    #[tokio::test]
    async fn model_provider_contract_can_be_implemented_by_a_mock() {
        let provider = EchoProvider;
        let response = provider
            .generate(GenerateRequest::text("hello").with_model("mock-1"))
            .await
            .expect("mock provider should return a response");

        assert_eq!(response.text, "hello");
        assert_eq!(response.provider, "mock");
        assert_eq!(response.model, "mock-1");
    }
}
