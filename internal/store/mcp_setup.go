package store

import (
	"context"
	"database/sql"
	"errors"
)

// PendingMCPSetupItems returns unresolved hosted setup results for one owned Chat.
func (s *Store) PendingMCPSetupItems(ctx context.Context, conversationID string, limit int) ([]ConversationItem, error) {
	if _, err := s.Conversation(ctx, conversationID); err != nil || limit < 1 || limit > 100 {
		return nil, errors.New("MCP setup conversation is unavailable")
	}
	rows, err := s.db.QueryContext(ctx, `SELECT item_id,conversation_id,COALESCE(turn_id,''),COALESCE(parent_item_id,''),
 sequence_index,kind,status,author_actor_id,COALESCE(content_text,''),COALESCE(provider_content_text,''),payload_json,metadata_json,created_at_ms
 FROM conversation_items WHERE conversation_id=? AND deleted_at_ms IS NULL
 AND json_extract(payload_json,'$.metadata.action.name')='mcp.connect_service'
 AND json_extract(payload_json,'$.metadata.action.success')=1
 AND json_extract(payload_json,'$.metadata.action.payload.status') IN ('needs_auth','authentication_available','ready_for_policy')
 AND json_extract(payload_json,'$.metadata.action.payload.intervention_resolution') IS NULL
 ORDER BY sequence_index DESC LIMIT ?`, conversationID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := make([]ConversationItem, 0)
	for rows.Next() {
		item, err := scanConversationItem(rows)
		if err != nil {
			return nil, err
		}
		result = append(result, item)
	}
	return result, rows.Err()
}

// ResolveMCPSetupItem binds one setup result to a configured matching server.
func (s *Store) ResolveMCPSetupItem(ctx context.Context, conversationID, itemID, serverID string) (bool, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return false, err
	}
	defer tx.Rollback()
	var owner string
	if err := tx.QueryRowContext(ctx, `SELECT owner_human_id FROM conversations WHERE conversation_id=?`, conversationID).Scan(&owner); err != nil || owner != "human:local" {
		return false, errors.New("MCP setup intervention is unavailable")
	}
	result, err := tx.ExecContext(ctx, `UPDATE conversation_items SET payload_json=json_set(payload_json,
 '$.metadata.action.payload.intervention_resolution',json_object('mcp_server_id',?))
 WHERE conversation_id=? AND item_id=? AND deleted_at_ms IS NULL
 AND json_extract(payload_json,'$.metadata.action.name')='mcp.connect_service'
 AND json_extract(payload_json,'$.metadata.action.success')=1
 AND json_extract(payload_json,'$.metadata.action.payload.intervention_resolution') IS NULL`, serverID, conversationID, itemID)
	if err != nil {
		return false, err
	}
	changed, _ := result.RowsAffected()
	if changed == 0 {
		var resolved string
		err = tx.QueryRowContext(ctx, `SELECT json_extract(payload_json,'$.metadata.action.payload.intervention_resolution.mcp_server_id')
 FROM conversation_items WHERE conversation_id=? AND item_id=?`, conversationID, itemID).Scan(&resolved)
		if err != nil || resolved != serverID {
			return false, errors.New("MCP setup intervention changed")
		}
	}
	if err := tx.Commit(); err != nil {
		return false, err
	}
	return changed == 1, nil
}
