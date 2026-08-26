use crate::WorkDomainError;

const MAX_ID_BYTES: usize = 255;

fn validate_id(
    value: String,
    kind: &'static str,
    prefix: &'static str,
) -> Result<String, WorkDomainError> {
    if value.trim().is_empty() {
        return Err(WorkDomainError::InvalidInput {
            field: kind,
            message: "identifier cannot be blank".to_string(),
        });
    }
    if value.len() > MAX_ID_BYTES {
        return Err(WorkDomainError::InvalidInput {
            field: kind,
            message: "identifier exceeds 255 UTF-8 bytes".to_string(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(WorkDomainError::InvalidInput {
            field: kind,
            message: "identifier cannot contain control characters".to_string(),
        });
    }
    if !value.starts_with(prefix) {
        return Err(WorkDomainError::InvalidInput {
            field: kind,
            message: format!("identifier must start with {prefix}"),
        });
    }
    if value[prefix.len()..].trim().is_empty() {
        return Err(WorkDomainError::InvalidInput {
            field: kind,
            message: "identifier must contain a value after its prefix".to_string(),
        });
    }
    Ok(value)
}

noema_workspaces::semantic_id!(WorkDomainError, validate_id;
    WorkflowId, "workflow", "workflow:";
    WorkflowStageId, "workflow_stage", "stage:";
    TaskId, "task", "task:";
    TaskGateId, "task_gate", "gate:";
    TaskMessageId, "task_message", "task_message:";
    TaskRecurrenceId, "task_recurrence", "recurrence:";
    WorkEventId, "work_event", "event:";
);

#[cfg(test)]
mod tests {
    use super::{TaskId, WorkflowId};

    #[test]
    fn task_ids_reject_wrong_prefix_controls_and_oversize_values() {
        assert!(TaskId::new("workflow:one").is_err());
        assert!(TaskId::new(" ").is_err());
        assert!(TaskId::new("task:bad\nvalue").is_err());
        assert!(TaskId::new(format!("task:{}", "x".repeat(251))).is_err());
        assert_eq!(TaskId::new("task:one").unwrap().as_str(), "task:one");
    }

    #[test]
    fn ids_serialize_as_opaque_wire_strings_and_fail_closed() {
        let id = WorkflowId::new("workflow:personal:default").unwrap();
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            "\"workflow:personal:default\""
        );
        assert!(serde_json::from_str::<WorkflowId>("\"task:one\"").is_err());
    }
}
