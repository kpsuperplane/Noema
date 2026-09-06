package store

// schemaV35SQL gives migrated display results their own activity identity.
const schemaV35SQL = `
UPDATE conversation_items
SET payload_json = json_set(json_remove(payload_json, '$.metadata.action.id', '$.metadata.action.provider_item_id'),
        '$.id', 'tool_result:' || turn_id || ':' || json_extract(metadata_json, '$.provider_round') || ':' || json_extract(metadata_json, '$.output_index'),
        '$.metadata.action.call_id', json_extract(payload_json, '$.id')),
    metadata_json = json_set(metadata_json, '$.source', 'provider_action_result')
WHERE kind = 'tool_result' AND json_extract(metadata_json, '$.source') = 'provider_action'
    AND json_extract(payload_json, '$.metadata.action.name') = 'noema.present_multiple_choice'
    AND json_extract(payload_json, '$.metadata.action.payload.status') = 'displayed';
`
