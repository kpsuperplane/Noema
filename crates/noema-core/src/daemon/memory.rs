pub(super) mod context;
pub(super) mod tool;

pub(in crate::daemon) const HUMAN_MEMORY_SCOPE_ID: &str = "human:local";

pub(in crate::daemon) fn conversation_scope_id(conversation_id: &str) -> String {
    if conversation_id.starts_with("conversation:") {
        conversation_id.to_string()
    } else {
        format!("conversation:{conversation_id}")
    }
}
