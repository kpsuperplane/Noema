package store

// schemaV30SQL stores the primary Chat's durable work-notification cursor.
const schemaV30SQL = `
ALTER TABLE local_human_state ADD COLUMN primary_task_notification_event_id INTEGER NOT NULL DEFAULT 0;
UPDATE local_human_state SET primary_task_notification_event_id =
    COALESCE((SELECT MAX(event_id) FROM work_events), 0)
WHERE state_id = 1;
`
