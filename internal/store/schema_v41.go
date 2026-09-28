package store

// schemaV41SQL records which Task run consumed each authenticated reply.
const schemaV41SQL = `
ALTER TABLE task_messages ADD COLUMN consumed_by_run_id TEXT REFERENCES task_runs(run_id) ON DELETE RESTRICT;
ALTER TABLE task_messages ADD COLUMN consumed_at_ms INTEGER;
`
