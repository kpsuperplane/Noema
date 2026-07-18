use noema_providers::{ProviderContextMetadata, ProviderOperations};

const DEFAULT_CONTEXT_SAFETY_TOKENS: u32 = 128;
const FALLBACK_CHARS_PER_TOKEN: usize = 3;

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

    pub(super) fn available_input_tokens(self) -> Option<u32> {
        self.available_input_tokens_with_output_reserve(self.output_reserve_tokens.unwrap_or(0))
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
