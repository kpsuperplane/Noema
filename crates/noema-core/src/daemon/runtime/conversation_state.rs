use crate::daemon::protocol::DaemonError;

use super::actor::{ActiveConversation, CodexRuntimeActor};

impl CodexRuntimeActor {
    pub(super) async fn hydrate_active_conversation(
        &mut self,
        conversation_id: &str,
        cwd_override: Option<String>,
    ) -> Result<ActiveConversation, DaemonError> {
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
            cwd,
            next_turn_index,
        };
        self.conversations
            .insert(conversation_id.to_string(), conversation.clone());
        Ok(conversation)
    }
}

#[cfg(test)]
mod tests {
    use super::ActiveConversation;

    #[test]
    fn active_conversation_state_is_provider_route_agnostic() {
        let conversation = ActiveConversation {
            cwd: Some("/tmp/project".to_string()),
            next_turn_index: 7,
        };

        let debug = format!("{conversation:?}");
        assert!(debug.contains("next_turn_index: 7"));
        assert!(!debug.contains("provider"));
    }
}
