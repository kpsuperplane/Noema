package store

// schemaV17SQL repairs legacy Task stages and retains approval decisions.
const schemaV17SQL = `
ALTER TABLE task_messages ADD COLUMN approval_decision TEXT
    CHECK (approval_decision IS NULL OR approval_decision IN ('approved','declined'));
CREATE INDEX command_receipts_task_document
    ON command_receipts(result_task_id, request_digest) WHERE result_task_id IS NOT NULL;

UPDATE tasks SET
    stage_key = CASE state
        WHEN 'running' THEN 'doing'
        WHEN 'completed' THEN 'done'
        WHEN 'failed' THEN 'waiting'
        WHEN 'cancelled' THEN 'cancelled'
        ELSE 'inbox'
    END,
    completed_at_ms = CASE WHEN state = 'completed' THEN updated_at_ms ELSE completed_at_ms END,
    cancelled_at_ms = CASE WHEN state = 'cancelled' THEN updated_at_ms ELSE cancelled_at_ms END
WHERE stage_key = 'inbox' AND state <> 'captured';
`
