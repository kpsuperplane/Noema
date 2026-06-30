use serde::Deserialize;
use surrealdb::types::{Datetime, SurrealValue};

use crate::memory::Sensitivity;

use super::model::ClaimStatus;

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct EntityIdRow {
    #[allow(dead_code)]
    pub(super) entity_id: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct SourceItemRow {
    #[allow(dead_code)]
    pub(super) item_id: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct PredicateExistsRow {
    #[allow(dead_code)]
    pub(super) predicate_id: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct ClaimIdRow {
    pub(super) claim_id: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct ClaimRow {
    pub(super) claim_id: String,
    pub(super) subject_entity_id: String,
    pub(super) object_entity_id: Option<String>,
    pub(super) predicate_id: String,
    pub(super) fact: String,
    pub(super) status: String,
    pub(super) sensitivity: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct CountRow {
    pub(super) count: i64,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct InspectionClaimRow {
    pub(super) claim_id: String,
    pub(super) subject_entity_id: String,
    pub(super) object_entity_id: Option<String>,
    pub(super) predicate_id: String,
    pub(super) fact: String,
    pub(super) status: String,
    pub(super) sensitivity: String,
    pub(super) confidence: Option<f64>,
    pub(super) created_at: Datetime,
    pub(super) updated_at: Datetime,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct InspectionPredicateRow {
    pub(super) predicate_id: String,
    pub(super) label: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct InspectionEntityRow {
    pub(super) entity_id: String,
    pub(super) entity_type: String,
    pub(super) canonical_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InspectionEntity {
    pub(super) entity_type: String,
    pub(super) canonical_name: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct InspectionEvidenceCountRow {
    pub(super) claim_id: String,
    pub(super) count: i64,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct InspectionEvidenceRow {
    pub(super) relation_id: Option<String>,
    pub(super) source_item_id: Option<String>,
    pub(super) authority: String,
    pub(super) excerpt: Option<String>,
    pub(super) observed_at: Option<Datetime>,
    pub(super) created_at: Datetime,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct ConsolidationMatchRow {
    pub(super) claim_id: String,
    pub(super) subject_entity_id: String,
    pub(super) object_entity_id: Option<String>,
    pub(super) predicate_id: String,
    pub(super) fact: String,
    pub(super) status: String,
    pub(super) sensitivity: String,
    pub(super) confidence: Option<f64>,
    #[allow(dead_code)]
    pub(super) updated_at: Datetime,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct RelatedClaimRow {
    pub(super) relation_id: String,
    pub(super) claim_id: String,
    pub(super) related_claim_id: String,
    pub(super) relation_kind: String,
    pub(super) rationale: String,
    pub(super) created_at: Datetime,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct ClaimStatusRow {
    #[allow(dead_code)]
    pub(super) claim_id: String,
    pub(super) status: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct RelationIdRow {
    #[allow(dead_code)]
    pub(super) relation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ClaimIdentityRow {
    pub(super) subject_entity_id: String,
    pub(super) object_entity_id: Option<String>,
    pub(super) predicate_id: String,
    pub(super) status: ClaimStatus,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct ClaimIdentityRecord {
    #[allow(dead_code)]
    pub(super) claim_id: String,
    pub(super) subject_entity_id: String,
    pub(super) object_entity_id: Option<String>,
    pub(super) predicate_id: String,
    pub(super) status: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ExistingClaimMergeRow {
    pub(super) status: ClaimStatus,
    pub(super) sensitivity: Sensitivity,
    pub(super) confidence: Option<f64>,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct ExistingClaimMergeRecord {
    #[allow(dead_code)]
    pub(super) claim_id: String,
    pub(super) status: String,
    pub(super) sensitivity: String,
    pub(super) confidence: Option<f64>,
}
