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
