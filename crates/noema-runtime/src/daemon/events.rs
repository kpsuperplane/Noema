//! Transport-neutral live runtime events.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::broadcast;

use super::TurnStreamEvent;

/// Live conversation event emitted by the runtime.
#[derive(Clone, Debug)]
pub enum ConversationRuntimeEvent {
    /// One runtime turn stream event.
    Turn {
        /// Client-generated id for optimistic UI correlation.
        client_message_id: Option<String>,
        /// Runtime event.
        event: Box<TurnStreamEvent>,
    },
    /// Turn completion event.
    Completed {
        /// Durable Noema conversation id.
        conversation_id: String,
        /// Client-generated id for optimistic UI correlation.
        client_message_id: Option<String>,
    },
}

/// Live task detail event used to invalidate and refill task projections.
#[derive(Clone, Debug)]
pub enum TaskRuntimeEvent {
    /// Durable task/run state or safe run activity changed.
    Changed {
        /// Durable task identifier.
        task_id: String,
    },
}

impl ConversationRuntimeEvent {
    /// Return the durable conversation id for this event.
    #[must_use]
    pub fn conversation_id(&self) -> &str {
        match self {
            Self::Turn { event, .. } => event.conversation_id(),
            Self::Completed {
                conversation_id, ..
            } => conversation_id,
        }
    }
}

/// In-process registry for transport-neutral runtime events.
#[derive(Clone, Debug, Default)]
pub struct RuntimeEventRegistry {
    conversations: Arc<Mutex<HashMap<String, broadcast::Sender<ConversationRuntimeEvent>>>>,
    tasks: Arc<Mutex<HashMap<String, broadcast::Sender<TaskRuntimeEvent>>>>,
}

impl RuntimeEventRegistry {
    /// Subscribe to one conversation's live runtime events.
    #[must_use]
    pub fn subscribe_conversation(
        &self,
        conversation_id: &str,
    ) -> broadcast::Receiver<ConversationRuntimeEvent> {
        self.conversation_sender(conversation_id).subscribe()
    }

    /// Publish one live conversation event.
    pub fn publish_conversation(&self, event: ConversationRuntimeEvent) {
        let sender = self.conversation_sender(event.conversation_id());
        let _ = sender.send(event);
    }

    /// Subscribe to one task's durable and live detail updates.
    #[must_use]
    pub fn subscribe_task(&self, task_id: &str) -> broadcast::Receiver<TaskRuntimeEvent> {
        self.task_sender(task_id).subscribe()
    }

    /// Publish one task detail update.
    pub fn publish_task(&self, event: TaskRuntimeEvent) {
        let task_id = match &event {
            TaskRuntimeEvent::Changed { task_id } => task_id,
        };
        let _ = self.task_sender(task_id).send(event);
    }

    fn conversation_sender(
        &self,
        conversation_id: &str,
    ) -> broadcast::Sender<ConversationRuntimeEvent> {
        let mut conversations = self
            .conversations
            .lock()
            .expect("runtime conversation event registry poisoned");
        conversations
            .entry(conversation_id.to_string())
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }

    fn task_sender(&self, task_id: &str) -> broadcast::Sender<TaskRuntimeEvent> {
        let mut tasks = self
            .tasks
            .lock()
            .expect("runtime task event registry poisoned");
        tasks
            .entry(task_id.to_string())
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::{AgentStatus, daemon::TurnStreamEvent};

    use super::*;

    #[tokio::test]
    async fn registry_delivers_events_to_subscriber() {
        let registry = RuntimeEventRegistry::default();
        let mut receiver = registry.subscribe_conversation("conversation_1");

        registry.publish_conversation(ConversationRuntimeEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::AgentStatusChanged {
                conversation_id: "conversation_1".to_string(),
                status: AgentStatus::Thinking,
            }),
        });

        let event = receiver.recv().await.expect("event should be delivered");
        assert_eq!(event.conversation_id(), "conversation_1");
    }

    #[tokio::test]
    async fn registry_delivers_task_events_to_subscriber() {
        let registry = RuntimeEventRegistry::default();
        let mut receiver = registry.subscribe_task("task_1");

        registry.publish_task(TaskRuntimeEvent::Changed {
            task_id: "task_1".to_string(),
        });

        assert!(
            matches!(receiver.recv().await, Ok(TaskRuntimeEvent::Changed { task_id }) if task_id == "task_1")
        );
    }
}
