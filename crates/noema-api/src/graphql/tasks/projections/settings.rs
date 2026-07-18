use async_graphql::SimpleObject;

use crate::graphql::agents::GraphqlReasoningEffort;

use super::super::GraphqlTaskComplexity;

/// One human-controlled executor model-pool entry.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskModelPoolEntry")]
pub struct GraphqlTaskModelPoolEntry {
    /// Stable pool entry id.
    pub pool_entry_id: String,
    /// Complexity tier exposed to the primary agent.
    pub complexity: GraphqlTaskComplexity,
    /// Optional human-facing label.
    pub label: Option<String>,
    /// Provider family for this entry.
    pub provider_kind: String,
    /// Provider account owning the model profile.
    pub provider_account_id: String,
    /// Exact provider model/profile.
    pub model_profile: String,
    /// Optional reasoning effort.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
    /// Whether this entry can be selected for new tasks.
    pub enabled: bool,
    /// Human-controlled ordering within its tier.
    pub sort_order: i32,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl From<noema_tasks::TaskModelPoolEntry> for GraphqlTaskModelPoolEntry {
    fn from(value: noema_tasks::TaskModelPoolEntry) -> Self {
        Self {
            pool_entry_id: value.pool_entry_id,
            complexity: value.complexity.into(),
            label: value.label,
            provider_kind: value.model.provider_kind,
            provider_account_id: value.model.provider_account_id,
            model_profile: value.model.model_profile.unwrap_or_default(),
            reasoning_effort: value.model.reasoning_effort.map(Into::into),
            enabled: value.enabled,
            sort_order: i32::try_from(value.sort_order).unwrap_or(i32::MAX),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
