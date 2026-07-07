use crate::daemon::protocol::DaemonError;

use super::actor::{ActiveConversation, CodexRuntimeActor};

impl CodexRuntimeActor {
    pub(super) async fn hydrate_active_conversation(
        &mut self,
        conversation_id: &str,
        cwd_override: Option<String>,
    ) -> Result<ActiveConversation, DaemonError> {
        let selection =
            provider_selection_for_conversation(&self.store, &self.default_provider_kind).await?;
        let next_turn_index = self
            .store
            .next_conversation_turn_index(conversation_id)
            .await?;
        let cwd = cwd_override.or_else(|| {
            self.conversations
                .get(conversation_id)
                .and_then(|conversation| conversation.cwd.clone())
        });
        let conversation = ActiveConversation {
            provider_kind: selection.provider_kind,
            model: selection.model,
            reasoning_effort: selection.reasoning_effort,
            cwd,
            next_turn_index,
        };
        self.conversations
            .insert(conversation_id.to_string(), conversation.clone());
        Ok(conversation)
    }
}

pub(super) async fn provider_selection_for_conversation(
    store: &crate::NoemaStore,
    default_provider_kind: &str,
) -> Result<ConversationProviderSelection, DaemonError> {
    let Some(preference) = store.get_agent_runtime_preference("agent:primary").await? else {
        return Ok(ConversationProviderSelection {
            provider_kind: default_provider_kind.to_string(),
            model: None,
            reasoning_effort: None,
        });
    };
    Ok(ConversationProviderSelection {
        provider_kind: preference.provider_kind,
        model: Some(preference.model_profile),
        reasoning_effort: preference.reasoning_effort,
    })
}

#[derive(Debug, Clone)]
pub(super) struct ConversationProviderSelection {
    pub(super) provider_kind: String,
    pub(super) model: Option<String>,
    pub(super) reasoning_effort: Option<crate::provider::ReasoningEffort>,
}
