use crate::graphql::agents::{GraphqlModelPreferenceSelectionMode, GraphqlReasoningEffort};

use super::super::GraphqlTaskComplexity;

graphql_object_from! { "One human-controlled executor model-pool entry." => pub struct GraphqlTaskModelPoolEntry("TaskModelPoolEntry")
    from noema_tasks::TaskModelPoolEntry as value {
    "Stable pool entry id." => pool_entry_id: String = value.pool_entry_id,
    "Complexity tier exposed to the primary agent." => complexity: GraphqlTaskComplexity = value.complexity.into(),
    "Optional human-facing label." => label: Option<String> = value.label,
    "Provider family for this entry." => provider_kind: String = value.model.provider_kind,
    "Provider account owning the model profile." => provider_account_id: String = value.model.provider_account_id,
    "Exact provider model/profile for an explicit preference." => model_profile: Option<String> = value.preference.model_profile().map(str::to_string),
    "Optional explicit reasoning effort." => reasoning_effort: Option<GraphqlReasoningEffort> = value.preference.reasoning_effort().map(Into::into),
    "Whether this preference requests faster service." => fast_mode: bool = value.model.fast_mode,
    "Whether Noema or the human chooses the concrete model." => selection_mode: GraphqlModelPreferenceSelectionMode = (&value.preference).into(),
    "Whether this entry can be selected for new tasks." => enabled: bool = value.enabled,
    "Human-controlled ordering within its tier." => sort_order: i32 = i32::try_from(value.sort_order).unwrap_or(i32::MAX),
    "Creation timestamp." => created_at: String = value.created_at,
    "Last update timestamp." => updated_at: String = value.updated_at,
} }
