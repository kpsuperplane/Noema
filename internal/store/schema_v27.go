package store

// schemaV27SQL records the Chat source and timezone for model-created Tasks.
const schemaV27SQL = `
ALTER TABLE tasks ADD COLUMN source_conversation_id TEXT;
ALTER TABLE tasks ADD COLUMN source_turn_id TEXT;
ALTER TABLE tasks ADD COLUMN source_item_id TEXT;
ALTER TABLE tasks ADD COLUMN source_tool_call_id TEXT;
ALTER TABLE tasks ADD COLUMN source_client_time_zone TEXT;
`
