use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    WorkDomainError,
    error::invalid_input,
    validation::{optional as normalize_optional, required as non_empty},
};

/// One immutable criterion in an execution contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskValidationCriterion {
    pub criterion_id: String,
    pub ordinal: u32,
    pub description: String,
    pub expected_evidence: Option<String>,
}

impl TaskValidationCriterion {
    /// Validate and normalize one criterion.
    /// # Errors
    /// Returns [`WorkDomainError`] when the identity or description is blank,
    /// or the ordinal is zero.
    pub fn normalized(&self) -> Result<Self, WorkDomainError> {
        let criterion_id = non_empty(&self.criterion_id, "criterion_id")?;
        let description = non_empty(&self.description, "description")?;
        if self.ordinal == 0 {
            return Err(invalid_input(
                "criterion.ordinal",
                "ordinal must be positive",
            ));
        }
        Ok(Self {
            criterion_id,
            ordinal: self.ordinal,
            description,
            expected_evidence: normalize_optional(self.expected_evidence.as_deref()),
        })
    }
}

/// Criterion supplied before the store allocates a stable id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct NewTaskValidationCriterion {
    pub criterion_id: Option<String>,
    pub ordinal: u32,
    pub description: String,
    pub expected_evidence: Option<String>,
}

impl NewTaskValidationCriterion {
    /// Normalize caller input while leaving id allocation to persistence.
    /// # Errors
    /// Returns [`WorkDomainError`] when the identity or description is blank,
    /// or the ordinal is zero.
    pub fn normalized(&self) -> Result<Self, WorkDomainError> {
        if self.ordinal == 0 {
            return Err(invalid_input(
                "criterion.ordinal",
                "ordinal must be positive",
            ));
        }
        Ok(Self {
            criterion_id: normalize_optional(self.criterion_id.as_deref()),
            ordinal: self.ordinal,
            description: non_empty(&self.description, "description")?,
            expected_evidence: normalize_optional(self.expected_evidence.as_deref()),
        })
    }
}

/// Normalize a complete criterion set supplied by a Planner or human
/// amendment.  Unassigned (`None`) criterion IDs are intentionally allowed so
/// the store can allocate them, but every supplied ID, description, and ordinal
/// must be globally unique.
pub(crate) fn normalize_new_criteria(
    criteria: &[NewTaskValidationCriterion],
) -> Result<Vec<NewTaskValidationCriterion>, WorkDomainError> {
    if criteria.is_empty() {
        return Err(invalid_input(
            "criteria",
            "at least one criterion is required",
        ));
    }
    let mut normalized = criteria
        .iter()
        .map(NewTaskValidationCriterion::normalized)
        .collect::<Result<Vec<_>, _>>()?;
    normalized.sort_by_key(|criterion| criterion.ordinal);
    let mut ordinals = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut descriptions = BTreeSet::new();
    for criterion in &normalized {
        if !ordinals.insert(criterion.ordinal) {
            return Err(invalid_input(
                "criteria.ordinal",
                "criterion ordinals must be unique",
            ));
        }
        if let Some(criterion_id) = &criterion.criterion_id
            && !ids.insert(criterion_id)
        {
            return Err(invalid_input(
                "criteria.criterion_id",
                "criterion IDs must be unique",
            ));
        }
        if !descriptions.insert(&criterion.description) {
            return Err(invalid_input(
                "criteria.description",
                "criterion descriptions must be unique",
            ));
        }
    }
    Ok(normalized)
}

pub(crate) fn normalize_criteria(
    criteria: &[TaskValidationCriterion],
) -> Result<Vec<TaskValidationCriterion>, WorkDomainError> {
    if criteria.is_empty() {
        return Err(invalid_input(
            "criteria",
            "at least one criterion is required",
        ));
    }
    let mut normalized = criteria
        .iter()
        .map(TaskValidationCriterion::normalized)
        .collect::<Result<Vec<_>, _>>()?;
    normalized.sort_by_key(|criterion| criterion.ordinal);
    let mut ordinals = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut descriptions = BTreeSet::new();
    for criterion in &normalized {
        if !ordinals.insert(criterion.ordinal) {
            return Err(invalid_input(
                "criteria",
                "criterion ordinals must be unique",
            ));
        }
        if !ids.insert(&criterion.criterion_id) {
            return Err(invalid_input("criteria", "criterion ids must be unique"));
        }
        if !descriptions.insert(&criterion.description) {
            return Err(invalid_input(
                "criteria",
                "criterion descriptions must be unique",
            ));
        }
    }
    Ok(normalized)
}
