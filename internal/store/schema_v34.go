package store

// schemaV34SQL releases old choice pauses and keeps the displayed options and selections.
const schemaV34SQL = `
INSERT INTO conversation_items (
    item_id, conversation_id, turn_id, parent_item_id, sequence_index, kind, status,
    author_actor_id, payload_json, metadata_json, created_at_ms, updated_at_ms
)
SELECT 'item:' || lower(hex(randomblob(16))), call.conversation_id, call.turn_id, call.item_id,
    (SELECT MAX(sequence_index) FROM conversation_items WHERE conversation_id = call.conversation_id)
        + ROW_NUMBER() OVER (PARTITION BY call.conversation_id ORDER BY call.sequence_index),
    'tool_result', 'completed', call.author_actor_id,
    json_set(call.payload_json, '$.activity_kind', 'tool_result', '$.status', 'completed',
        '$.title', 'Tool result: noema.present_multiple_choice',
        '$.metadata.action.success', json('true'), '$.metadata.action.payload', json('{"status":"displayed"}')),
    call.metadata_json, call.created_at_ms, call.updated_at_ms
FROM conversation_items call
WHERE call.kind = 'tool_call' AND call.status = 'running'
    AND EXISTS (SELECT 1 FROM conversation_items prompt
        WHERE prompt.kind = 'multiple_choice_prompt' AND prompt.parent_item_id = call.item_id)
    AND NOT EXISTS (SELECT 1 FROM conversation_items result
        WHERE result.kind = 'tool_result' AND result.parent_item_id = call.item_id);
UPDATE conversation_items SET status = 'completed'
WHERE kind = 'tool_call' AND status = 'running' AND item_id IN (
    SELECT parent_item_id FROM conversation_items WHERE kind = 'multiple_choice_prompt'
);
UPDATE conversations SET agent_status = 'idle'
WHERE conversation_id IN (SELECT conversation_id FROM conversation_turns WHERE status = 'waiting_for_tool'
    AND turn_id IN (SELECT turn_id FROM conversation_items WHERE kind = 'multiple_choice_prompt'
        AND json_extract(payload_json, '$.lifecycle') IN ('pending', 'answered')));
UPDATE conversation_turns SET status = 'completed', completed_at_ms = updated_at_ms
WHERE status = 'waiting_for_tool' AND turn_id IN (
    SELECT turn_id FROM conversation_items WHERE kind = 'multiple_choice_prompt'
        AND json_extract(payload_json, '$.lifecycle') IN ('pending', 'answered')
);
UPDATE conversation_items SET payload_json = json_remove(payload_json,
    '$.lifecycle', '$.interaction_revision', '$.provider_selection', '$.provider_round', '$.response_id',
    '$.hosted_state', '$.credential_revision', '$.tool_catalog_digest', '$.call_item_id')
WHERE kind = 'multiple_choice_prompt';
`
