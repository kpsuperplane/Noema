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
    /// Human intervention state changed without adding a transcript item.
    HumanInterventionsChanged {
        /// Durable Noema conversation id whose intervention projection changed.
        conversation_id: String,
    },
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
        /// Run whose transcript changed, when the event came from a run item write.
        run_id: Option<String>,
    },
}

/// Live invalidation for one committed Work event.
///
/// The event contains no derived task state. Consumers use it only as a wake
/// signal and refill their projection from the durable Work ledger/store.
#[derive(Clone, Debug)]
pub enum WorkRuntimeEvent {
    /// A Work event committed for a workspace.
    Committed {
        /// Workspace whose durable event changed.
        workspace_id: String,
        /// Affected task, when the event is task-scoped.
        task_id: Option<String>,
    },
}

/// Live invalidation for native-memory projections.
#[derive(Clone, Debug)]
pub enum MemoryRuntimeEvent {
    /// Pending source items, update status, or canonical pages changed.
    Changed,
}

impl ConversationRuntimeEvent {
    /// Return the durable conversation id for this event.
    #[must_use]
    pub fn conversation_id(&self) -> &str {
        match self {
            Self::HumanInterventionsChanged { conversation_id } => conversation_id,
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
    all_conversations: Arc<Mutex<Option<broadcast::Sender<ConversationRuntimeEvent>>>>,
    tasks: Arc<Mutex<HashMap<String, broadcast::Sender<TaskRuntimeEvent>>>>,
    workspaces: Arc<Mutex<HashMap<String, broadcast::Sender<WorkRuntimeEvent>>>>,
    memory: Arc<Mutex<Option<broadcast::Sender<MemoryRuntimeEvent>>>>,
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
        let _ = sender.send(event.clone());
        let _ = self.all_conversation_sender().send(event);
    }

    /// Subscribe to every live conversation event for delivery-neutral observers.
    #[must_use]
    pub fn subscribe_all_conversations(&self) -> broadcast::Receiver<ConversationRuntimeEvent> {
        self.all_conversation_sender().subscribe()
    }

    /// Subscribe to one task's durable and live detail updates.
    #[must_use]
    pub fn subscribe_task(&self, task_id: &str) -> broadcast::Receiver<TaskRuntimeEvent> {
        self.task_sender(task_id).subscribe()
    }

    /// Publish one task detail update.
    pub fn publish_task(&self, event: TaskRuntimeEvent) {
        let task_id = match &event {
            TaskRuntimeEvent::Changed { task_id, .. } => task_id,
        };
        let _ = self.task_sender(task_id).send(event);
    }

    /// Subscribe to committed Work invalidations for one workspace.
    #[must_use]
    pub fn subscribe_work(&self, workspace_id: &str) -> broadcast::Receiver<WorkRuntimeEvent> {
        self.work_sender(workspace_id).subscribe()
    }

    /// Publish one Work invalidation after its durable transaction commits.
    pub fn publish_work(&self, event: WorkRuntimeEvent) {
        let workspace_id = match &event {
            WorkRuntimeEvent::Committed { workspace_id, .. } => workspace_id,
        };
        let _ = self.work_sender(workspace_id).send(event);
    }

    /// Subscribe to native-memory projection invalidations.
    #[must_use]
    pub fn subscribe_memory(&self) -> broadcast::Receiver<MemoryRuntimeEvent> {
        self.memory_sender().subscribe()
    }

    /// Publish one native-memory projection invalidation.
    pub fn publish_memory(&self, event: MemoryRuntimeEvent) {
        let _ = self.memory_sender().send(event);
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

    fn all_conversation_sender(&self) -> broadcast::Sender<ConversationRuntimeEvent> {
        let mut sender = self
            .all_conversations
            .lock()
            .expect("runtime conversation observer registry poisoned");
        sender
            .get_or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }

    fn work_sender(&self, workspace_id: &str) -> broadcast::Sender<WorkRuntimeEvent> {
        let mut workspaces = self
            .workspaces
            .lock()
            .expect("runtime Work event registry poisoned");
        workspaces
            .entry(workspace_id.to_string())
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }

    fn memory_sender(&self) -> broadcast::Sender<MemoryRuntimeEvent> {
        let mut memory = self
            .memory
            .lock()
            .expect("runtime memory event registry poisoned");
        memory
            .get_or_insert_with(|| broadcast::channel(32).0)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn registry_delivers_scoped_conversation_and_task_events() {
        let registry = RuntimeEventRegistry::default();
        let mut receiver = registry.subscribe_conversation("conversation_1");

        registry.publish_conversation(ConversationRuntimeEvent::HumanInterventionsChanged {
            conversation_id: "conversation_1".to_string(),
        });

        let event = receiver.recv().await.expect("event should be delivered");
        assert_eq!(event.conversation_id(), "conversation_1");
        let mut receiver = registry.subscribe_task("task_1");

        registry.publish_task(TaskRuntimeEvent::Changed {
            task_id: "task_1".to_string(),
            run_id: None,
        });

        assert!(
            matches!(receiver.recv().await, Ok(TaskRuntimeEvent::Changed { task_id, run_id }) if task_id == "task_1" && run_id.is_none())
        );

        let mut receiver = registry.subscribe_work("workspace:personal");
        registry.publish_work(WorkRuntimeEvent::Committed {
            workspace_id: "workspace:personal".to_string(),
            task_id: Some("task_1".to_string()),
        });
        assert!(matches!(
            receiver.recv().await,
            Ok(WorkRuntimeEvent::Committed { workspace_id, task_id })
                if workspace_id == "workspace:personal" && task_id.as_deref() == Some("task_1")
        ));

        let mut receiver = registry.subscribe_memory();
        registry.publish_memory(MemoryRuntimeEvent::Changed);
        assert!(matches!(
            receiver.recv().await,
            Ok(MemoryRuntimeEvent::Changed)
        ));
    }
}
