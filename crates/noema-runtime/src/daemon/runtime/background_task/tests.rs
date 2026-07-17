use super::*;

#[test]
fn expired_run_deadline_gets_a_bounded_terminal_grace_window() {
    let now = tokio::time::Instant::now();
    let expired = now - Duration::from_secs(1);
    assert_eq!(
        task_finalization_deadline(expired, now),
        now + Duration::from_secs(30)
    );
}

#[test]
fn compaction_provider_error_is_preserved_for_task_failure_finalization() {
    let error = ProviderError::ProviderUnavailable {
        provider: "test-provider".to_string(),
        message: "continuation compaction failed".to_string(),
    };

    let propagated = propagate_compaction_result(Err(error)).expect_err("propagate error");

    assert!(matches!(propagated, RuntimeError::Provider(_)));
    assert!(
        propagated
            .to_string()
            .contains("continuation compaction failed")
    );
}
