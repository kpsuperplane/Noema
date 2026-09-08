package store

// schemaV38SQL retires ACP execution and preserves Task history.
const schemaV38SQL = `
UPDATE task_run_items SET status='interrupted',
 updated_at_ms=CAST(strftime('%s','now') AS INTEGER)*1000
WHERE status IN ('queued','running') AND run_id IN (
 SELECT run_id FROM task_runs WHERE executor_backend='acp'
 OR task_id IN (SELECT task_id FROM tasks WHERE executor_acp_connection_revision IS NOT NULL)
);
UPDATE task_runs SET status='failed',error_code='unsupported_executor',
 error_message='ACP support was removed. Check previous external results before creating a replacement Task.',
 ended_at_ms=CAST(strftime('%s','now') AS INTEGER)*1000,
 updated_at_ms=CAST(strftime('%s','now') AS INTEGER)*1000
WHERE status IN ('queued','leased','running','waiting_for_approval','interrupted')
 AND (executor_backend='acp' OR task_id IN (
 SELECT task_id FROM tasks WHERE executor_acp_connection_revision IS NOT NULL));
UPDATE task_gates SET gate_state='superseded',resolved_by='actor:system',
 resolved_at_ms=CAST(strftime('%s','now') AS INTEGER)*1000
WHERE gate_state='open' AND task_id IN (
 SELECT task_id FROM tasks WHERE executor_acp_connection_revision IS NOT NULL);
UPDATE tasks SET state='failed',stage_key='waiting',current_run_id=NULL,active_gate_id=NULL,
 revision=revision+1,generation=generation+1,
 updated_at_ms=CAST(strftime('%s','now') AS INTEGER)*1000
WHERE executor_acp_connection_revision IS NOT NULL AND state NOT IN ('completed','cancelled');
UPDATE task_recurrences SET lifecycle='paused',revision=revision+1,
 pending_coalesced_at_ms=NULL,updated_at_ms=CAST(strftime('%s','now') AS INTEGER)*1000
WHERE executor_acp_connection_revision IS NOT NULL AND lifecycle='active';
DROP TABLE acp_auth_attempts;
DROP TABLE acp_agents;
`
