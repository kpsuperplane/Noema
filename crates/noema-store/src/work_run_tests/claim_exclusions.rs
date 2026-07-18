use noema_tasks::{TaskId, WorkDomainError};

use super::*;

#[tokio::test]
async fn fifo_claim_skips_excluded_task_without_dequeuing_it() {
    let (store, service) = fixture().await;
    let (first_task, first_run_id) = queued_task(
        &service,
        "fifo-exclusion:capture:a",
        "fifo-exclusion:queue:a",
        "excluded oldest",
    )
    .await;
    let (second_task, second_run_id) = queued_task(
        &service,
        "fifo-exclusion:capture:b",
        "fifo-exclusion:queue:b",
        "claimable next",
    )
    .await;
    store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE agent_runs SET queued_at = CASE run_id WHEN ?1 THEN '2026-01-01T00:00:00.000Z' ELSE '2026-01-01T00:00:01.000Z' END WHERE run_id IN (?1, ?2)",
                rusqlite::params![first_run_id, second_run_id],
            )?;
            Ok(())
        })
        .await
        .expect("order queued runs");

    let claimed = service
        .claim_next_work_run(
            "worker:fifo-exclusion",
            30,
            std::slice::from_ref(&first_task.task_id),
        )
        .await
        .expect("claim next unexcluded run")
        .expect("unexcluded run exists");
    assert_eq!(claimed.run.task_id, second_task.task_id);
    assert_eq!(claimed.run.run_id, second_run_id);

    let first_status: String = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT status FROM agent_runs WHERE run_id = ?1",
                    [first_run_id],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("read excluded run status");
    assert_eq!(first_status, "queued");
}

#[tokio::test]
async fn claim_exclusions_are_bounded_and_unique() {
    let (_store, service) = fixture().await;
    let duplicate = TaskId::new("task:duplicate-exclusion").expect("task id");
    let duplicate_error = service
        .claim_next_work_run(
            "worker:duplicate-exclusions",
            30,
            &[duplicate.clone(), duplicate],
        )
        .await
        .expect_err("duplicate exclusions must fail");
    assert!(matches!(
        duplicate_error,
        StoreError::Work(WorkDomainError::InvalidInput {
            field: "run.excluded_task_ids",
            ..
        })
    ));

    let oversized = (0..9)
        .map(|index| TaskId::new(format!("task:excluded:{index}")).expect("task id"))
        .collect::<Vec<_>>();
    let oversized_error = service
        .claim_next_work_run("worker:oversized-exclusions", 30, &oversized)
        .await
        .expect_err("oversized exclusions must fail");
    assert!(matches!(
        oversized_error,
        StoreError::Work(WorkDomainError::InvalidInput {
            field: "run.excluded_task_ids",
            ..
        })
    ));
}
