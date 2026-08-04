use noema_providers::{GenerateInput, ProviderContextMetadata, ProviderOperations, ProviderTool};

const DEFAULT_CONTEXT_SAFETY_TOKENS: u32 = 128;
const FALLBACK_CHARS_PER_TOKEN: usize = 3;
const COMPACTION_THRESHOLD_NUMERATOR: u32 = 7;
const COMPACTION_THRESHOLD_DENOMINATOR: u32 = 10;
const HOSTED_SEARCH_OVERHEAD_TOKENS: u32 = 256;
const RECENT_SUFFIX_NUMERATOR: u32 = 1;
const RECENT_SUFFIX_DENOMINATOR: u32 = 5;
const RECENT_SUFFIX_MAX_TOKENS: u32 = 8_192;

pub(super) struct RequestContext<'a> {
    pub(super) model: Option<&'a str>,
    pub(super) instructions: Option<&'a str>,
    pub(super) input: &'a GenerateInput,
    pub(super) tools: &'a [ProviderTool],
    pub(super) hosted_web_search: bool,
    pub(super) output_reserve_tokens: Option<u32>,
    pub(super) has_compactable_history: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContextAdmission {
    Ready {
        estimated_input_tokens: u32,
        available_input_tokens: Option<u32>,
    },
    CompactablePressure {
        estimated_input_tokens: u32,
        available_input_tokens: u32,
    },
    HardOverflowWithCompactableHistory {
        estimated_input_tokens: u32,
        available_input_tokens: u32,
    },
    HardOverflowWithOnlyActiveContext {
        estimated_input_tokens: u32,
        available_input_tokens: u32,
    },
}

impl ContextAdmission {
    pub(super) const fn requires_compaction(self) -> bool {
        matches!(
            self,
            Self::CompactablePressure { .. } | Self::HardOverflowWithCompactableHistory { .. }
        )
    }

    pub(super) const fn estimated_input_tokens(self) -> u32 {
        match self {
            Self::Ready {
                estimated_input_tokens,
                ..
            }
            | Self::CompactablePressure {
                estimated_input_tokens,
                ..
            }
            | Self::HardOverflowWithCompactableHistory {
                estimated_input_tokens,
                ..
            }
            | Self::HardOverflowWithOnlyActiveContext {
                estimated_input_tokens,
                ..
            } => estimated_input_tokens,
        }
    }
}

/// Prompt budget derived from provider context metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ContextBudget {
    context_window_tokens: Option<u32>,
    output_reserve_tokens: Option<u32>,
    safety_tokens: u32,
    compact_summary_target_tokens: Option<u32>,
}

impl ContextBudget {
    pub(super) fn from_metadata(metadata: ProviderContextMetadata) -> Self {
        Self {
            context_window_tokens: metadata.context_window_tokens,
            output_reserve_tokens: metadata.default_output_reserve_tokens,
            safety_tokens: DEFAULT_CONTEXT_SAFETY_TOKENS,
            compact_summary_target_tokens: metadata.compact_summary_target_tokens,
        }
    }

    pub(super) const fn output_reserve_tokens(self) -> Option<u32> {
        self.output_reserve_tokens
    }

    pub(super) const fn compact_summary_target_tokens(self) -> Option<u32> {
        self.compact_summary_target_tokens
    }

    pub(super) fn effective_output_reserve_tokens(self, requested: Option<u32>) -> u32 {
        match requested {
            Some(tokens) => tokens,
            None => self.output_reserve_tokens.unwrap_or(0),
        }
    }

    pub(super) fn available_input_tokens(self) -> Option<u32> {
        self.available_input_tokens_with_output_reserve(self.output_reserve_tokens.unwrap_or(0))
    }

    pub(super) fn recent_suffix_token_cap(self) -> u32 {
        self.available_input_tokens()
            .map(|available| {
                available.saturating_mul(RECENT_SUFFIX_NUMERATOR) / RECENT_SUFFIX_DENOMINATOR
            })
            .unwrap_or(RECENT_SUFFIX_MAX_TOKENS)
            .min(RECENT_SUFFIX_MAX_TOKENS)
    }

    pub(super) fn available_input_tokens_with_output_reserve(
        self,
        output_reserve_tokens: u32,
    ) -> Option<u32> {
        let window = self.context_window_tokens?;
        Some(
            window
                .saturating_sub(output_reserve_tokens)
                .saturating_sub(self.safety_tokens),
        )
    }

    pub(super) fn fits(self, input_tokens: u32) -> bool {
        self.available_input_tokens()
            .is_none_or(|available| input_tokens <= available)
    }

    pub(super) fn fits_with_output_reserve(
        self,
        input_tokens: u32,
        output_reserve_tokens: u32,
    ) -> bool {
        self.available_input_tokens_with_output_reserve(output_reserve_tokens)
            .is_none_or(|available| input_tokens <= available)
    }
}

