package store

// schemaV40SQL restores immutable human authority for Tasks and recurrences.
const schemaV40SQL = `
ALTER TABLE tasks ADD COLUMN authorization_context_json TEXT NOT NULL DEFAULT '{"kind":"none"}'
 CHECK(json_valid(authorization_context_json) AND json_type(authorization_context_json)='object'
 AND length(CAST(authorization_context_json AS BLOB))<=262144);
ALTER TABLE task_recurrences ADD COLUMN authorization_context_json TEXT NOT NULL DEFAULT '{"kind":"none"}'
 CHECK(json_valid(authorization_context_json) AND json_type(authorization_context_json)='object'
 AND length(CAST(authorization_context_json AS BLOB))<=262144);
`
