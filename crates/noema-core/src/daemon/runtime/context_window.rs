use crate::provider::ProviderContextMetadata;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_context_window_fits_without_budgeting() {
        let budget = ContextBudget::from_metadata(ProviderContextMetadata::default());

        assert!(budget.fits(10_000));
        assert_eq!(budget.available_input_tokens(), None);
    }

    #[test]
    fn context_window_reserves_output_and_safety_tokens() {
        let budget = ContextBudget::from_metadata(ProviderContextMetadata {
            context_window_tokens: Some(4_096),
            default_output_reserve_tokens: Some(512),
            compact_summary_target_tokens: Some(512),
        });

        assert_eq!(budget.available_input_tokens(), Some(3_456));
        assert!(budget.fits(3_456));
        assert!(!budget.fits(3_457));
    }

    #[test]
    fn estimated_tokens_are_conservative_for_text() {
        assert_eq!(estimate_text_tokens("abc"), 1);
        assert_eq!(estimate_text_tokens("abcd"), 2);
        assert_eq!(estimate_text_tokens(""), 0);
    }
}
