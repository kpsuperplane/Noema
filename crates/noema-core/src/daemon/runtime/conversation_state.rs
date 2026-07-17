use crate::daemon::protocol::DaemonError;
use noema_providers::ProviderSelectionSnapshot;

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
            provider_selection: selection,
            cwd,
            next_turn_index,
        };
        self.conversations
            .insert(conversation_id.to_string(), conversation.clone());
        Ok(conversation)
    }
}

pub(super) async fn provider_selection_for_conversation(
    store: &noema_store::NoemaStore,
    default_provider_kind: &str,
) -> Result<ProviderSelectionSnapshot, DaemonError> {
    let Some(preference) = store.get_agent_runtime_preference("agent:primary").await? else {
        return Ok(ProviderSelectionSnapshot::provider_default(
            default_provider_kind,
            format!("provider_account:{default_provider_kind}:default"),
            None,
            Some("runtime_default".to_string()),
        ));
    };
    Ok(ProviderSelectionSnapshot::explicit(
        preference.provider_kind,
        preference.provider_account_id,
        preference.model_profile,
        preference.reasoning_effort,
        Some("agent_runtime_preference".to_string()),
    ))
}
