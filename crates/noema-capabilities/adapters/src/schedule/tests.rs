use super::*;
use crate::CursorBinding;
use noema_home::NoemaPaths;
use tempfile::tempdir;

fn store() -> (tempfile::TempDir, ScheduleStore) {
    let directory = tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(directory.path()).expect("paths");
    (directory, ScheduleStore::new(paths))
}

fn schedule() -> PollSchedule {
    PollSchedule {
        schedule_id: "schedule-1".to_string(),
        connection_id: "connection-1".to_string(),
        semantic_digest: "a".repeat(64),
        operation_id: "list_items".to_string(),
        account_kind: "personal_user".to_string(),
        interval_seconds: 60,
        enabled: true,
        revision: 1,
        next_attempt_epoch_seconds: 0,
        retry_attempt: 0,
        lease: None,
    }
}

#[test]
fn install_scan_claim_and_commit_are_filesystem_authoritative() {
    let (_home, store) = store();
    let installed = store
        .install(&schedule(), &PollCheckpoint::default())
        .expect("install");
    assert_eq!(installed.projection.status, "ready");
    let claim = store
        .claim("schedule-1", "worker-a", 10, 30)
        .expect("claim");
    assert!(matches!(
        store.claim("schedule-1", "worker-b", 11, 30),
        Err(ScheduleError::LeaseFenced)
    ));
    let checkpoint = PollCheckpoint {
        cursor: Some(CursorHandle {
            secret_reference: "cursor-ref".to_string(),
            binding: CursorBinding {
                connection_id: "connection-1".to_string(),
                semantic_digest: "a".repeat(64),
                operation_id: "list_items".to_string(),
                account_kind: "personal_user".to_string(),
                grant_revision: 1,
            },
            expires_at_epoch_seconds: 100,
        }),
        last_event_key: Some("event-1".to_string()),
        full_resync_required: false,
        revision: 1,
    };
    let committed = store.commit(&claim, &checkpoint, 70, 0).expect("commit");
    assert_eq!(
        committed.projection.last_event_key.as_deref(),
        Some("event-1")
    );
    assert_eq!(store.scan().expect("scan")[0].checkpoint, checkpoint);
}

#[test]
fn stale_claim_cannot_commit_after_revoke_and_restart() {
    let (_home, store) = store();
    store
        .install(&schedule(), &PollCheckpoint::default())
        .expect("install");
    let claim = store.claim("schedule-1", "worker-a", 1, 2).expect("claim");
    let revoked = store.revoke("schedule-1").expect("revoke");
    assert!(!revoked.schedule.enabled);
    assert!(matches!(
        store.commit(&claim, &PollCheckpoint::default(), 10, 0),
        Err(ScheduleError::LeaseFenced)
    ));
    let restarted = store.scan().expect("restart scan");
    assert_eq!(restarted[0].projection.status, "revoked");
}

#[test]
fn retry_policy_and_full_resync_projection_are_bounded() {
    let policy = PollRetryPolicy {
        initial_delay_seconds: 2,
        max_delay_seconds: 10,
        max_attempts: 4,
    };
    policy.validate().expect("valid policy");
    assert_eq!(policy.delay_seconds(0), 2);
    assert_eq!(policy.delay_seconds(3), 10);
    assert!(matches!(
        PollRetryPolicy {
            initial_delay_seconds: 0,
            max_delay_seconds: 10,
            max_attempts: 4,
        }
        .validate(),
        Err(ScheduleError::Invalid("retry_policy"))
    ));

    let (_home, store) = store();
    store
        .install(&schedule(), &PollCheckpoint::default())
        .expect("install");
    let claim = store.claim("schedule-1", "worker-a", 0, 30).expect("claim");
    let committed = store
        .commit(
            &claim,
            &PollCheckpoint {
                full_resync_required: true,
                revision: 1,
                ..PollCheckpoint::default()
            },
            0,
            1,
        )
        .expect("commit");
    assert_eq!(committed.projection.status, "full_resync_required");
    assert!(committed.projection.full_resync_required);
}

#[test]
fn scan_recovers_an_abandoned_atomic_replacement() {
    let (home, store) = store();
    store
        .install(&schedule(), &PollCheckpoint::default())
        .expect("install");
    let orphan = home
        .path()
        .join("adapters/schedules/schedule-1/.replace-aaaaaaaaaaaaaaaaaaaaaaaa");
    write_new_file(&orphan, b"unpublished").expect("orphan replacement");

    store.recover().expect("startup recovery");
    assert_eq!(store.scan().expect("recovered scan").len(), 1);
    assert!(!orphan.exists());
}