pub(super) async fn admit_request(
    provider: &dyn ProviderOperations,
    request: RequestContext<'_>,
) -> ContextAdmission {
    let budget = ContextBudget::from_metadata(provider.context_metadata(request.model).await);
    let reserve = budget.effective_output_reserve_tokens(request.output_reserve_tokens);
    let Some(available) = budget.available_input_tokens_with_output_reserve(reserve) else {
        return ContextAdmission::Ready {
            estimated_input_tokens: 0,
            available_input_tokens: None,
        };
    };
    let rendered_input = request.input.render_for_token_count();
    let mut estimated = count_tokens_or_estimate(
        provider,
        request.instructions,
        &rendered_input,
        request.model,
    )
    .await;
    estimated = estimated.saturating_add(estimate_tools_tokens(request.tools));
    if request.hosted_web_search {
        estimated = estimated.saturating_add(HOSTED_SEARCH_OVERHEAD_TOKENS);
    }
    let hard_overflow = estimated > available;
    if hard_overflow && request.has_compactable_history {
        return ContextAdmission::HardOverflowWithCompactableHistory {
            estimated_input_tokens: estimated,
            available_input_tokens: available,
        };
    }
    if hard_overflow {
        return ContextAdmission::HardOverflowWithOnlyActiveContext {
            estimated_input_tokens: estimated,
            available_input_tokens: available,
        };
    }
    let threshold = soft_compaction_threshold(available);
    if estimated >= threshold && request.has_compactable_history {
        return ContextAdmission::CompactablePressure {
            estimated_input_tokens: estimated,
            available_input_tokens: available,
        };
    }
    ContextAdmission::Ready {
        estimated_input_tokens: estimated,
        available_input_tokens: Some(available),
    }
}

pub(super) const fn soft_compaction_threshold(available_input_tokens: u32) -> u32 {
    available_input_tokens.saturating_mul(COMPACTION_THRESHOLD_NUMERATOR)
        / COMPACTION_THRESHOLD_DENOMINATOR
}

pub(super) fn hard_overflow_error(admission: ContextAdmission) -> noema_providers::ProviderError {
    let ContextAdmission::HardOverflowWithOnlyActiveContext {
        estimated_input_tokens,
        available_input_tokens,
    } = admission
    else {
        unreachable!("hard_overflow_error requires active-context overflow")
    };
    noema_providers::ProviderError::InvalidRequest {
        message: format!(
            "request context requires approximately {estimated_input_tokens} input tokens but the selected model allows {available_input_tokens}; no completed history remains to compact"
        ),
    }
}

fn estimate_tools_tokens(tools: &[ProviderTool]) -> u32 {
    if tools.is_empty() {
        return 0;
    }
    let rendered = serde_json::to_string(
        &tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.exposed_name(),
                    "description": tool.canonical_spec().description,
                    "input_schema": tool.canonical_spec().input_schema,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_default();
    estimate_text_tokens(&rendered)
}

pub(super) fn estimate_text_tokens(value: &str) -> u32 {
    let chars = value.chars().count();
    if chars == 0 {
        0
    } else {
        chars.div_ceil(FALLBACK_CHARS_PER_TOKEN) as u32
    }
}

pub(super) async fn count_tokens_or_estimate(
    provider: &dyn ProviderOperations,
    instructions: Option<&str>,
    input: &str,
    model: Option<&str>,
) -> u32 {
    match provider.count_tokens(instructions, input, model).await {
        Ok(Some(tokens)) => tokens,
        Ok(None) | Err(_) => {
            instructions.map_or(0, estimate_text_tokens) + estimate_text_tokens(input)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{future::Future, pin::Pin};

    use noema_capabilities::ToolSpec;
    use noema_providers::{
        GenerateRequest, GenerateResponse, GenerateStreamEvent, GenerateToolResultInput,
        ProviderError,
    };
    use serde_json::json;

    use super::*;

    #[derive(Debug)]
    struct MetadataProvider;

    impl ProviderOperations for MetadataProvider {
        fn context_metadata(
            &self,
            _model: Option<&str>,
        ) -> noema_providers::ProviderContextFuture<'_> {
            Box::pin(async {
                ProviderContextMetadata {
                    context_window_tokens: Some(2_000),
                    default_output_reserve_tokens: Some(100),
                    compact_summary_target_tokens: Some(128),
                }
            })
        }

        fn generate_streaming<'a>(
            &'a self,
            _request: GenerateRequest,
            _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
        ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>>
        {
            Box::pin(async { unreachable!("admission does not generate") })
        }
    }

    #[tokio::test]
    async fn full_request_accounting_includes_every_provider_visible_surface() {
        let provider = MetadataProvider;
        let instructions = "follow the exact task contract";
        let input = GenerateInput::NativeToolResults(vec![GenerateToolResultInput {
            id: Some("result_1".to_string()),
            call_id: "call_1".to_string(),
            name: "mail.get".to_string(),
            provider_name: Some("mail_get".to_string()),
            arguments: json!({"id": "message_1"}),
            success: true,
            payload: json!({"body": "x".repeat(3_600)}),
        }]);
        let tools = vec![ProviderTool::canonical(
            ToolSpec::new(
                "mail.get",
                "Fetch a complete message body and metadata.",
                json!({"type": "object", "properties": {"id": {"type": "string"}}}),
            )
            .expect("tool"),
        )];
        let admission = admit_request(
            &provider,
            RequestContext {
                model: Some("test"),
                instructions: Some(instructions),
                input: &input,
                tools: &tools,
                hosted_web_search: true,
                output_reserve_tokens: Some(800),
                has_compactable_history: false,
            },
        )
        .await;
        let expected = estimate_text_tokens(instructions)
            + estimate_text_tokens(&input.render_for_token_count())
            + estimate_tools_tokens(&tools)
            + HOSTED_SEARCH_OVERHEAD_TOKENS;
        assert_eq!(admission.estimated_input_tokens(), expected);
        assert_eq!(
            admission,
            ContextAdmission::HardOverflowWithOnlyActiveContext {
                estimated_input_tokens: expected,
                available_input_tokens: 1_072,
            }
        );
    }
}
