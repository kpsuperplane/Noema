use crate::daemon::protocol::RuntimeError;

use super::actor::{ActiveConversation, RuntimeActor};

impl RuntimeActor {
    pub(super) async fn hydrate_active_conversation(
        &mut self,
        conversation_id: &str,
        cwd_override: Option<String>,
    ) -> Result<ActiveConversation, RuntimeError> {
        let next_turn_index = self
            .store
            .next_conversation_turn_index(conversation_id)
            .await?;
        let cwd = self
            .store
            .conversation_working_directory(conversation_id, cwd_override.as_deref())
            .await?
            .to_string_lossy()
            .into_owned();
        let conversation = ActiveConversation {
            cwd: Some(cwd),
            next_turn_index,
        };
        self.conversations
            .insert(conversation_id.to_string(), conversation.clone());
        Ok(conversation)
    }
}
