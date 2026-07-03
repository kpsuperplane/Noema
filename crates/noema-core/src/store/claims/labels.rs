use surrealdb::types::Datetime;

use crate::memory::Sensitivity;

use super::model::ClaimStatus;
use crate::store::StoreError;

pub(super) fn parse_sensitivity(value: &str) -> Result<Sensitivity, StoreError> {
    Sensitivity::from_wire(value).ok_or_else(|| StoreError::InvalidEnum {
        kind: "sensitivity",
        value: value.to_string(),
    })
}

pub(super) fn allowed_match_sensitivities(sensitivity: Sensitivity) -> Vec<String> {
    match sensitivity {
        Sensitivity::Public => vec!["public".to_string()],
        Sensitivity::Normal => vec!["public".to_string(), "normal".to_string()],
        Sensitivity::Private | Sensitivity::Sensitive | Sensitivity::Secret => {
            vec![sensitivity.as_str().to_string()]
        }
    }
}

pub(super) fn exact_match_only_sensitivity(sensitivity: Sensitivity) -> bool {
    matches!(
        sensitivity,
        Sensitivity::Private | Sensitivity::Sensitive | Sensitivity::Secret
    )
}

pub(super) fn compatible_match_predicates(predicate_id: &str) -> Vec<String> {
    match predicate_id {
        "likes" => vec!["likes".to_string(), "dislikes".to_string()],
        "dislikes" => vec!["dislikes".to_string(), "likes".to_string()],
        other => vec![other.to_string()],
    }
}

pub(super) fn contains_case_folded(value: &str, query: &str) -> bool {
    value.to_ascii_lowercase().contains(query)
}

pub(super) fn default_memory_graph_statuses() -> Vec<ClaimStatus> {
    vec![
        ClaimStatus::Candidate,
        ClaimStatus::Active,
        ClaimStatus::Confirmed,
    ]
}

pub(in crate::store) fn format_datetime(value: Datetime) -> String {
    value.to_string()
}

pub(super) fn entity_record_id(entity_id: &str) -> String {
    crate::store::ids::hex_record_fragment("entity_", entity_id)
}
